use axum::http::StatusCode;
use cobotusr::config::BootstrapClient;
use reqwest::Client;

use super::super::common::IdentityFixture;

const SPA_CLIENT_ID: &str = "resource-app-spa-test";
const MACHINE_CLIENT_ID: &str = "resource-app-sync-test";
const MACHINE_CLIENT_SECRET: &str = "machine-secret";
const PERMISSION_NAME: &str = "device:read";

/// Boot a cobotusr with a public SPA client + a confidential machine client,
/// mirroring the production resource-app/resource-app-sync split. Returns the Postgres
/// container (must stay alive for the server's lifetime), base URL and an
/// admin access token for verifying results via the permission API.
async fn start_server() -> (IdentityFixture, String, String) {
    let fixture = IdentityFixture::start_configured(|config| {
        config.oidc.bootstrap_clients.push(BootstrapClient {
            client_id: SPA_CLIENT_ID.to_string(),
            name: "SPA test client".to_string(),
            redirect_uris: vec!["http://localhost:4001/oauth/callback".to_string()],
            allowed_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            client_secret: None,
            public_client: true,
        });
        config.oidc.bootstrap_clients.push(BootstrapClient {
            client_id: MACHINE_CLIENT_ID.to_string(),
            name: "Machine test client".to_string(),
            redirect_uris: vec![],
            allowed_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            grant_types: vec!["client_credentials".to_string()],
            client_secret: Some(MACHINE_CLIENT_SECRET.to_string()),
            public_client: false,
        });
    })
    .await;
    super::super::common::seed_legacy_bootstrap(&fixture).await;
    let access_token = super::identity_support::management_access(&fixture).await;
    let base = fixture.base_url.clone();
    (fixture, base, access_token)
}

async fn sync_permissions(
    client: &Client,
    base_url: &str,
    body: serde_json::Value,
) -> reqwest::Response {
    client
        .post(format!("{base_url}/api/v1/application/sync-permissions"))
        .json(&body)
        .send()
        .await
        .expect("sync-permissions request should complete")
}

#[tokio::test]
async fn test_sync_permissions_app_key_proxy_and_guards() {
    let (fixture, base_url, access_token) = start_server().await;
    let client = Client::new();

    // ── 1. machine client + app_key = SPA client → permissions land on the
    //       SPA client's application (the one PDP / require_perm! checks).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": MACHINE_CLIENT_SECRET,
            "app_key": SPA_CLIENT_ID,
            "permissions": [{
                "name": PERMISSION_NAME,
                "action": "read",
                "resource_type": "device",
            }],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("sync response should be JSON");
    let app_id = body["application_id"]
        .as_i64()
        .expect("sync response should carry application_id");
    assert_eq!(body["synced"], 1);

    let perms = client
        .get(format!("{base_url}/api/v1/permission"))
        .query(&[
            ("application_id", app_id.to_string()),
            ("page_size", "50".to_string()),
            ("page_number", "1".to_string()),
        ])
        .bearer_auth(&access_token)
        .send()
        .await
        .expect("permission list request should complete")
        .error_for_status()
        .expect("permission list should be authorized");
    let perms_body: serde_json::Value = perms.json().await.expect("permission list JSON");
    let names: Vec<&str> = perms_body["data_list"]
        .as_array()
        .expect("data_list should be an array")
        .iter()
        .map(|p| p["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        names.contains(&PERMISSION_NAME),
        "permission should be registered under the app_key application, got: {names:?}"
    );

    // ── 2. unknown app_key → 400 (explicit app_keys must already exist).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": MACHINE_CLIENT_SECRET,
            "app_key": "no-such-application",
            "permissions": [],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // ── 3. no app_key → defaults to the authenticating client's own
    //       application (legacy behaviour).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": MACHINE_CLIENT_SECRET,
            "permissions": [{
                "name": "mcp:access",
                "action": "access",
                "resource_type": "mcp",
            }],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("sync response should be JSON");
    assert_eq!(body["synced"], 1);

    // ── 4. public client cannot authenticate (no secret to verify).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": SPA_CLIENT_ID,
            "client_secret": "anything",
            "permissions": [],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // ── 5. wrong secret on the machine client → 401.
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": "wrong-secret",
            "permissions": [],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    fixture.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_sync_has_no_managed_or_implicit_creation_path_and_is_atomic() {
    use super::identity_support::{admin_id, scalar};
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let (fixture, base_url, _) = start_server().await;
    let body = serde_json::json!({"client_id":MACHINE_CLIENT_ID,"client_secret":MACHINE_CLIENT_SECRET,
        "app_key":SPA_CLIENT_ID,"permissions":[{"name":"sync-atomic","action":"read"}]});
    fixture.db.execute_unprepared("CREATE FUNCTION reject_sync_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END; $$; CREATE CONSTRAINT TRIGGER reject_sync_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_sync_commit()").await.unwrap();
    let rejected = sync_permissions(&fixture.client, &base_url, body.clone()).await;
    assert!(rejected.status().is_server_error());
    assert_eq!(
        scalar(
            &fixture,
            "SELECT COUNT(*)::bigint AS value FROM permission WHERE name='sync-atomic'"
        )
        .await,
        0
    );
    assert_eq!(scalar(&fixture,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='legacy.permissions.sync'").await,0);
    fixture
        .db
        .execute_unprepared("DROP TRIGGER reject_sync_commit ON identity_audit_event")
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        sync_permissions(&fixture.client, &base_url, body.clone()),
        sync_permissions(&fixture.client, &base_url, body.clone())
    );
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(
        scalar(
            &fixture,
            "SELECT COUNT(*)::bigint AS value FROM permission WHERE name='sync-atomic'"
        )
        .await,
        1
    );
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE application SET management_state='managed' WHERE app_key=$1",
            vec![SPA_CLIENT_ID.into()],
        ))
        .await
        .unwrap();
    assert_eq!(
        sync_permissions(&fixture.client, &base_url, body.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE application SET management_state='legacy_unmanaged' WHERE app_key=$1",
            vec![SPA_CLIENT_ID.into()],
        ))
        .await
        .unwrap();
    let managed = super::identity_support::app(&fixture, "sync-managed", false).await;
    let managed_body = serde_json::json!({"client_id":"sync-managed-dev","client_secret":"fixture-client-secret","app_key":SPA_CLIENT_ID,"permissions":[]});
    assert!(
        sync_permissions(&fixture.client, &base_url, managed_body)
            .await
            .status()
            .is_client_error()
    );
    assert!(managed.app > 0);
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE oauth_client SET management_state='legacy_unmanaged' WHERE client_id=$1",
            vec![MACHINE_CLIENT_ID.into()],
        ))
        .await
        .unwrap();
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM application WHERE app_key=$1",
            vec![MACHINE_CLIENT_ID.into()],
        ))
        .await
        .unwrap();
    let implicit = serde_json::json!({"client_id":MACHINE_CLIENT_ID,"client_secret":MACHINE_CLIENT_SECRET,"permissions":[]});
    assert_eq!(
        sync_permissions(&fixture.client, &base_url, implicit)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(scalar(&fixture,&format!("SELECT COUNT(*)::bigint AS value FROM application WHERE app_key='{MACHINE_CLIENT_ID}'")).await,0);
    // A client owner with no current enterprise administrator qualification is refused.
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE \"user\" SET role_id=3 WHERE id=$1",
            vec![admin_id(&fixture).await.into()],
        ))
        .await
        .unwrap();
    assert_eq!(
        sync_permissions(&fixture.client, &base_url, body)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.close().await;
}

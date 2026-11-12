use super::super::common::IdentityFixture;
use super::identity_support::*;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::json;

async fn person(f: &IdentityFixture, username: &str, role_id: i32) -> i32 {
    let hash = bcrypt::hash("fixture-person-password", bcrypt::DEFAULT_COST).unwrap();
    let id:i32=f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO \"user\" (username,password,role_id,create_time,update_time) VALUES ($1,$2,$3,NOW(),NOW()) RETURNING id",
        [username.into(),hash.into(),role_id.into()])).await.unwrap().unwrap().try_get("","id").unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_subject (issuer,sub,user_id,created_at) VALUES ($1,$2,$3,0)",
        [f.base_url.clone().into(), id.to_string().into(), id.into()],
    ))
    .await
    .unwrap();
    id
}
async fn token(f: &IdentityFixture) -> String {
    let (root, _) = center_secret(f, admin_id(f).await).await;
    management_tokens(f, &root)
        .await
        .access_token
        .expose_secret()
        .into()
}
async fn request(
    f: &IdentityFixture,
    token: &str,
    method: reqwest::Method,
    path: &str,
    body: serde_json::Value,
) -> reqwest::Response {
    f.client
        .request(method, format!("{}/api/v1{path}", f.base_url))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap()
}
async fn status(response: reqwest::Response, expected: u16) -> String {
    let actual = response.status().as_u16();
    let body = response.text().await.unwrap();
    assert_eq!(actual, expected, "{body}");
    body
}

#[tokio::test]
async fn identity_migration_legacy_consent_is_atomic_and_new_code_is_digest_only() {
    use yufu::service::oidc::OidcService;
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let (client, secret) = OidcService::register_client(
        &f.db,
        admin_id(&f).await,
        "legacy-consent".into(),
        "Legacy consent".into(),
        vec!["https://legacy.example/callback".into()],
        vec!["openid".into()],
        vec!["authorization_code".into()],
        false,
    )
    .await
    .unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ($1,$2,$2,$2,TRUE)",vec![f.base_url.clone().into(),client.client_id.clone().into()])).await.unwrap();
    let body = json!({"client_id":client.client_id,"redirect_uri":"https://legacy.example/callback","response_type":"code","scope":"openid","state":"state& =+/中文","nonce":"legacy-nonce","approved":true});
    f.db.execute_unprepared("CREATE FUNCTION reject_consent_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END; $$; CREATE CONSTRAINT TRIGGER reject_consent_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_consent_commit()").await.unwrap();
    let failure = request(
        &f,
        &manager,
        reqwest::Method::POST,
        "/oidc/authorize/consent",
        body.clone(),
    )
    .await;
    assert!(failure.status().is_server_error());
    let failure = failure.text().await.unwrap();
    assert!(!failure.contains("redirect_url"));
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE client_id='legacy-consent'").await,0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_consent WHERE client_id='legacy-consent'"
        )
        .await,
        0
    );
    f.db.execute_unprepared("DROP TRIGGER reject_consent_commit ON identity_audit_event")
        .await
        .unwrap();
    let completed = status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            "/oidc/authorize/consent",
            body.clone(),
        )
        .await,
        200,
    )
    .await;
    let value: serde_json::Value = serde_json::from_str(&completed).unwrap();
    let redirect = reqwest::Url::parse(value["redirect_url"].as_str().unwrap()).unwrap();
    let fields = redirect
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(fields.get("state").unwrap(), "state& =+/中文");
    let code = fields.get("code").unwrap().to_string();
    let digest = format!(
        "sha256:{}",
        yufu::domain::identity::account::SecretDigest::of(&code).hex()
    );
    let stored=f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT code,EXTRACT(EPOCH FROM expires_at-create_time)::bigint AS ttl FROM oauth_authorization_code WHERE client_id=$1",vec![client.client_id.clone().into()])).await.unwrap().unwrap();
    assert_eq!(stored.try_get::<String>("", "code").unwrap(), digest);
    assert_ne!(digest, code);
    assert_eq!(stored.try_get::<i64>("", "ttl").unwrap(), 60);
    let exchange = |value: String| {
        f.client
            .post(format!("{}/oidc/token", f.base_url))
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", client.client_id.as_str()),
                ("client_secret", secret.as_deref().unwrap()),
                ("redirect_uri", "https://legacy.example/callback"),
                ("code", value.as_str()),
            ])
            .send()
    };
    assert!(exchange(digest).await.unwrap().status().is_client_error());
    // The failed use of a leaked digest does not consume the real code.
    assert!(exchange(code).await.unwrap().status().is_success());
    let a = app(&f, "managed-consent", false).await;
    let managed_id: String =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT client_id FROM oauth_client WHERE environment_id=$1",
            vec![a.development.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "client_id")
        .unwrap();
    let mut managed_body = body;
    managed_body["client_id"] = json!(managed_id);
    status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            "/oidc/authorize/consent",
            managed_body,
        )
        .await,
        400,
    )
    .await;
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE environment_id IS NOT NULL").await,0);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_account_role_changes_revoke_roots_and_preserve_subjects() {
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let target = person(&f, "legacy-target", 3).await;
    let (root, secret) = center_secret(&f, target).await;
    status(
        request(
            &f,
            &manager,
            reqwest::Method::PATCH,
            "/user/role",
            json!({"user_id":target,"new_role_id":2}),
        )
        .await,
        200,
    )
    .await;
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT auth_epoch AS value FROM \"user\" WHERE id={target}")
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NOT NULL",root.id)).await,1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='legacy.management.change'").await,1);
    let restore = yufu::service::identity::sso::SsoService::new(
        std::sync::Arc::new(repository(&f)),
        f.clock.clone(),
        f.config.clone(),
    )
    .unwrap();
    assert!(
        restore
            .authenticate_browser(&secret, &budget(&f))
            .await
            .is_err()
    );
    status(
        request(
            &f,
            &manager,
            reqwest::Method::DELETE,
            &format!("/user/{target}"),
            json!(null),
        )
        .await,
        200,
    )
    .await;
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE id={target} AND NOT enabled"
            )
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT COUNT(*)::bigint AS value FROM identity_subject WHERE user_id={target}"
            )
        )
        .await,
        1
    );
    status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            "/user",
            json!({"username":"bypass","password":"fixture-password","role_id":3}),
        )
        .await,
        400,
    )
    .await;
    status(
        request(
            &f,
            &manager,
            reqwest::Method::PATCH,
            "/user",
            json!({"user_id":target,"password":"new-password"}),
        )
        .await,
        400,
    )
    .await;
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE username='bypass'"
        )
        .await,
        0
    );
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_last_administrator_and_active_owner_are_protected() {
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let admin = admin_id(&f).await;
    let result = status(
        request(
            &f,
            &manager,
            reqwest::Method::PATCH,
            "/user/role",
            json!({"user_id":admin,"new_role_id":3}),
        )
        .await,
        409,
    )
    .await;
    assert!(result.contains("last_administrator"));
    person(&f, "second-admin", 1).await;
    let app = app(&f, "legacy-owner", true).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE application SET management_mode='admin_configured' WHERE id=$1",
        [app.app.into()],
    ))
    .await
    .unwrap();
    let result = status(
        request(
            &f,
            &manager,
            reqwest::Method::PATCH,
            "/user/role",
            json!({"user_id":admin,"new_role_id":3}),
        )
        .await,
        409,
    )
    .await;
    assert!(result.contains("owner_required"), "{result}");
    status(
        request(
            &f,
            &manager,
            reqwest::Method::DELETE,
            &format!("/user/{admin}"),
            json!(null),
        )
        .await,
        409,
    )
    .await;
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT role_id::bigint AS value FROM \"user\" WHERE id={admin}")
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='legacy.management.change'").await,0);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_rbac_batch_and_commit_failure_roll_back() {
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let target = person(&f, "rbac-target", 3).await;
    let (root, _) = center_secret(&f, target).await;
    let role:i32=f.db.query_one_raw(Statement::from_string(DbBackend::Postgres,"INSERT INTO role (name,create_time,update_time) VALUES ('legacy-rbac',NOW(),NOW()) RETURNING id")).await.unwrap().unwrap().try_get("","id").unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET role_id=$2 WHERE id=$1",
        [target.into(), role.into()],
    ))
    .await
    .unwrap();
    let authority: i32 =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id FROM authority WHERE method='GET' AND authority_uri='/user'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    let response = request(
        &f,
        &manager,
        reqwest::Method::POST,
        "/role_authority/batch",
        json!({"role_id":role,"authority_ids":[authority,2147483647]}),
    )
    .await;
    assert!(response.status().is_client_error() || response.status().is_server_error());
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT COUNT(*)::bigint AS value FROM authority_role WHERE role_id={role}")
        )
        .await,
        0
    );
    // A deferred trigger fails at COMMIT, after both state and audit were written.
    f.db.execute_unprepared("CREATE FUNCTION reject_legacy_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END; $$; CREATE CONSTRAINT TRIGGER reject_legacy_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_legacy_commit()").await.unwrap();
    let response = request(
        &f,
        &manager,
        reqwest::Method::POST,
        "/role_authority",
        json!({"role_id":role,"authority_id":authority}),
    )
    .await;
    assert_eq!(response.status(), 503);
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT COUNT(*)::bigint AS value FROM authority_role WHERE role_id={role}")
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NULL",root.id)).await,1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='legacy.management.change'").await,0);
    f.db.execute_unprepared("DROP TRIGGER reject_legacy_commit ON identity_audit_event; DROP FUNCTION reject_legacy_commit()").await.unwrap();
    status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            "/role_authority",
            json!({"role_id":role,"authority_id":authority}),
        )
        .await,
        200,
    )
    .await;
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NOT NULL",root.id)).await,1);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_client_configuration_and_directory_cannot_bypass_governance() {
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let a = app(&f, "legacy-client-guard", false).await;
    let client = scalar(
        &f,
        &format!(
            "SELECT oauth_client_id::bigint AS value FROM identity_app_environment WHERE id='{}'",
            a.development
        ),
    )
    .await;
    for (method, path, body) in [
        (
            reqwest::Method::POST,
            "/oidc/clients".into(),
            json!({"name":"bypass","redirect_uri":"https://evil.example/callback"}),
        ),
        (
            reqwest::Method::PATCH,
            format!("/oidc/client/{client}"),
            json!({"redirect_uris":["https://evil.example/callback"]}),
        ),
        (
            reqwest::Method::DELETE,
            format!("/oidc/client/{client}"),
            json!(null),
        ),
        (
            reqwest::Method::POST,
            format!("/oidc/client/{client}/secret"),
            json!(null),
        ),
        (
            reqwest::Method::PATCH,
            "/application".into(),
            json!({"id":a.app,"name":"bypass"}),
        ),
        (
            reqwest::Method::DELETE,
            "/application".into(),
            json!({"id":a.app}),
        ),
    ] {
        status(request(&f, &manager, method, &path, body).await, 400).await;
    }
    let user = person(&f, "directory-developer", 3).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET developer_enabled=TRUE,person_kind='collaborator' WHERE id=$1",
        [user.into()],
    ))
    .await
    .unwrap();
    let (root, _) = center_secret(&f, user).await;
    // A signature-valid credential absent from the registry is insufficient.
    let unregistered = f
        .signer
        .sign_access_token(
            &user.to_string(),
            &f.base_url,
            "yufu",
            "",
            300,
            vec![],
            vec![],
        )
        .unwrap();
    status(
        request(
            &f,
            &unregistered,
            reqwest::Method::GET,
            "/users",
            json!(null),
        )
        .await,
        401,
    )
    .await;
    let delegated = identity_tokens(&f, &a, &center(&f, a.owner).await, "openid").await;
    status(
        request(
            &f,
            delegated.0.access_token.expose_secret(),
            reqwest::Method::GET,
            "/users",
            json!(null),
        )
        .await,
        401,
    )
    .await;
    let collaborator =
        management_tokens_for_user(&f, &root, "directory-developer", "fixture-person-password")
            .await;
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,TRUE,NOW(),NOW() FROM authority WHERE method='GET' AND authority_uri='/users'",[user.into()])).await.unwrap();
    status(
        request(
            &f,
            collaborator.access_token.expose_secret(),
            reqwest::Method::GET,
            "/users?page_number=1&page_size=10",
            json!(null),
        )
        .await,
        403,
    )
    .await;
    status(
        request(
            &f,
            &manager,
            reqwest::Method::GET,
            "/users?page_number=1&page_size=10",
            json!(null),
        )
        .await,
        200,
    )
    .await;
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_concurrent_admin_demotion_preserves_one_administrator() {
    let f = IdentityFixture::start().await;
    let first = admin_id(&f).await;
    let second = person(&f, "concurrent-admin", 1).await;
    let first_token = token(&f).await;
    let (root, _) = center_secret(&f, second).await;
    let second_token =
        management_tokens_for_user(&f, &root, "concurrent-admin", "fixture-person-password").await;
    let (a, b) = tokio::join!(
        request(
            &f,
            &first_token,
            reqwest::Method::PATCH,
            "/user/role",
            json!({"user_id":first,"new_role_id":3})
        ),
        request(
            &f,
            second_token.access_token.expose_secret(),
            reqwest::Method::PATCH,
            "/user/role",
            json!({"user_id":second,"new_role_id":3})
        )
    );
    let mut results = [a.status().as_u16(), b.status().as_u16()];
    results.sort();
    assert_eq!(results, [200, 409]);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE role_id=1 AND enabled"
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='legacy.management.change'").await,1);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_effective_permission_denial_revokes_once() {
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let target = person(&f, "permission-target", 3).await;
    let a = app(&f, "permission-source", false).await;
    let permission = yufu::entity::permission::Entity::insert_one(
        &f.db,
        yufu::entity::permission::PermissionInsertReq {
            application_id: a.app,
            name: "device:read".into(),
            description: None,
            action: "read".into(),
            resource_type: Some("device".into()),
            resource_pattern: Some("{}".into()),
        },
    )
    .await
    .unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO role_permission (role_id,permission_id,create_time) VALUES (3,$1,NOW())",
        [permission.id.into()],
    ))
    .await
    .unwrap();
    let (root, _) = center_secret(&f, target).await;
    let body = json!({"user_id":target,"permission_id":permission.id,"allow":false});
    status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            "/user_permission_override",
            body.clone(),
        )
        .await,
        200,
    )
    .await;
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT auth_epoch AS value FROM \"user\" WHERE id={target}")
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NOT NULL",root.id)).await,1);
    // Idempotent denial does not invalidate a freshly authenticated root again.
    let (fresh, _) = center_secret(&f, target).await;
    status(
        request(
            &f,
            &manager,
            reqwest::Method::PATCH,
            "/user_permission_override",
            body,
        )
        .await,
        200,
    )
    .await;
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT auth_epoch AS value FROM \"user\" WHERE id={target}")
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NULL",fresh.id)).await,1);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_legacy_client_writes_commit_with_audit_and_no_secret_on_failure() {
    use sea_orm::EntityTrait;
    use yufu::service::oidc::OidcService;
    let f = IdentityFixture::start().await;
    let manager = token(&f).await;
    let (client, old_secret) = OidcService::register_client(
        &f.db,
        admin_id(&f).await,
        "orphan-client".into(),
        "Legacy".into(),
        vec!["https://rp.example/callback".into()],
        vec!["openid".into()],
        vec!["authorization_code".into()],
        false,
    )
    .await
    .unwrap();
    let path = format!("/oidc/client/{}", client.id);
    status(
        request(
            &f,
            &manager,
            reqwest::Method::PATCH,
            &path,
            json!({"name":"Legacy updated"}),
        )
        .await,
        200,
    )
    .await;
    f.db.execute_unprepared("CREATE FUNCTION reject_client_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END; $$; CREATE CONSTRAINT TRIGGER reject_client_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_client_commit()").await.unwrap();
    let failed = status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            &format!("{path}/secret"),
            json!(null),
        )
        .await,
        503,
    )
    .await;
    assert!(!failed.contains("client_secret"));
    let current = yufu::entity::oauth_client::Entity::find_by_id(client.id)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.client_secret_hash, client.client_secret_hash);
    assert!(OidcService::client_verify_secret(
        &current,
        old_secret.as_deref().unwrap()
    ));
    f.db.execute_unprepared("DROP TRIGGER reject_client_commit ON identity_audit_event; DROP FUNCTION reject_client_commit()").await.unwrap();
    let changed = status(
        request(
            &f,
            &manager,
            reqwest::Method::POST,
            &format!("{path}/secret"),
            json!(null),
        )
        .await,
        200,
    )
    .await;
    let changed: serde_json::Value = serde_json::from_str(&changed).unwrap();
    let current = yufu::entity::oauth_client::Entity::find_by_id(client.id)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert!(OidcService::client_verify_secret(
        &current,
        changed["client_secret"].as_str().unwrap()
    ));
    status(
        request(&f, &manager, reqwest::Method::DELETE, &path, json!(null)).await,
        204,
    )
    .await;
    assert!(
        yufu::entity::oauth_client::Entity::find_by_id(client.id)
            .one(&f.db)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='legacy.management.change'").await,3);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE change_summary::text LIKE '%client_secret%'").await,0);
    f.close().await;
}

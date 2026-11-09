use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_login},
    identity_support::*,
};
use cobotusr::{
    domain::identity::{
        account::IssuedSecret,
        application::{AppEnvironmentKind, ClientKind},
        error::IdentityError,
        migration::AdoptLegacyApplication,
        registration::{CreateAdminApplication, CreateApplication},
    },
    service::{
        identity::{application::ApplicationService, repository::*},
        oidc::{CreateAuthCodeParams, OidcService},
    },
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;
const SECRET: &str = "historical-client-secret";
const CALLBACK: &str = "https://rp.example/callback";
async fn legacy(f: &IdentityFixture, key: &str, linked: bool, public: bool) -> (i32, Option<i32>) {
    let owner = admin_id(f).await;
    let hash = if public {
        String::new()
    } else {
        bcrypt::hash(SECRET, bcrypt::DEFAULT_COST).unwrap()
    };
    let client:i32=f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,client_secret_hash,public_client,grant_types,create_time,update_time) VALUES ($1,$2,$1,$3,'openid,profile,email',$4,$5,'authorization_code,refresh_token',NOW(),NOW()) RETURNING id",
        vec![key.into(),owner.into(),CALLBACK.into(),hash.into(),public.into()])).await.unwrap().unwrap().try_get("","id").unwrap();
    let app = if linked {
        Some(f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO application (app_key,name,oidc_client_id,create_time,update_time) VALUES ($1,$1,$2,NOW(),NOW()) RETURNING id",
        vec![key.into(),client.into()])).await.unwrap().unwrap().try_get("","id").unwrap())
    } else {
        None
    };
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ($1,$2,$2,$2,TRUE)",vec![f.base_url.clone().into(),key.into()])).await.unwrap();
    (client, app)
}
fn service(f: &IdentityFixture) -> ApplicationService {
    ApplicationService::new(Arc::new(repository(f)), f.clock.clone())
}
async fn adoption(
    f: &IdentityFixture,
    id: i32,
    app: Option<i32>,
    key: &str,
    retained: AppEnvironmentKind,
    kind: ClientKind,
) -> AdoptLegacyApplication {
    let mut tx = repository(f).begin(&budget(f)).await.unwrap();
    let snapshot = tx.legacy_workspace(id).await.unwrap().summary.snapshot;
    tx.rollback().await.unwrap();
    let other = if kind == ClientKind::BrowserSpa {
        ClientKind::ServerWeb
    } else {
        kind
    };
    let (dev, prod) = match retained {
        AppEnvironmentKind::Development => (kind, other),
        AppEnvironmentKind::Production => (other, kind),
    };
    AdoptLegacyApplication {
        oauth_client_id: id,
        expected_snapshot: snapshot,
        existing_application_id: app,
        retained_environment: retained,
        registration: CreateAdminApplication {
            application: CreateApplication {
                app_key: key.into(),
                name: "Adopted service".into(),
                description: None,
                third_party: true,
                development: config(AppEnvironmentKind::Development, dev, false).to_input(),
                production: config(AppEnvironmentKind::Production, prod, false).to_input(),
            },
            owner_user_id: admin_id(f).await,
            assignments: vec![AssignmentTarget::User(admin_id(f).await)],
            accept_logout_limitation: false,
        },
    }
}
async fn code(f: &IdentityFixture, key: &str) -> String {
    OidcService::create_authorization_code(
        &f.db,
        CreateAuthCodeParams {
            user_id: admin_id(f).await,
            client_id: key.into(),
            redirect_uri: Some(CALLBACK.into()),
            code_challenge: None,
            code_challenge_method: "S256".into(),
            scope: Some("openid profile email".into()),
            state: Some("historical-state".into()),
            nonce: Some("historical-nonce".into()),
        },
    )
    .await
    .unwrap()
}
async fn tokens(f: &IdentityFixture, key: &str) -> serde_json::Value {
    let code = code(f, key).await;
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", key),
            ("client_secret", SECRET),
            ("redirect_uri", CALLBACK),
            ("code", &code),
        ])
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert!(status.is_success(), "{body}");
    serde_json::from_str(&body).unwrap()
}
async fn hash(f: &IdentityFixture, id: i32) -> String {
    f.db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT client_secret_hash AS value FROM oauth_client WHERE id=$1",
        vec![id.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "value")
    .unwrap()
}

#[tokio::test]
async fn identity_migration_adoption_scans_historical_jwt_issuer_without_disclosing_tokens() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let f = IdentityFixture::start().await;
    let (client, app) = legacy(&f, "history-jwt-issuer", true, false).await;
    let original_hash = hash(&f, client).await;
    let input = adoption(
        &f,
        client,
        app,
        "history-jwt-issuer",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await;
    let token = format!(
        "historical.{}.signature",
        URL_SAFE_NO_PAD.encode(br#"{"iss":"https://different.example","sub":"old-sub"}"#)
    );
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO oauth_token (user_id,client_id,access_token,id_token,refresh_token,token_type,create_time,update_time,access_expires_at,refresh_expires_at,revoked) VALUES ($1,$2,$3,$3,'opaque-fixture-refresh','Bearer',NOW(),NOW(),NOW()+INTERVAL '1 minute',NOW()+INTERVAL '1 hour',FALSE)",
        vec![admin_id(&f).await.into(),"history-jwt-issuer".into(),token.clone().into()])).await.unwrap();
    let (_, root) = center_secret(&f, admin_id(&f).await).await;
    let e = write_evidence(
        &f,
        Some(&root),
        &IssuedSecret::from_entropy([73; 32]).into_response(),
    );
    let s = service(&f);
    let scan = s
        .scan_legacy(&e, 0, 50, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert!(
        scan.items[0]
            .conflict_kinds
            .iter()
            .any(|kind| kind == "issuer_conflict")
    );
    assert_ne!(scan.items[0].snapshot, input.expected_snapshot);
    let safe = serde_json::to_string(&scan).unwrap();
    assert!(!safe.contains(&token));
    assert!(!safe.contains("old-sub"));
    assert!(!safe.contains("opaque-fixture-refresh"));
    let mut current = input;
    current.expected_snapshot = scan.items[0].snapshot.clone();
    assert!(
        s.adopt_legacy(&e, current, "jwt-conflict", Uuid::new_v4(), &budget(&f))
            .await
            .is_err()
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_environment"
        )
        .await,
        0
    );
    assert_eq!(hash(&f, client).await, original_hash);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_adoption_preserves_identifiers_and_revokes_only_historical_client() {
    let f = IdentityFixture::start().await;
    let (client, app) = legacy(&f, "history-related", true, false).await;
    legacy(&f, "history-other", true, false).await;
    let old = tokens(&f, "history-related").await;
    let other = tokens(&f, "history-other").await;
    let pending = code(&f, "history-related").await;
    OidcService::save_consent(
        &f.db,
        admin_id(&f).await,
        "history-related",
        "openid profile",
    )
    .await
    .unwrap();
    let old_hash = hash(&f, client).await;
    let key = tokio::fs::read(&f.key_path).await.unwrap();
    let (_, root) = center_secret(&f, admin_id(&f).await).await;
    let e = write_evidence(
        &f,
        Some(&root),
        &IssuedSecret::from_entropy([64; 32]).into_response(),
    );
    let s = service(&f);
    let input = adoption(
        &f,
        client,
        app,
        "history-related",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await;
    let scan = s
        .scan_legacy(&e, 0, 50, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(scan.items.len(), 2);
    let safe = serde_json::to_string(&scan).unwrap();
    assert!(!safe.contains(&old_hash));
    assert!(!safe.contains(old["access_token"].as_str().unwrap()));
    s.scan_legacy(&e, 0, 50, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_migration_conflict WHERE resolved_at IS NULL").await,2);
    let a = budget(&f);
    let b = budget(&f);
    let (one, two) = tokio::join!(
        s.adopt_legacy(&e, input.clone(), "same-adoption", Uuid::new_v4(), &a),
        s.adopt_legacy(&e, input.clone(), "same-adoption", Uuid::new_v4(), &b)
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.operation_id, two.operation_id);
    assert_eq!(one.application_id, app.unwrap());
    assert_eq!(
        one.newly_issued_secrets.len() + two.newly_issued_secrets.len(),
        1
    );
    assert_eq!(
        one.environments
            .iter()
            .find(|e| e.kind == AppEnvironmentKind::Production)
            .unwrap()
            .client_id,
        "history-related"
    );
    assert_eq!(hash(&f, client).await, old_hash);
    assert_eq!(tokio::fs::read(&f.key_path).await.unwrap(), key);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind='application.legacy.adopt'").await,1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_migration_conflict WHERE resolution='explicit_adoption'").await,1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM security_credential WHERE oidc_client_id='history-related' AND revoked_at IS NULL").await,0);
    assert!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM security_credential WHERE oidc_client_id='history-other' AND revoked_at IS NULL").await>0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_consent WHERE client_id='history-related'"
        )
        .await,
        0
    );
    for (token, expected) in [(&old, 401), (&other, 200)] {
        let response = f
            .client
            .get(format!("{}/oidc/userinfo", f.base_url))
            .bearer_auth(token["access_token"].as_str().unwrap())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
    }
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", "history-related"),
            ("client_secret", SECRET),
            ("redirect_uri", CALLBACK),
            ("code", pending.as_str()),
        ])
        .send()
        .await
        .unwrap();
    assert!(!response.status().is_success());
    let replay = s
        .adopt_legacy(
            &e,
            input.clone(),
            "same-adoption",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(replay.newly_issued_secrets.is_empty());
    let mut changed = input;
    changed.registration.application.name = "Different payload".into();
    assert_eq!(
        s.adopt_legacy(&e, changed, "same-adoption", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    f.close().await;
}

#[tokio::test]
async fn identity_migration_adoption_orphan_spa_retains_development_without_recovering_old_secrets()
{
    let f = IdentityFixture::start().await;
    let (client, app) = legacy(&f, "orphan-spa", false, true).await;
    let (_, root) = center_secret(&f, admin_id(&f).await).await;
    let e = write_evidence(
        &f,
        Some(&root),
        &IssuedSecret::from_entropy([65; 32]).into_response(),
    );
    let s = service(&f);
    let input = adoption(
        &f,
        client,
        app,
        "orphan-managed",
        AppEnvironmentKind::Development,
        ClientKind::BrowserSpa,
    )
    .await;
    let scan = s
        .scan_legacy(&e, 0, 50, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(scan.items[0].conflict_kinds, vec!["orphan_client"]);
    let result = s
        .adopt_legacy(&e, input, "orphan-adopt", Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(result.newly_issued_secrets.len(), 1);
    assert_eq!(
        result
            .environments
            .iter()
            .find(|e| e.kind == AppEnvironmentKind::Development)
            .unwrap()
            .client_id,
        "orphan-spa"
    );
    assert_eq!(hash(&f, client).await, "");
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE management_state='managed'"
        )
        .await,
        2
    );
    f.close().await;
}

#[tokio::test]
async fn identity_migration_adoption_conflicts_require_explicit_repair_and_current_snapshot() {
    let f = IdentityFixture::start().await;
    let (client, app) = legacy(&f, "conflicted-history", true, false).await;
    let (_, root) = center_secret(&f, admin_id(&f).await).await;
    let e = write_evidence(
        &f,
        Some(&root),
        &IssuedSecret::from_entropy([66; 32]).into_response(),
    );
    let s = service(&f);
    let mut input = adoption(
        &f,
        client,
        app,
        "conflicted-history",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await;
    f.db.execute_unprepared("INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://different.example','conflicted-history','conflicted-history','conflicted-history',TRUE)").await.unwrap();
    input.expected_snapshot = adoption(
        &f,
        client,
        app,
        "conflicted-history",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await
    .expected_snapshot;
    let scan = s
        .scan_legacy(&e, 0, 50, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert!(
        scan.items[0]
            .conflict_kinds
            .contains(&"issuer_conflict".into())
    );
    assert_eq!(
        s.adopt_legacy(&e, input.clone(), "bad-issuer", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    f.db.execute_unprepared("DELETE FROM security_session_client WHERE issuer='https://different.example'; INSERT INTO application (app_key,name,oidc_client_id,create_time,update_time) SELECT 'conflict-second','Second',id,NOW(),NOW() FROM oauth_client WHERE client_id='conflicted-history'").await.unwrap();
    input.expected_snapshot = adoption(
        &f,
        client,
        app,
        "conflicted-history",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await
    .expected_snapshot;
    assert_eq!(
        s.adopt_legacy(&e, input.clone(), "multiple", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    f.db.execute_unprepared("DELETE FROM application WHERE app_key='conflict-second'")
        .await
        .unwrap();
    assert_eq!(
        s.adopt_legacy(&e, input.clone(), "stale", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    input.expected_snapshot = adoption(
        &f,
        client,
        app,
        "conflicted-history",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await
    .expected_snapshot;
    input.registration.application.production = config(
        AppEnvironmentKind::Production,
        ClientKind::BrowserSpa,
        false,
    )
    .to_input();
    assert_eq!(
        s.adopt_legacy(&e, input, "wrong-type", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::InvalidInput)
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind='application.legacy.adopt'").await,0);
    f.close().await;
}

#[tokio::test]
async fn identity_migration_adoption_commit_failure_rolls_back_client_revocation_and_receipt() {
    let f = IdentityFixture::start().await;
    let (client, app) = legacy(&f, "failure-history", true, false).await;
    let old = tokens(&f, "failure-history").await;
    let old_hash = hash(&f, client).await;
    let (_, root) = center_secret(&f, admin_id(&f).await).await;
    let e = write_evidence(
        &f,
        Some(&root),
        &IssuedSecret::from_entropy([67; 32]).into_response(),
    );
    let s = service(&f);
    let input = adoption(
        &f,
        client,
        app,
        "failure-history",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await;
    f.db.execute_unprepared("CREATE FUNCTION reject_adoption_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END; $$; CREATE CONSTRAINT TRIGGER reject_adoption_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_adoption_commit()").await.unwrap();
    assert!(
        s.adopt_legacy(
            &e,
            input.clone(),
            "failed-adopt",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    assert_eq!(hash(&f, client).await, old_hash);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_environment"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE management_state='managed'"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind='application.legacy.adopt'").await,0);
    let response = f
        .client
        .get(format!("{}/oidc/userinfo", f.base_url))
        .bearer_auth(old["access_token"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    f.db.execute_unprepared("DROP TRIGGER reject_adoption_commit ON identity_audit_event; DROP FUNCTION reject_adoption_commit()").await.unwrap();
    assert!(
        s.adopt_legacy(&e, input, "failed-adopt", Uuid::new_v4(), &budget(&f))
            .await
            .is_ok()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_migration_adoption_http_requires_current_admin_csrf_exact_operation_and_key() {
    let f = IdentityFixture::start().await;
    let (client, app) = legacy(&f, "http-history", true, false).await;
    let url = format!("{}/api/v1/admin/identity/migrations", f.base_url);
    assert_eq!(
        f.client.get(&url).send().await.unwrap().status().as_u16(),
        401
    );
    let mut cookies = BrowserCookies::default();
    let login = http_login(&f, &mut cookies).await;
    let list = f
        .client
        .get(&url)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(list.status().as_u16(), 200);
    assert_eq!(list.headers()["cache-control"], "no-store");
    let input = adoption(
        &f,
        client,
        app,
        "http-history",
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
    )
    .await;
    let target = format!("{url}/{client}/adopt");
    let invoke = |key: bool| {
        let mut r = f
            .client
            .post(&target)
            .header("cookie", cookies.header())
            .header("origin", &f.base_url)
            .header("x-csrf-token", &login)
            .json(&input);
        if key {
            r = r.header("idempotency-key", "http-adoption");
        }
        r
    };
    assert_eq!(invoke(false).send().await.unwrap().status().as_u16(), 400);
    f.db.execute_unprepared("INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT u.id,a.id,FALSE,NOW(),NOW() FROM \"user\" u CROSS JOIN authority a WHERE u.username='identity-admin' AND a.method='POST' AND a.authority_uri='/admin/identity/migrations/{id}/adopt'").await.unwrap();
    assert_eq!(invoke(true).send().await.unwrap().status().as_u16(), 403);
    f.db.execute_unprepared("DELETE FROM user_authority WHERE authority_id IN (SELECT id FROM authority WHERE authority_uri='/admin/identity/migrations/{id}/adopt')").await.unwrap();
    let response = invoke(true).send().await.unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert!(status.is_success(), "{body}");
    let first: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(first["newly_issued_secrets"].as_array().unwrap().len(), 1);
    let response = invoke(true)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(response["operation_id"], first["operation_id"]);
    assert!(
        response["newly_issued_secrets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_migration_adoption_bootstrap_does_not_create_or_expand_trust() {
    use cobotusr::config::BootstrapClient;
    let f = IdentityFixture::start_configured(|config| {
        config.oidc.bootstrap_clients.push(BootstrapClient {
            client_id: "bootstrap-new".into(),
            name: "Configured".into(),
            redirect_uris: vec![CALLBACK.into()],
            allowed_scopes: vec!["openid".into()],
            grant_types: vec!["authorization_code".into()],
            client_secret: Some(SECRET.into()),
            public_client: false,
        })
    })
    .await;
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM oauth_client").await,
        0
    );
    let (client, _) = legacy(&f, "bootstrap-existing", false, false).await;
    let old_hash = hash(&f, client).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut config = f.config.clone();
    config.http.listen = address;
    config.oidc.bootstrap_clients.push(BootstrapClient {
        client_id: "bootstrap-existing".into(),
        name: "Must remain historical".into(),
        redirect_uris: vec!["https://unapproved.example/callback".into()],
        allowed_scopes: vec!["email".into()],
        grant_types: vec!["authorization_code".into()],
        client_secret: None,
        public_client: true,
    });
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let clock = f.clock.clone();
    let server = tokio::spawn(cobotusr::serve_with_security_clock(
        config,
        listener,
        async {
            let _ = stopped.await;
        },
        clock,
    ));
    let ready = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            assert!(!server.is_finished());
            if f.client
                .get(format!("http://{address}/.well-known/openid-configuration"))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(ready.is_ok());
    assert_eq!(hash(&f, client).await, old_hash);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE client_id='bootstrap-existing' AND NOT public_client AND redirect_uris='https://rp.example/callback'").await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE client_id='bootstrap-new'"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM application").await,
        0
    );
    stop.send(()).unwrap();
    server.await.unwrap();
    f.close().await;
}

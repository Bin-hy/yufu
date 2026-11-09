use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_login},
    identity_refresh::issued,
    identity_support::*,
    identity_token::protocol,
};
use cobotusr::{
    domain::identity::{
        account::{PersonKind, SecretDigest},
        error::IdentityError,
    },
    service::identity::{console::ConsoleService, repository::*},
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn service(f: &IdentityFixture) -> ConsoleService {
    ConsoleService::new(Arc::new(repository(f)), f.clock.clone())
}
fn safe(value: &Value) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                assert!(
                    ![
                        "secret",
                        "secret_digest",
                        "password",
                        "nonce",
                        "protocol_sid",
                        "token_jti",
                        "endpoint_snapshot",
                        "lease_owner",
                        "token",
                        "refresh_token",
                        "authorization"
                    ]
                    .contains(&key.as_str()),
                    "unsafe key: {key}"
                );
                safe(v);
            }
        }
        Value::Array(items) => {
            for v in items {
                safe(v)
            }
        }
        _ => {}
    }
}
async fn get(f: &IdentityFixture, cookies: &BrowserCookies, path: &str) -> reqwest::Response {
    f.client
        .get(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
}
async fn fault(f: &IdentityFixture, on: bool) {
    if on {
        f.db.execute_unprepared("CREATE FUNCTION console_commit_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END $$").await.unwrap();
        f.db.execute_unprepared("CREATE CONSTRAINT TRIGGER console_commit_failure AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION console_commit_failure()").await.unwrap();
    } else {
        f.db.execute_unprepared("DROP TRIGGER console_commit_failure ON identity_audit_event")
            .await
            .unwrap();
        f.db.execute_unprepared("DROP FUNCTION console_commit_failure()")
            .await
            .unwrap();
    }
}
#[tokio::test]
async fn identity_console_http_lists_only_own_metadata_and_requires_exact_admin_grants() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "console-list", false).await;
    let mut cookies = BrowserCookies::default();
    http_login(&f, &mut cookies).await;
    let root = cookies.0.get("cobotusr-dev-sso").unwrap().clone();
    issued(&f, "console-list-dev", &root, "openid profile").await;
    for path in [
        "/account/apps",
        "/account/consents",
        "/account/sessions",
        "/account/rp-sessions",
        "/admin/accounts",
        "/admin/audit-events",
        "/admin/logout-deliveries",
    ] {
        let response = get(&f, &cookies, path).await;
        assert_eq!(response.status(), 200, "{path}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        safe(&response.json::<Value>().await.unwrap());
        assert_eq!(
            f.client
                .get(format!("{}/api/v1{path}", f.base_url))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    let apps = get(&f, &cookies, "/account/apps")
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(apps["items"].as_array().unwrap().len(), 1);
    assert_eq!(apps["items"][0]["application_id"], a.app);
    let root_digest = SecretDigest::of(&root).hex();
    let roots = get(&f, &cookies, "/account/sessions")
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert!(!roots.to_string().contains(&root_digest));
    assert_eq!(roots["items"][0]["current"], true);
    for path in [
        "/account/apps?user_id=999",
        "/account/consents?after=-1",
        "/account/sessions?after=bad",
        "/admin/accounts?limit=0",
        "/admin/logout-deliveries?limit=101",
    ] {
        assert_eq!(get(&f, &cookies, path).await.status(), 400, "{path}");
    }
    let repo = repository(&f);
    let mut tx = repo.begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(ordinary) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "ordinary-console".into(),
            nickname: "Ordinary".into(),
            password_hash: auth::security::identity::SecretToken::new(
                bcrypt::hash("fixture-password", 4).unwrap(),
            )
            .unwrap(),
            person_kind: PersonKind::Collaborator,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account")
    };
    tx.commit().await.unwrap();
    let (_, other_secret) = center_secret(&f, ordinary.user_id).await;
    let mut other = BrowserCookies::default();
    other.0.insert("cobotusr-dev-sso".into(), other_secret);
    assert_eq!(
        get(&f, &other, "/account/apps")
            .await
            .json::<Value>()
            .await
            .unwrap()["items"],
        json!([])
    );
    assert_eq!(
        get(&f, &other, "/account/consents")
            .await
            .json::<Value>()
            .await
            .unwrap()["items"],
        json!([])
    );
    for path in [
        "/admin/accounts",
        "/admin/audit-events",
        "/admin/logout-deliveries",
    ] {
        assert_eq!(get(&f, &other, path).await.status(), 403);
    }
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_authority(user_id,authority_id,allow,create_time,update_time) SELECT $1,id,FALSE,NOW(),NOW() FROM authority WHERE method='GET' AND authority_uri='/admin/accounts'",[a.owner.into()])).await.unwrap();
    assert_eq!(get(&f, &cookies, "/admin/accounts").await.status(), 403);
    assert_eq!(get(&f, &cookies, "/admin/audit-events").await.status(), 200);
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET enabled=FALSE,auth_epoch=auth_epoch+1 WHERE id=$1",
        [ordinary.user_id.into()],
    ))
    .await
    .unwrap();
    assert_eq!(get(&f, &other, "/account/sessions").await.status(), 401);
    f.close().await;
}
#[tokio::test]
async fn identity_console_consent_withdrawal_is_atomic_and_preserves_other_application_and_center()
{
    let f = IdentityFixture::start().await;
    let a = app(&f, "console-withdraw", true).await;
    app(&f, "console-keep", false).await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    let secret = cookies.0.get("cobotusr-dev-sso").unwrap();
    let one = issued(&f, "console-withdraw-dev", secret, "openid profile").await;
    let other = issued(&f, "console-keep-dev", secret, "openid").await;
    let consents = get(&f, &cookies, "/account/consents")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let consent = consents["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["application_id"] == a.app)
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let path = format!("/account/consents/{consent}/revoke");
    let response = f
        .client
        .post(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    fault(&f, true).await;
    let response = f
        .client
        .post(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM oauth_consent").await,
        2
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE revoked_at IS NOT NULL").await,0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        0
    );
    fault(&f, false).await;
    let response = f
        .client
        .post(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"revoked":true})
    );
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM oauth_consent").await,
        1
    );
    let svc = protocol(&f, f.signer.clone());
    assert!(
        svc.userinfo(&one.access_token, f.signer.as_ref(), &budget(&f))
            .await
            .is_err()
    );
    assert!(
        svc.userinfo(&other.access_token, f.signer.as_ref(), &budget(&f))
            .await
            .is_ok()
    );
    assert_eq!(get(&f, &cookies, "/account/session").await.status(), 200);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        1
    );
    assert_eq!(
        service(&f)
            .withdraw_consent(
                &write_evidence(&f, Some(secret), &"A".repeat(43)),
                consent as i32,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await,
        Err(IdentityError::NotFound)
    );
    f.close().await;
}
#[tokio::test]
async fn identity_console_delivery_retry_retains_target_jti_and_respects_lease_horizon_and_commit()
{
    let f = IdentityFixture::start().await;
    let a = app(&f, "console-retry", true).await;
    let (_, secret) = center_secret(&f, a.owner).await;
    issued(&f, "console-retry-dev", &secret, "openid").await;
    let evidence = write_evidence(&f, Some(&secret), &"A".repeat(43));
    let svc = service(&f);
    let id = scalar(&f, "SELECT id::bigint AS value FROM oauth_consent").await as i32;
    svc.withdraw_consent(&evidence, id, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let row =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id,token_jti,endpoint_snapshot FROM identity_logout_delivery".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let delivery: Uuid = row.try_get("", "id").unwrap();
    let jti: Uuid = row.try_get("", "token_jti").unwrap();
    let endpoint: String = row.try_get("", "endpoint_snapshot").unwrap();
    f.db.execute_unprepared("UPDATE identity_logout_delivery SET status='permanent_failure',last_error_code='http_4xx',attempt_count=3").await.unwrap();
    fault(&f, true).await;
    assert!(
        svc.retry_delivery(&evidence, delivery, Uuid::new_v4(), 86_400, &budget(&f))
            .await
            .is_err()
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE status='permanent_failure'").await,1);
    fault(&f, false).await;
    svc.retry_delivery(&evidence, delivery, Uuid::new_v4(), 86_400, &budget(&f))
        .await
        .unwrap();
    let row =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT token_jti,endpoint_snapshot,attempt_count FROM identity_logout_delivery"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<Uuid>("", "token_jti").unwrap(), jti);
    assert_eq!(
        row.try_get::<String>("", "endpoint_snapshot").unwrap(),
        endpoint
    );
    assert_eq!(row.try_get::<i32>("", "attempt_count").unwrap(), 3);
    f.db.execute_unprepared("UPDATE identity_logout_delivery SET status='leased',lease_owner='00000000-0000-0000-0000-000000000001',lease_until=next_attempt_at+10000").await.unwrap();
    assert_eq!(
        svc.retry_delivery(&evidence, delivery, Uuid::new_v4(), 86_400, &budget(&f))
            .await,
        Err(IdentityError::Conflict)
    );
    f.db.execute_unprepared(
        "UPDATE identity_logout_delivery SET status='delivered',delivered_at=next_attempt_at,lease_owner=NULL,lease_until=NULL",
    )
    .await
    .unwrap();
    assert_eq!(
        svc.retry_delivery(&evidence, delivery, Uuid::new_v4(), 86_400, &budget(&f))
            .await,
        Err(IdentityError::AlreadyConsumed)
    );
    f.db.execute_unprepared("UPDATE identity_logout_delivery SET status='permanent_failure',delivered_at=NULL,created_at=created_at-86400000").await.unwrap();
    assert_eq!(
        svc.retry_delivery(&evidence, delivery, Uuid::new_v4(), 86_400, &budget(&f))
            .await,
        Err(IdentityError::Expired)
    );
    assert_eq!(
        svc.retry_delivery(
            &evidence,
            Uuid::new_v4(),
            Uuid::new_v4(),
            86_400,
            &budget(&f)
        )
        .await,
        Err(IdentityError::NotFound)
    );
    f.close().await;
}

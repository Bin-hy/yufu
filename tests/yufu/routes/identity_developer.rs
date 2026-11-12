//! Browser-facing application and reader management with real primary transactions.
use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_csrf, http_login},
    identity_support::*,
};
use auth::security::identity::SecretToken;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};
use uuid::Uuid;
use yufu::{
    domain::identity::{
        account::PersonKind,
        application::{AppEnvironmentKind, ClientKind},
        registration::CreateApplication,
    },
    service::identity::repository::*,
};

fn request(key: &str, kind: ClientKind) -> CreateApplication {
    CreateApplication {
        app_key: key.into(),
        name: "Developer application".into(),
        description: None,
        third_party: false,
        development: config(AppEnvironmentKind::Development, kind, false).to_input(),
        production: config(AppEnvironmentKind::Production, kind, false).to_input(),
    }
}
fn write(
    f: &IdentityFixture,
    cookies: &BrowserCookies,
    csrf: &str,
    path: &str,
) -> reqwest::RequestBuilder {
    f.client
        .post(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
}
async fn enable_developer(f: &IdentityFixture) {
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET developer_enabled=true WHERE id=$1",
        [admin_id(f).await.into()],
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn identity_developer_http_configuration_submission_review_and_lifecycle_use_exact_permissions()
 {
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    enable_developer(&f).await;
    let registered: Value = write(&f, &cookies, &csrf, "/developer/apps")
        .header("idempotency-key", "governance-http-register")
        .json(&request("governance-http", ClientKind::ServerWeb))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let app = registered["application_id"].as_i64().unwrap();
    let prod = registered["environments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "production")
        .unwrap()["environment_id"]
        .as_str()
        .unwrap();
    let config_path = format!("/developer/apps/{app}/environments/{prod}/config");
    let saved=f.client.put(format!("{}/api/v1{config_path}",f.base_url)).header("cookie",cookies.header()).header("origin",&f.base_url).header("x-csrf-token",&csrf)
        .header("idempotency-key","governance-http-draft").json(&json!({"expected_revision":0,"config":config(AppEnvironmentKind::Production,ClientKind::BrowserSpa,false).to_input()})).send().await.unwrap();
    assert_eq!(saved.status(), 200);
    assert!(!saved.headers().contains_key("set-cookie"));
    let saved: Value = saved.json().await.unwrap();
    let version = &saved["summary"]["version_id"];
    let submitted = write(
        &f,
        &cookies,
        &csrf,
        &format!("/developer/apps/{app}/environments/{prod}/submit"),
    )
    .header("idempotency-key", "governance-http-submit")
    .json(&json!({"expected_revision":1,"version_id":version}))
    .send()
    .await
    .unwrap();
    assert_eq!(submitted.status(), 200);
    let submitted: Value = submitted.json().await.unwrap();
    let approval = submitted["summary"]["approval_id"].as_str().unwrap();
    let review_path = format!("/admin/apps/{app}/approvals/{approval}/review");
    let self_review=write(&f,&cookies,&csrf,&review_path).header("idempotency-key","governance-http-self-review")
        .json(&json!({"expected_revision":2,"approve":true,"reason":null,"accept_logout_limitation":false})).send().await.unwrap();
    assert_eq!(self_review.status(), 403);
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(peer) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "http-reviewer".into(),
            nickname: "Reviewer".into(),
            password_hash: SecretToken::new("fixture-unused-password".into()).unwrap(),
            person_kind: PersonKind::Employee,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"UPDATE \"user\" SET role_id=(SELECT role_id FROM \"user\" WHERE username='identity-admin') WHERE id=$1",[peer.user_id.into()])).await.unwrap();
    let (_, secret) = center_secret(&f, peer.user_id).await;
    let mut reviewer = BrowserCookies::default();
    reviewer.0.insert("yufu-dev-sso".into(), secret);
    let reviewer_csrf = http_csrf(&f, &mut reviewer).await;
    let detail = f
        .client
        .get(format!(
            "{}/api/v1/admin/apps/{app}/approvals/{approval}",
            f.base_url
        ))
        .header("cookie", reviewer.header())
        .send()
        .await
        .unwrap();
    assert_eq!(detail.status(), 200);
    assert_safe_metadata(&detail.json::<Value>().await.unwrap());
    let approved=write(&f,&reviewer,&reviewer_csrf,&review_path).header("idempotency-key","governance-http-review")
        .json(&json!({"expected_revision":2,"approve":true,"reason":"reviewed","accept_logout_limitation":false})).send().await.unwrap();
    assert_eq!(approved.status(), 200);
    let approved: Value = approved.json().await.unwrap();
    assert_eq!(approved["summary"]["revision"], 3);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    // An explicit per-user denial wins over the reviewer's superadmin role.
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,false,NOW(),NOW() FROM authority WHERE method='PATCH' AND authority_uri='/admin/apps/{id}/lifecycle'",[peer.user_id.into()])).await.unwrap();
    let lifecycle = format!("/admin/apps/{app}/lifecycle");
    let denied = f
        .client
        .patch(format!("{}/api/v1{lifecycle}", f.base_url))
        .header("cookie", reviewer.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &reviewer_csrf)
        .header("idempotency-key", "denied-lifecycle")
        .json(&json!({"expected_revision":0,"active":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    let suspended = f
        .client
        .patch(format!("{}/api/v1{lifecycle}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", "governance-http-suspend")
        .json(&json!({"expected_revision":0,"active":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(suspended.status(), 200);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_session_client WHERE enabled"
        )
        .await,
        0
    );
    let missing = f
        .client
        .patch(format!("{}/api/v1{lifecycle}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("idempotency-key", Uuid::new_v4().to_string())
        .json(&json!({"expected_revision":1,"active":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 403);
    f.close().await;
}
fn assert_safe_metadata(value: &Value) {
    let text = value.to_string();
    for key in [
        "client_secret",
        "secret_hash",
        "password",
        "browser_proof",
        "token",
    ] {
        assert!(!text.contains(key), "{key}");
    }
}

#[tokio::test]
async fn identity_developer_http_register_requires_csrf_key_and_qualification_and_reads_never_reissue_secrets()
 {
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    let input = request("http-register", ClientKind::ServerWeb);
    let denied = write(&f, &cookies, &csrf, "/developer/apps")
        .header("idempotency-key", "http-create")
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    enable_developer(&f).await;
    let missing = write(&f, &cookies, &csrf, "/developer/apps")
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 400);
    let wrong_csrf = write(&f, &cookies, "wrong", "/developer/apps")
        .header("idempotency-key", "http-create")
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_csrf.status(), 403);
    let mut injection = serde_json::to_value(&input).unwrap();
    injection["owner_user_id"] = json!(1);
    let injected = write(&f, &cookies, &csrf, "/developer/apps")
        .header("idempotency-key", "http-create")
        .json(&injection)
        .send()
        .await
        .unwrap();
    assert_eq!(injected.status(), 400);
    let response = write(&f, &cookies, &csrf, "/developer/apps")
        .header("idempotency-key", "http-create")
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(!response.headers().contains_key("set-cookie"));
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-origin")
    );
    let first: Value = response.json().await.unwrap();
    assert_eq!(first["secrets"].as_array().unwrap().len(), 2);
    let app = first["application_id"].as_i64().unwrap();
    let operation = first["operation_id"].as_str().unwrap();
    let repeated = write(&f, &cookies, &csrf, "/developer/apps")
        .header("idempotency-key", "http-create")
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(repeated.status(), 200);
    let repeated: Value = repeated.json().await.unwrap();
    assert_eq!(repeated["secrets"], json!([]));
    assert_eq!(repeated["operation_id"], first["operation_id"]);
    for path in [
        format!("/developer/apps/{app}"),
        "/developer/apps?limit=1".into(),
        format!("/developer/operations/{operation}"),
    ] {
        let response = f
            .client
            .get(format!("{}/api/v1{path}", f.base_url))
            .header("cookie", cookies.header())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("set-cookie"));
        assert_safe_metadata(&response.json().await.unwrap());
    }
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(outsider) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "developer-outsider".into(),
            nickname: "Outside".into(),
            password_hash: SecretToken::new("unused-fixture-hash".into()).unwrap(),
            person_kind: PersonKind::Collaborator,
            developer_enabled: true,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    let (_, secret) = center_secret(&f, outsider.user_id).await;
    let mut outside = BrowserCookies::default();
    outside.0.insert("yufu-dev-sso".into(), secret);
    for (path, expected) in [
        (format!("/developer/apps/{app}"), 403),
        (format!("/developer/operations/{operation}"), 404),
    ] {
        let response = f
            .client
            .get(format!("{}/api/v1{path}", f.base_url))
            .header("cookie", outside.header())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let list: Value = f
        .client
        .get(format!("{}/api/v1/developer/apps", f.base_url))
        .header("cookie", outside.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["items"], json!([]));
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    let anonymous = f
        .client
        .get(format!("{}/api/v1/developer/apps/{app}", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    f.close().await;
}

#[tokio::test]
async fn identity_developer_http_reader_requires_exact_application_environment_and_revision_and_lists_no_hash()
 {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "reader-management", false, ClientKind::BrowserSpa).await;
    let b = app_kind(&f, "reader-management-other", false, ClientKind::BrowserSpa).await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    let path = format!(
        "/developer/apps/{}/environments/{}/integration/readers",
        a.app, a.development
    );
    let wrong_path = format!(
        "/developer/apps/{}/environments/{}/integration/readers",
        b.app, a.development
    );
    let invalid = write(&f, &cookies, &csrf, &wrong_path)
        .header("idempotency-key", "reader-invalid-path")
        .json(&json!({"audience":"urn:test:Development"}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 403);
    let invalid = write(&f, &cookies, &csrf, &path)
        .header("idempotency-key", "reader-http-0")
        .json(&json!({"audience":"urn:unapproved"}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 403);
    let response = write(&f, &cookies, &csrf, &path)
        .header("idempotency-key", "reader-http-1")
        .json(&json!({"audience":"urn:test:Development"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(!response.headers().contains_key("set-cookie"));
    let created: Value = response.json().await.unwrap();
    assert_eq!(created["secret"].as_str().unwrap().len(), 43);
    let reader = created["reader_id"].as_str().unwrap();
    let item_path = format!("{path}/{reader}");
    let list = f
        .client
        .get(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let list: Value = list.json().await.unwrap();
    assert_safe_metadata(&list);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let list = f
        .client
        .get(format!("{}/api/v1{wrong_path}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 403);
    let reset = f
        .client
        .patch(format!("{}/api/v1{item_path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", uuid::Uuid::new_v4().to_string())
        .json(&json!({"expected_revision":1,"enabled":true,"reset_secret":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(reset.status(), 200);
    let reset: Value = reset.json().await.unwrap();
    assert_ne!(reset["secret"], created["secret"]);
    assert_eq!(reset["revision"], 2);
    let stale = f
        .client
        .patch(format!("{}/api/v1{item_path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", uuid::Uuid::new_v4().to_string())
        .json(&json!({"expected_revision":1,"enabled":false,"reset_secret":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 409);
    let stopped = f
        .client
        .patch(format!("{}/api/v1{item_path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", uuid::Uuid::new_v4().to_string())
        .json(&json!({"expected_revision":2,"enabled":false,"reset_secret":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(stopped.status(), 200);
    assert!(
        stopped
            .json::<Value>()
            .await
            .unwrap()
            .get("secret")
            .is_none()
    );
    let mut stranger = BrowserCookies::default();
    let stranger_csrf = http_csrf(&f, &mut stranger).await;
    let denied = write(&f, &stranger, &stranger_csrf, &path)
        .header("idempotency-key", "reader-stranger")
        .json(&json!({"audience":"urn:test:Development"}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_introspection_reader"
        )
        .await,
        1
    );
    f.close().await;
}

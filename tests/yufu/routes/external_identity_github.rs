//! GitHub OAuth login/bind through real HTTP, with a local fake provider.
use super::super::common::IdentityFixture;
use super::identity_browser::{BrowserCookies, http_csrf, http_login};
use super::identity_support::{admin_id, app, scalar};
use axum::{
    Json, Router,
    extract::Form,
    http::HeaderMap,
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};

const KEY: [u8; 32] = [9; 32];

async fn fake_github() -> u16 {
    async fn token(Form(form): Form<std::collections::HashMap<String, String>>) -> Json<Value> {
        Json(json!({
            "access_token": form.get("code").cloned().unwrap_or_default(),
            "token_type": "bearer",
            "scope": "read:user user:email"
        }))
    }
    async fn user(headers: HeaderMap) -> Json<Value> {
        let token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let id = if token == "Bearer unbound" { 99 } else { 42 };
        Json(json!({
            "id": id,
            "login": if id == 42 { "octocat" } else { "unbound" },
            "name": if id == 42 { "Octo Cat" } else { "Unbound" },
            "avatar_url": "https://avatars.example/u/42",
            "email": null
        }))
    }
    async fn emails() -> Json<Value> {
        Json(json!([
            {"email":"old@example.test","primary":false,"verified":true},
            {"email":"primary@example.test","primary":true,"verified":true}
        ]))
    }
    let app = Router::new()
        .route("/token", post(token))
        .route("/user", get(user))
        .route("/emails", get(emails));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

async fn fixture_with_github(port: u16) -> IdentityFixture {
    let key = URL_SAFE_NO_PAD.encode(KEY);
    unsafe {
        std::env::set_var("YUFU_EXTERNAL_SECRET_KEY", key);
    }
    let cipher = yufu::service::identity::external::SecretCipher::from_key(KEY, 1).unwrap();
    let ciphertext = cipher.encrypt("github", "client-secret").unwrap();
    IdentityFixture::start_configured_with(
        |config| {
            config.external_identity.enabled = true;
        },
        move |db| async move {
            let config = json!({
                "authorize_endpoint": format!("http://127.0.0.1:{port}/authorize"),
                "token_endpoint": format!("http://127.0.0.1:{port}/token"),
                "user_endpoint": format!("http://127.0.0.1:{port}/user"),
                "emails_endpoint": format!("http://127.0.0.1:{port}/emails")
            });
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE identity_external_provider SET enabled=TRUE,client_id=$1,secret_ciphertext=$2,config=$3::jsonb,revision=1,updated_at=$4 WHERE provider_key='github'",
                [
                    "client-id".into(),
                    ciphertext.into(),
                    config.into(),
                    1_i64.into(),
                ],
            ))
            .await
            .unwrap();
        },
    )
    .await
}

async fn start_flow(f: &IdentityFixture, cookies: &mut BrowserCookies, path: &str) -> String {
    let csrf = http_csrf(f, cookies).await;
    let response = f
        .client
        .post(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let value = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    assert_eq!(status, 200, "start failed: {value}");
    let url = reqwest::Url::parse(value["authorize_url"].as_str().unwrap()).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    assert!(url.query_pairs().any(|(key, _)| key == "code_challenge"));
    state
}

async fn callback(
    f: &IdentityFixture,
    cookies: &mut BrowserCookies,
    code: &str,
    state: &str,
) -> reqwest::Response {
    let response = f
        .client
        .get(format!("{}/identity/external/github/callback", f.base_url))
        .query(&[
            ("code", code),
            ("state", state),
            ("iss", "https://github.com/login/oauth"),
        ])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    cookies.absorb(response.headers());
    response
}

#[tokio::test]
async fn github_login_creates_center_session_and_refuses_unbound_identity() {
    let port = fake_github().await;
    let f = fixture_with_github(port).await;
    let admin = admin_id(&f).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_external_identity (id,user_id,provider_key,provider_subject,created_at,updated_at) VALUES ($1,$2,'github','42',$3,$3)",
        [
            uuid::Uuid::new_v4().into(),
            admin.into(),
            1_i64.into(),
        ],
    ))
    .await
    .unwrap();
    let mut cookies = BrowserCookies::default();

    let state = start_flow(&f, &mut cookies, "/identity/external/github/start").await;
    let response = callback(&f, &mut cookies, "bound", &state).await;
    assert_eq!(response.status(), 303);
    assert_eq!(response.headers()[reqwest::header::LOCATION], "/account");
    assert!(
        cookies.header().contains("yufu-dev-sso="),
        "login must set the center cookie"
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_external_identity"
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='external.login' AND result='success'"
        )
        .await,
        1
    );

    let response = f
        .client
        .get(format!("{}/api/v1/account/identities", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let identities = response.json::<Value>().await.unwrap();
    assert_eq!(identities.as_array().unwrap().len(), 1);
    assert_eq!(identities[0]["provider_key"], "github");
    assert_eq!(identities[0]["provider_label"], "GitHub");
    assert_ne!(identities[0]["subject_hint"], "42");

    let state = start_flow(&f, &mut cookies, "/identity/external/github/start").await;
    let response = callback(&f, &mut cookies, "unbound", &state).await;
    assert_eq!(response.status(), 303);
    assert!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap()
            .contains("external_identity_not_linked")
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_external_identity"
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='external.login' AND result='denied'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn github_login_continues_existing_oidc_flow() {
    let port = fake_github().await;
    let f = fixture_with_github(port).await;
    let _ = app(&f, "ext-oidc", true).await;
    let admin = admin_id(&f).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_external_identity (id,user_id,provider_key,provider_subject,created_at,updated_at) VALUES ($1,$2,'github','42',$3,$3)",
        [uuid::Uuid::new_v4().into(), admin.into(), 1_i64.into()],
    ))
    .await
    .unwrap();
    let mut cookies = BrowserCookies::default();

    let mut authorize = reqwest::Url::parse(&format!("{}/oidc/authorize", f.base_url)).unwrap();
    authorize.query_pairs_mut().extend_pairs([
        ("client_id", "ext-oidc-dev"),
        ("redirect_uri", "https://rp.example/callback"),
        ("response_type", "code"),
        ("scope", "openid profile"),
        ("state", "oidc-state"),
        ("nonce", "oidc-nonce"),
        (
            "code_challenge",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
        ("code_challenge_method", "S256"),
    ]);
    let response = {
        let mut request = f.client.get(authorize);
        if !cookies.header().is_empty() {
            request = request.header("cookie", cookies.header());
        }
        request.send().await.unwrap()
    };
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.unwrap();
    assert_eq!(status, 303, "authorize failed: {body}");
    cookies.absorb(&headers);
    let location = headers[reqwest::header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let flow_id = reqwest::Url::parse(&format!("{}{}", f.base_url, location))
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "flow_id")
        .unwrap()
        .1
        .into_owned();

    let csrf = http_csrf(&f, &mut cookies).await;
    let response = f
        .client
        .post(format!(
            "{}/api/v1/identity/external/github/start",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&json!({"flow_id": flow_id}))
        .send()
        .await
        .unwrap();
    let value = response.json::<Value>().await.unwrap();
    let state = reqwest::Url::parse(value["authorize_url"].as_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();

    let response = callback(&f, &mut cookies, "bound", &state).await;
    assert_eq!(response.status(), 303);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION],
        format!("/oidc/consent?flow_id={flow_id}")
    );

    let response = f
        .client
        .get(format!("{}/api/v1/identity/flows/{flow_id}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let summary = response.json::<Value>().await.unwrap();
    assert_eq!(summary["interaction"], "consent");

    let csrf = http_csrf(&f, &mut cookies).await;
    let response = f
        .client
        .post(format!(
            "{}/api/v1/identity/flows/{flow_id}/complete",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&json!({"approved": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let completed = response.json::<Value>().await.unwrap();
    assert!(
        completed["redirect_url"]
            .as_str()
            .unwrap()
            .starts_with("https://rp.example/callback?code=")
    );
}

#[tokio::test]
async fn github_bind_and_unbind_requires_center_session_and_keeps_password() {
    let port = fake_github().await;
    let f = fixture_with_github(port).await;
    let mut cookies = BrowserCookies::default();
    let _ = http_login(&f, &mut cookies).await;

    let state = start_flow(&f, &mut cookies, "/identity/external/github/bind/start").await;
    let response = callback(&f, &mut cookies, "bound", &state).await;
    assert_eq!(response.status(), 303);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION],
        "/account/identities?bound=github"
    );
    let admin = admin_id(&f).await;
    assert_eq!(
        scalar(
            &f,
            "SELECT user_id::bigint AS value FROM identity_external_identity WHERE provider_key='github'"
        )
        .await,
        i64::from(admin)
    );

    let response = f
        .client
        .get(format!("{}/api/v1/account/identities", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    let identities = response.json::<Value>().await.unwrap();
    let id = identities[0]["id"].as_str().unwrap().to_owned();

    let csrf = http_csrf(&f, &mut cookies).await;
    let response = f
        .client
        .post(format!(
            "{}/api/v1/account/identities/{id}/unbind",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_external_identity"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='external.unbind'"
        )
        .await,
        1
    );
}

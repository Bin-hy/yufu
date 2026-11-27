//! QQ OAuth login through real HTTP, with a local JSONP fake provider.
use super::super::common::IdentityFixture;
use super::identity_browser::{BrowserCookies, http_csrf, http_login};
use super::identity_support::scalar;
use axum::{Router, extract::Query, response::IntoResponse, routing::get};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};
use std::collections::HashMap;

const KEY: [u8; 32] = [9; 32];

async fn fake_qq() -> u16 {
    async fn token(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
        let code = query.get("code").cloned().unwrap_or_default();
        if code == "bad" {
            "callback( {\"error\":100019} );".to_owned()
        } else {
            format!("callback( {{\"access_token\":\"{code}\"}} );")
        }
    }
    async fn me(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
        let token = query.get("access_token").cloned().unwrap_or_default();
        format!("callback( {{\"client_id\":\"qq-appid\",\"openid\":\"{token}\"}} );")
    }
    async fn user() -> impl IntoResponse {
        serde_json::json!({"ret":0,"nickname":"QQ用户","figureurl_qq_2":"https://qlogo.example/a.png"}).to_string()
    }
    let app = Router::new()
        .route("/token", get(token))
        .route("/me", get(me))
        .route("/user", get(user));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

async fn fixture_with_qq(port: u16) -> IdentityFixture {
    unsafe {
        std::env::set_var("YUFU_EXTERNAL_SECRET_KEY", URL_SAFE_NO_PAD.encode(KEY));
    }
    let cipher = yufu::service::identity::external::SecretCipher::from_key(KEY, 1).unwrap();
    let ciphertext = cipher.encrypt("qq", "qq-appkey").unwrap();
    IdentityFixture::start_configured_with(
        |config| {
            config.external_identity.enabled = true;
        },
        move |db| async move {
            let config = json!({
                "authorize_endpoint": format!("http://127.0.0.1:{port}/authorize"),
                "token_endpoint": format!("http://127.0.0.1:{port}/token"),
                "me_endpoint": format!("http://127.0.0.1:{port}/me"),
                "user_endpoint": format!("http://127.0.0.1:{port}/user")
            });
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE identity_external_provider SET enabled=TRUE,client_id=$1,secret_ciphertext=$2,config=$3::jsonb,revision=1,updated_at=$4 WHERE provider_key='qq'",
                [
                    "qq-appid".into(),
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

async fn start_flow(f: &IdentityFixture, cookies: &mut BrowserCookies) -> String {
    let csrf = http_csrf(f, cookies).await;
    let response = f
        .client
        .post(format!("{}/api/v1/identity/external/qq/start", f.base_url))
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
    assert!(!url.query_pairs().any(|(key, _)| key == "code_challenge"));
    url.query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned()
}

async fn callback(
    f: &IdentityFixture,
    cookies: &mut BrowserCookies,
    code: &str,
    state: &str,
) -> reqwest::Response {
    let response = f
        .client
        .get(format!("{}/identity/external/qq/callback", f.base_url))
        .query(&[("code", code), ("state", state)])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    cookies.absorb(response.headers());
    response
}

#[tokio::test]
async fn qq_login_unbound_and_upstream_error() {
    let port = fake_qq().await;
    let f = fixture_with_qq(port).await;
    let admin = super::identity_support::admin_id(&f).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_external_identity (id,user_id,provider_key,provider_subject,created_at,updated_at) VALUES ($1,$2,'qq','bound',$3,$3)",
        [uuid::Uuid::new_v4().into(), admin.into(), 1_i64.into()],
    ))
    .await
    .unwrap();
    let mut cookies = BrowserCookies::default();

    let state = start_flow(&f, &mut cookies).await;
    let response = callback(&f, &mut cookies, "bound", &state).await;
    assert_eq!(response.status(), 303);
    assert_eq!(response.headers()[reqwest::header::LOCATION], "/account");
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_external_identity WHERE provider_key='qq'"
        )
        .await,
        1
    );

    let state = start_flow(&f, &mut cookies).await;
    let response = callback(&f, &mut cookies, "unbound", &state).await;
    assert!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap()
            .contains("external_identity_not_linked")
    );

    let state = start_flow(&f, &mut cookies).await;
    let response = callback(&f, &mut cookies, "bad", &state).await;
    assert!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap()
            .contains("unauthenticated")
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='external.login' AND result='failure'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn qq_bind_and_unbind_from_center_session() {
    let port = fake_qq().await;
    let f = fixture_with_qq(port).await;
    let mut cookies = BrowserCookies::default();
    let _ = http_login(&f, &mut cookies).await;

    let state = {
        let csrf = http_csrf(&f, &mut cookies).await;
        let response = f
            .client
            .post(format!(
                "{}/api/v1/identity/external/qq/bind/start",
                f.base_url
            ))
            .header("cookie", cookies.header())
            .header("origin", &f.base_url)
            .header("x-csrf-token", csrf)
            .send()
            .await
            .unwrap();
        let value = response.json::<Value>().await.unwrap();
        reqwest::Url::parse(value["authorize_url"].as_str().unwrap())
            .unwrap()
            .query_pairs()
            .find(|(key, _)| key == "state")
            .unwrap()
            .1
            .into_owned()
    };
    let response = callback(&f, &mut cookies, "bound", &state).await;
    assert_eq!(response.status(), 303);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION],
        "/account/identities?bound=qq"
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
}

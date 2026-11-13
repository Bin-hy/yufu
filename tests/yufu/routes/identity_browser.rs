//! Real cross-site OP, Web RP, SPA and protected API browser scenarios.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use axum::http;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct BrowserCookies(pub(super) BTreeMap<String, String>);
impl BrowserCookies {
    pub(super) fn absorb(&mut self, headers: &http::HeaderMap) {
        for value in headers.get_all(http::header::SET_COOKIE) {
            let value = value.to_str().unwrap();
            let (name, secret) = value.split(';').next().unwrap().split_once('=').unwrap();
            if value.contains("Max-Age=0") {
                self.0.remove(name);
            } else {
                self.0.insert(name.into(), secret.into());
            }
        }
    }
    pub(super) fn header(&self) -> String {
        self.0
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub(super) async fn http_csrf(f: &IdentityFixture, cookies: &mut BrowserCookies) -> String {
    let mut request = f.client.get(format!("{}/api/v1/identity/csrf", f.base_url));
    if !cookies.0.is_empty() {
        request = request.header("cookie", cookies.header());
    }
    let response = request.send().await.unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned()
}

pub(super) async fn http_login(f: &IdentityFixture, cookies: &mut BrowserCookies) -> String {
    let csrf = http_csrf(f, cookies).await;
    let response = f
        .client
        .post(format!("{}/api/v1/identity/login", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({"username":"identity-admin","password":"fixture-admin-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn identity_browser_http_password_login_restore_csrf_reauthentication_and_logout() {
    use auth::security::credential::CredentialRef;
    use std::sync::Arc;
    use yufu::domain::identity::{account::SecretDigest, session::IdentityRevocationScope};
    use yufu::service::{
        identity::repository::IdentityRepository,
        security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let url = |path: &str| format!("{}/api/v1{path}", f.base_url);
    let response = f.client.get(url("/account/session")).send().await.unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(response.headers()[http::header::CACHE_CONTROL], "no-store");
    let response = f.client.get(url("/identity/csrf")).send().await.unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    let csrf = response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = f
        .client
        .post(url("/identity/login"))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({"username":"identity-admin","password":"wrong-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_sso_session"
        )
        .await,
        0
    );
    let response = f
        .client
        .post(url("/identity/login"))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .json(&serde_json::json!({"username":"identity-admin","password":"fixture-admin-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let sso_cookie = response
        .headers()
        .get_all(http::header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .find(|value| value.starts_with("yufu-dev-sso="))
        .unwrap();
    for attribute in ["HttpOnly", "Path=/", "SameSite=Lax", "Max-Age=43200"] {
        assert!(sso_cookie.contains(attribute));
    }
    assert!(!sso_cookie.contains("Domain="));
    cookies.absorb(response.headers());
    let login = response.json::<serde_json::Value>().await.unwrap();
    assert!(login.get("access_token").is_none());
    assert!(login.get("email").is_none());
    assert_eq!(login["username"], "identity-admin");
    let root_secret = cookies.0["yufu-dev-sso"].clone();
    let mut transaction = repository(&f).begin(&budget(&f)).await.unwrap();
    let root = transaction
        .load_browser_session(&SecretDigest::of(&root_secret))
        .await
        .unwrap()
        .unwrap();
    transaction.rollback().await.unwrap();
    let a = app(&f, "http-login-rp", true).await;
    let (tokens, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    let csrf = login["csrf_token"].as_str().unwrap();
    let one = f
        .client
        .get(url("/account/session"))
        .header("cookie", cookies.header())
        .send();
    let two = f
        .client
        .get(url("/account/session"))
        .header("cookie", cookies.header())
        .send();
    let (one, two) = tokio::join!(one, two);
    for response in [one.unwrap(), two.unwrap()] {
        assert_eq!(response.status(), 200);
        assert_eq!(
            response
                .headers()
                .get_all(http::header::SET_COOKIE)
                .iter()
                .count(),
            0
        );
        let restored = response.json::<serde_json::Value>().await.unwrap();
        for field in ["auth_time", "expires_at", "csrf_token", "sso_session_id"] {
            assert_eq!(restored[field], login[field]);
        }
    }
    let denied = f
        .client
        .post(url("/identity/logout"))
        .header("cookie", cookies.header())
        .header("origin", "https://evil.example")
        .header("x-csrf-token", csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    f.clock.advance(2000);
    let response = f
        .client
        .post(url("/identity/login"))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&serde_json::json!({"username":"identity-admin","password":"fixture-admin-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    let next = response.json::<serde_json::Value>().await.unwrap();
    assert_ne!(cookies.0["yufu-dev-sso"], root_secret);
    assert_eq!(
        next["auth_time"].as_u64().unwrap(),
        login["auth_time"].as_u64().unwrap() + 2
    );
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    assert!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&f.base_url, &tokens.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    let current = uuid::Uuid::parse_str(next["sso_session_id"].as_str().unwrap()).unwrap();
    let mut transaction = repository(&f).begin(&budget(&f)).await.unwrap();
    let plan = transaction
        .revocation_plan(&IdentityRevocationScope::CurrentCenter {
            sso_session_id: current,
        })
        .await
        .unwrap();
    assert_eq!(plan.user_ids.len(), 1);
    transaction.rollback().await.unwrap();
    let response = f
        .client
        .post(url("/identity/logout"))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", next["csrf_token"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    assert!(!cookies.0.contains_key("yufu-dev-sso"));
    assert_eq!(
        f.client
            .get(url("/account/session"))
            .header("cookie", cookies.header())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.close().await;
}

#[tokio::test]
async fn identity_browser_http_login_flow_rejects_wrong_proof_and_repeated_reauthentication() {
    use auth::security::budget::Clock;
    use yufu::domain::identity::{
        account::{IssuedSecret, SecretDigest},
        session::{FlowHandle, FlowPurpose},
    };
    use yufu::service::identity::repository::*;
    let f = IdentityFixture::start().await;
    let a = app(&f, "login-flow", false).await;
    let mut cookies = BrowserCookies::default();
    let response = f
        .client
        .get(format!("{}/api/v1/identity/csrf", f.base_url))
        .send()
        .await
        .unwrap();
    cookies.absorb(response.headers());
    let csrf = response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let flow_id = uuid::Uuid::new_v4();
    let proof = IssuedSecret::from_entropy(rand::random());
    let mut transaction = repository(&f).begin(&budget(&f)).await.unwrap();
    transaction
        .apply(IdentityWrite::CreateFlow {
            interactive: true,
            flow: FlowHandle::pending(
                flow_id,
                FlowPurpose::Authorization,
                proof.digest(),
                f.clock.now().unwrap().unix_millis + 600_000,
            )
            .unwrap(),
            environment_id: a.development,
            config_version_id: a.dev_version,
            browser_binding_digest: SecretDigest::of(&cookies.0["yufu-dev-browser"]),
            request_payload: serde_json::json!({"state":"original-state"}),
        })
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let name = yufu::service::identity::browser::flow_cookie_name(&f.config, flow_id);
    cookies.0.insert(
        name.clone(),
        IssuedSecret::from_entropy(rand::random()).into_response(),
    );
    let login = |cookies: &BrowserCookies, csrf: &str| {
        f.client.post(format!("{}/api/v1/identity/login",f.base_url)).header("cookie",cookies.header()).header("origin",&f.base_url).header("x-csrf-token",csrf).json(&serde_json::json!({"username":"identity-admin","password":"fixture-admin-password","flow_id":flow_id}))
    };
    assert_eq!(login(&cookies, &csrf).send().await.unwrap().status(), 403);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_sso_session"
        )
        .await,
        0
    );
    cookies.0.insert(name, proof.into_response());
    let response = login(&cookies, &csrf).send().await.unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    let logged_in = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(
        login(&cookies, logged_in["csrf_token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_sso_session"
        )
        .await,
        1
    );
    let mut transaction = repository(&f).begin(&budget(&f)).await.unwrap();
    let flow = transaction
        .load_authorization_flow(flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(flow.reauth_completed_at.is_some());
    assert_eq!(
        flow.bound_sso_session_id.unwrap().to_string(),
        logged_in["sso_session_id"].as_str().unwrap()
    );
    assert_eq!(flow.request_payload["state"], "original-state");
    transaction.rollback().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_browser_center_lifetime_is_independent_of_two_hour_legacy_jwt() {
    let f = IdentityFixture::start_configured(|config| config.auth.exp_seconds = 7200).await;
    let mut cookies = BrowserCookies::default();
    http_login(&f, &mut cookies).await;
    let url = format!("{}/api/v1/account/session", f.base_url);
    let initial: serde_json::Value = f
        .client
        .get(&url)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let expires = initial["expires_at"].as_u64().unwrap();
    let authenticated = initial["auth_time"].as_u64().unwrap();
    assert_eq!(expires - authenticated, 43_200);
    f.clock.advance(7_201_000);
    let response = f
        .client
        .get(&url)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let restored: serde_json::Value = response.json().await.unwrap();
    for field in ["sso_session_id", "auth_time", "expires_at"] {
        assert_eq!(restored[field], initial[field]);
    }
    f.clock.advance(35_999_000);
    let response = f
        .client
        .get(&url)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    f.close().await;
}

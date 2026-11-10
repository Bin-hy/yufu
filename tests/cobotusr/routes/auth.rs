use cobotusr::config::Config;
use cobotusr::error::AppError;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

use super::super::common::*;

#[path = "auth_revocation.rs"]
mod revocation;

use cobotusr::route::auth::{LoginData, RegistryData};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::sync::Arc;

struct LocalHttpFixture {
    _container: testcontainers::ContainerAsync<testcontainers::GenericImage>,
    _rsa: super::oidc::TestRsaFile,
    db: DatabaseConnection,
    root: String,
    issuer: String,
    insecure_cookies: bool,
    api: String,
    client: Client,
    clock: Arc<dyn auth::security::budget::Clock>,
    _monitor: Option<cobotusr::clock::ClockMonitor>,
    _ntp: Option<tokio::task::JoinHandle<()>>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for LocalHttpFixture {
    fn drop(&mut self) {
        self.server.abort();
        if let Some(task) = &self._ntp {
            task.abort();
        }
    }
}
async fn local_http_fixture(mode: &str) -> LocalHttpFixture {
    let (container, database_url) = get_postgres_container().await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let root = format!("http://{address}");
    let mut config = Config::default();
    config.database.url = database_url.clone();
    let rsa = configure_local_server(&mut config, address);
    let api = format!("{root}{}", config.http.api_prefix);
    config.http.listen = address;
    let mut monitor = None;
    let mut ntp = None;
    if mode == "monitored-loopback" {
        config.auth.allow_insecure_loopback_cookies = true;
        config.auth.super_admin.username = "isolated-test-admin".into();
        config.auth.super_admin.password = "isolated test password".into();
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        config.security_clock = cobotusr::config::SecurityClock {
            servers: vec![socket.local_addr().unwrap().to_string()],
            min_agreeing_sources: 1,
            poll_interval_seconds: 1,
            sample_ttl_seconds: 3,
            timeout_millis: 100,
        };
        ntp = Some(tokio::spawn(async move {
            let mut request = [0u8; 512];
            while let Ok((size, sender)) = socket.recv_from(&mut request).await {
                if size < 48 {
                    continue;
                }
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap();
                let stamp = (((time.as_secs() + 2_208_988_800) << 32)
                    | ((u64::from(time.subsec_nanos()) << 32) / 1_000_000_000))
                    .to_be_bytes();
                let mut response = [0u8; 48];
                response[0] = 0x24;
                response[1] = 1;
                response[24..32].copy_from_slice(&request[40..48]);
                response[32..40].copy_from_slice(&stamp);
                response[40..48].copy_from_slice(&stamp);
                if socket.send_to(&response, sender).await.is_err() {
                    break;
                }
            }
        }));
        monitor = Some(
            cobotusr::clock::ClockMonitor::start(&config.security_clock)
                .await
                .unwrap(),
        );
    }
    let clock = monitor
        .as_ref()
        .map(|monitor| monitor.clock())
        .unwrap_or_else(|| test_clock(mode != "unhealthy"));
    if mode == "hmac" {
        config.oidc.signing_algorithm = "HS256".into();
    }
    // HTTP is the internal test transport. Healthy production fixtures model
    // TLS termination with a distinct, fixed HTTPS issuer and Secure cookies.
    let insecure_cookies = matches!(mode, "monitored-loopback" | "hmac");
    config.auth.allow_insecure_loopback_cookies = insecure_cookies;
    if !insecure_cookies {
        config.oidc.issuer = format!("https://{address}");
    }
    let issuer = config.oidc.issuer.clone();
    let server = if mode == "missing-clock" {
        tokio::spawn(cobotusr::serve(config, listener, shutdown_signal()))
    } else {
        tokio::spawn(cobotusr::serve_with_security_clock(
            config,
            listener,
            shutdown_signal(),
            clock.clone(),
        ))
    };
    let client = Client::new();
    client
        .get(format!("{root}/.well-known/openid-configuration"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let db = sea_orm::Database::connect(database_url).await.unwrap();
    LocalHttpFixture {
        _container: container,
        _rsa: rsa,
        db,
        root,
        issuer,
        insecure_cookies,
        api,
        client,
        clock,
        _monitor: monitor,
        _ntp: ntp,
        server,
    }
}
fn response_refresh_cookie(response: &reqwest::Response) -> String {
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("Secure") && cookie.contains("SameSite=Lax")
    );
    assert!(cookie.contains("Max-Age=3600"));
    cookie.split(';').next().unwrap().into()
}
fn bearer_headers(access: &str) -> axum::http::HeaderMap {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", format!("Bearer {access}").parse().unwrap());
    headers
}

fn csrf_request(
    fixture: &LocalHttpFixture,
    request: reqwest::RequestBuilder,
    refresh_cookie: Option<&str>,
) -> reqwest::RequestBuilder {
    let refresh = refresh_cookie.and_then(|cookie| cookie.strip_prefix("refresh_token="));
    let clock = test_clock(true);
    let token = auth::security::csrf::CsrfProtector::new("t24-local-test-key".into())
        .unwrap()
        .issue(&fixture.issuer, refresh, clock.as_ref())
        .unwrap();
    let cookies = format!(
        "{}={}{}",
        if fixture.insecure_cookies {
            "cobotusr-dev-csrf"
        } else {
            "__Host-cobotusr-csrf"
        },
        token.expose_secret(),
        refresh_cookie
            .map(|cookie| format!("; {cookie}"))
            .unwrap_or_default()
    );
    request
        .header("origin", &fixture.issuer)
        .header("x-csrf-token", token.expose_secret())
        .header("cookie", cookies)
}
#[tokio::test]
async fn security_t24_http_register_login_refresh_and_replay_use_registered_idp_credentials() {
    use auth::security::{
        credential::{CredentialExpectation, CredentialProfile, StrictAccessState},
        identity::{PrincipalKind, TokenUse},
    };
    use cobotusr::service::security::{
        lifecycle::CredentialLifecycle, repository::PgSecurityRepository,
    };
    let fixture = local_http_fixture("healthy").await;
    let registration = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/register", fixture.api)),
        None,
    )
    .json(&RegistryData {
        username: "local-http".into(),
        nickname: "local-http-nick".into(),
        password: "test password".into(),
    })
    .send()
    .await
    .unwrap();
    assert_eq!(registration.status(), 200);
    let original_cookie = response_refresh_cookie(&registration);
    let registered: serde_json::Value = registration.json().await.unwrap();
    assert_eq!(registered["nickname"], "local-http-nick");
    let checker = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(
            fixture.db.clone(),
            fixture.clock.clone(),
        )),
        fixture.clock.clone(),
    ));
    let strict = StrictAccessState::new(
        Arc::new(
            auth::jwks::JwksClient::new(
                format!("{}/.well-known/jwks.json", fixture.root),
                Some("cobotusr".into()),
            )
            .await
            .unwrap(),
        ),
        CredentialExpectation::new(
            &fixture.issuer,
            "cobotusr",
            "cobotusr",
            vec![CredentialProfile::new(PrincipalKind::User, TokenUse::UserSession).unwrap()],
        )
        .unwrap(),
        fixture.clock.clone(),
        checker,
    );
    let original_headers = bearer_headers(registered["access_token"].as_str().unwrap());
    let context = strict.authenticate(&original_headers).await.unwrap();
    assert_eq!(context.audience(), "cobotusr");
    let wrong_login = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/login", fixture.api)),
        None,
    )
    .json(&LoginData {
        username: "local-http".into(),
        password: "wrong".into(),
    })
    .send()
    .await
    .unwrap();
    assert_eq!(wrong_login.status(), 401);
    assert!(!wrong_login.headers().contains_key("set-cookie"));
    let logged = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/login", fixture.api)),
        None,
    )
    .json(&LoginData {
        username: "local-http".into(),
        password: "test password".into(),
    })
    .send()
    .await
    .unwrap();
    assert_eq!(logged.status(), 200);
    let _other_cookie = response_refresh_cookie(&logged);
    let logged: serde_json::Value = logged.json().await.unwrap();
    let other_headers = bearer_headers(logged["access_token"].as_str().unwrap());
    strict.authenticate(&other_headers).await.unwrap();
    let refreshed = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/refresh", fixture.api)),
        Some(&original_cookie),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(refreshed.status(), 200);
    let rotated_cookie = response_refresh_cookie(&refreshed);
    assert_ne!(rotated_cookie, original_cookie);
    let refreshed: serde_json::Value = refreshed.json().await.unwrap();
    let rotated_headers = bearer_headers(refreshed["access_token"].as_str().unwrap());
    strict.authenticate(&rotated_headers).await.unwrap();
    assert!(strict.authenticate(&original_headers).await.is_err());
    let replay = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/refresh", fixture.api)),
        Some(&original_cookie),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(replay.status(), 401);
    assert!(!replay.headers().contains_key("set-cookie"));
    assert!(strict.authenticate(&rotated_headers).await.is_err());
    strict.authenticate(&other_headers).await.unwrap();
    let row = fixture.db.query_one_raw(Statement::from_string(DbBackend::Postgres,
        "SELECT (SELECT COUNT(*) FROM refresh_token) AS legacy, (SELECT COUNT(*) FROM oauth_token) AS oauth, (SELECT COUNT(*) FROM security_credential) AS issued, (SELECT MIN(expires_at-issued_at) FROM security_credential WHERE credential_kind='access') AS ttl".to_owned())).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "legacy").unwrap(), 0);
    assert_eq!(row.try_get::<i64>("", "oauth").unwrap(), 0);
    assert_eq!(row.try_get::<i64>("", "issued").unwrap(), 6);
    assert_eq!(row.try_get::<i64>("", "ttl").unwrap(), 300_000);
}
#[tokio::test]
async fn security_t24_http_legacy_refresh_missing_and_invalid_inputs_never_issue() {
    let fixture = local_http_fixture("healthy").await;
    let jti = "stored-legacy-refresh";
    cobotusr::entity::refresh_token::Entity::upsert_refresh_token(&fixture.db, jti.into(), 1)
        .await
        .unwrap();
    let legacy = auth::token_manager::TokenManager::new("t24-local-test-key", 3600)
        .generate_refresh_token::<i32>(jti.into(), 1)
        .unwrap();
    let response = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/refresh", fixture.api)),
        Some(&format!("refresh_token={legacy}")),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 401);
    assert!(!response.headers().contains_key("set-cookie"));
    let missing = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/refresh", fixture.api)),
        None,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(missing.status(), 400);
    let invalid = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/register", fixture.api)),
        None,
    )
    .json(&RegistryData {
        username: "empty-password".into(),
        nickname: "test".into(),
        password: "".into(),
    })
    .send()
    .await
    .unwrap();
    assert_eq!(invalid.status(), 400);
    let malformed = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/login", fixture.api)),
        None,
    )
    .json(&serde_json::json!({"username":"no-password"}))
    .send()
    .await
    .unwrap();
    assert_eq!(malformed.status(), 422);
    let row = fixture
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS issued FROM security_credential".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "issued").unwrap(), 0);
}
#[tokio::test]
async fn security_t24_http_missing_clock_unhealthy_and_hmac_preserve_accounts_and_credentials() {
    for mode in ["missing-clock", "unhealthy", "hmac"] {
        let fixture = local_http_fixture(mode).await;
        fixture
            .db
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE \"user\" SET username='fault-user', password=$1 WHERE id=1",
                ["$2y$04$FAiHSb5/5zp.oDR1GjBYSex5RVBJYW1BdEVBwqpdg4Phfk1i7YIPu".into()],
            ))
            .await
            .unwrap();
        let registered = csrf_request(
            &fixture,
            fixture.client.post(format!("{}/register", fixture.api)),
            None,
        )
        .json(&RegistryData {
            username: "must-not-exist".into(),
            nickname: "test".into(),
            password: "test password".into(),
        })
        .send()
        .await
        .unwrap();
        assert_eq!(registered.status(), 503, "{mode}");
        assert!(!registered.headers().contains_key("set-cookie"));
        let logged = csrf_request(
            &fixture,
            fixture.client.post(format!("{}/login", fixture.api)),
            None,
        )
        .json(&LoginData {
            username: "fault-user".into(),
            password: "test password".into(),
        })
        .send()
        .await
        .unwrap();
        assert_eq!(logged.status(), 503, "{mode}");
        assert!(!logged.headers().contains_key("set-cookie"));
        let row = fixture.db.query_one_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT (SELECT COUNT(*) FROM \"user\" WHERE username='must-not-exist') AS created, (SELECT COUNT(*) FROM security_credential) AS issued, (SELECT COUNT(*) FROM refresh_token) AS legacy".to_owned())).await.unwrap().unwrap();
        for column in ["created", "issued", "legacy"] {
            assert_eq!(row.try_get::<i64>("", column).unwrap(), 0, "{mode}");
        }
    }
}

fn test_clock(healthy: bool) -> Arc<dyn auth::security::budget::Clock> {
    Arc::new(auth::security::budget::SystemClock::new(Arc::new(
        move || healthy,
    )))
}
fn configure_local_server(cfg: &mut Config, address: SocketAddr) -> super::oidc::TestRsaFile {
    cfg.oidc.issuer = format!("http://{address}");
    cfg.auth.exp_seconds = 300;
    cfg.auth.refresh_exp_seconds = 3600;
    cfg.auth.secret = "t24-local-test-key".into();
    cfg.auth.origins = vec!["*".into()];
    // Local lifetimes must not inherit a different client's OIDC overrides.
    cfg.oidc.access_token_expire_seconds = Some(30);
    cfg.oidc.refresh_token_expire_seconds = Some(60);
    super::oidc::rsa_test_token(cfg).1
}

#[tokio::test]
async fn test_register() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let _rsa = configure_local_server(&mut cfg, addr);
    tokio::spawn(cobotusr::serve_with_security_clock(
        cfg.clone(),
        listener,
        shutdown_signal(),
        test_clock(true),
    ));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/register";

    let client = Client::new();
    // build body
    let body = RegistryData {
        username: "test_user".to_string(),
        nickname: "test_user".to_string(),
        password: "test_user".to_string(),
    };

    let resp = bootstrap_request(
        &client,
        &format!("http://{addr}"),
        client.post(format!("http://{}{}{}", addr, api_prefix, uri)),
    )
    .await?
    .json(&body)
    .send()
    .await?
    .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

#[tokio::test]
async fn test_login() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let _rsa = configure_local_server(&mut cfg, addr);
    tokio::spawn(cobotusr::serve_with_security_clock(
        cfg.clone(),
        listener,
        shutdown_signal(),
        test_clock(true),
    ));

    let api_prefix = cfg.http.api_prefix;

    // 1. 注册用户
    let uri_register = "/register";
    let client_register = Client::new();
    let body_register = RegistryData {
        username: "test_user".to_string(),
        nickname: "test_user".to_string(),
        password: "test_user".to_string(),
    };
    let resp_register = bootstrap_request(
        &client_register,
        &format!("http://{addr}"),
        client_register.post(format!("http://{}{}{}", addr, api_prefix, uri_register)),
    )
    .await?
    .json(&body_register)
    .send()
    .await?
    .error_for_status()?;
    assert!(resp_register.status().is_success());

    // 2. 登陆用户
    let uri = "/login";

    let client = Client::new();
    // build body
    let body = LoginData {
        username: "test_user".to_string(),
        password: "test_user".to_string(),
    };
    let resp = bootstrap_request(
        &client,
        &format!("http://{addr}"),
        client.post(format!("http://{}{}{}", addr, api_prefix, uri)),
    )
    .await?
    .json(&body)
    .send()
    .await?
    .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

async fn bootstrap_request(
    client: &Client,
    root: &str,
    request: reqwest::RequestBuilder,
) -> Result<reqwest::RequestBuilder, AppError> {
    let response = client
        .get(format!("{root}/api/v1/csrf"))
        .send()
        .await?
        .error_for_status()?;
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body: serde_json::Value = response.json().await?;
    Ok(request
        .header("origin", root)
        .header("cookie", cookie)
        .header("x-csrf-token", body["csrf_token"].as_str().unwrap()))
}

#[tokio::test]
async fn security_browser_readiness_reports_missing_and_unhealthy_clock() {
    for mode in ["healthy", "missing-clock", "unhealthy"] {
        let fixture = local_http_fixture(mode).await;
        let ready = mode == "healthy";
        let response = fixture
            .client
            .get(format!("{}/health/ready", fixture.api))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), if ready { 200 } else { 503 });
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap(),
            serde_json::json!({"ready": ready, "security_clock_synchronized": ready})
        );
        let response = fixture
            .client
            .get(format!("{}/csrf", fixture.api))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), if ready { 200 } else { 503 });
        if ready {
            let cookie = response
                .headers()
                .get("set-cookie")
                .unwrap()
                .to_str()
                .unwrap();
            assert!(cookie.starts_with("__Host-cobotusr-csrf="));
            assert!(
                cookie.contains("Secure;")
                    && cookie.contains("Path=/;")
                    && cookie.contains("SameSite=Lax;")
            );
        } else {
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }
}

#[tokio::test]
async fn security_browser_loopback_cookie_flow_and_monitor_failure() {
    let fixture = local_http_fixture("monitored-loopback").await;
    let response = fixture
        .client
        .get(format!("{}/csrf", fixture.api))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let csrf_cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert!(csrf_cookie.starts_with("cobotusr-dev-csrf="));
    let proof = response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let forbidden = fixture.client.post(format!("{}/register", fixture.api))
        .header("origin", "https://remote.example.test")
        .header("cookie", &csrf_cookie).header("x-csrf-token", &proof)
        .json(&serde_json::json!({"username":"loopback-test", "nickname":"test", "password":"test password"}))
        .send().await.unwrap();
    assert_eq!(forbidden.status(), 403);
    assert!(!forbidden.headers().contains_key("set-cookie"));
    let registered = fixture.client.post(format!("{}/register", fixture.api))
        .header("origin", &fixture.issuer).header("cookie", &csrf_cookie).header("x-csrf-token", &proof)
        .json(&serde_json::json!({"username":"loopback-test", "nickname":"test", "password":"test password"}))
        .send().await.unwrap();
    assert_eq!(registered.status(), 200);
    let mut session_cookies = Vec::new();
    for cookie in registered.headers().get_all("set-cookie") {
        let cookie = cookie.to_str().unwrap();
        assert!(!cookie.contains("Secure"));
        assert!(cookie.contains("Path=/;") && cookie.contains("SameSite=Lax;"));
        if cookie.starts_with("refresh_token=") {
            assert!(cookie.contains("HttpOnly;"));
        }
        session_cookies.push(cookie.split(';').next().unwrap().to_owned());
    }
    let body = registered.json::<serde_json::Value>().await.unwrap();
    let logout = fixture
        .client
        .post(format!("{}/logout", fixture.api))
        .header("origin", &fixture.issuer)
        .header("cookie", session_cookies.join("; "))
        .header("x-csrf-token", body["csrf_token"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), 200);
    for cookie in logout.headers().get_all("set-cookie") {
        let cookie = cookie.to_str().unwrap();
        assert!(cookie.contains("Max-Age=0"));
        assert!(!cookie.contains("Secure"));
    }
    fixture._ntp.as_ref().unwrap().abort();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let response = fixture
                .client
                .get(format!("{}/health/ready", fixture.api))
                .send()
                .await
                .unwrap();
            if response.status() == 503 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    let response = fixture
        .client
        .get(format!("{}/csrf", fixture.api))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert!(!response.headers().contains_key("set-cookie"));
}

#[tokio::test]
async fn security_browser_loopback_aliases_and_rejection_diagnostics() {
    let fixture = local_http_fixture("monitored-loopback").await;
    let response = fixture
        .client
        .get(format!("{}/csrf", fixture.api))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let proof = response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let port = reqwest::Url::parse(&fixture.root).unwrap().port().unwrap();
    for origin in [
        fixture.root.clone(),
        format!("http://localhost:{port}"),
        format!("http://[::1]:{port}"),
    ] {
        let response = fixture
            .client
            .post(format!("{}/login", fixture.api))
            .header("origin", origin)
            .header("cookie", &cookie)
            .header("x-csrf-token", &proof)
            .json(&LoginData {
                username: "isolated-nonexistent".into(),
                password: "wrong password".into(),
            })
            .send()
            .await
            .unwrap();
        // A valid browser proof must reach account authentication, which rejects this account.
        assert_eq!(response.status(), 401);
    }
    let cases = [
        (
            Some(format!(
                "http://localhost:{}",
                if port == 4000 { 4001 } else { 4000 }
            )),
            Some(cookie.as_str()),
            Some(proof.as_str()),
            "csrf_origin_not_allowed",
        ),
        (
            Some(format!("https://localhost:{port}")),
            Some(cookie.as_str()),
            Some(proof.as_str()),
            "csrf_origin_not_allowed",
        ),
        (
            Some("https://remote.example.test".into()),
            Some(cookie.as_str()),
            Some(proof.as_str()),
            "csrf_origin_not_allowed",
        ),
        (
            None,
            Some(cookie.as_str()),
            Some(proof.as_str()),
            "csrf_origin_missing",
        ),
        (
            Some("null".into()),
            Some(cookie.as_str()),
            Some(proof.as_str()),
            "csrf_origin_invalid",
        ),
        (
            Some(format!("http://localhost:{port}/")),
            Some(cookie.as_str()),
            Some(proof.as_str()),
            "csrf_origin_invalid",
        ),
        (
            Some(fixture.root.clone()),
            Some("cobotusr-dev-csrf=first; cobotusr-dev-csrf=second"),
            Some(proof.as_str()),
            "csrf_cookie_invalid",
        ),
        (
            Some(fixture.root.clone()),
            None,
            Some(proof.as_str()),
            "csrf_cookie_missing",
        ),
        (
            Some(fixture.root.clone()),
            Some(cookie.as_str()),
            None,
            "csrf_proof_missing",
        ),
        (
            Some(fixture.root.clone()),
            Some(cookie.as_str()),
            Some("wrong-proof"),
            "csrf_proof_mismatch",
        ),
        (
            Some(fixture.root.clone()),
            Some("cobotusr-dev-csrf=tampered"),
            Some("tampered"),
            "csrf_proof_invalid",
        ),
    ];
    for (origin, cookie, proof, expected) in cases {
        let mut request = fixture
            .client
            .post(format!("{}/login", fixture.api))
            .json(&LoginData {
                username: "isolated-nonexistent".into(),
                password: "wrong password".into(),
            });
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        if let Some(proof) = proof {
            request = request.header("x-csrf-token", proof);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), 403, "{expected}");
        assert!(!response.headers().contains_key("set-cookie"));
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        let body = response.json::<serde_json::Value>().await.unwrap();
        assert_eq!(body["code"], expected);
        if let Some(proof) = proof {
            assert!(!body.to_string().contains(proof));
        }
    }
    // The fixed production HTTPS issuer requires Secure cookies and rejects
    // development aliases. GET /csrf without its cookie is insufficient.
    let secure = local_http_fixture("healthy").await;
    let response = secure
        .client
        .get(format!("{}/csrf", secure.api))
        .send()
        .await
        .unwrap();
    assert!(
        response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Secure")
    );
    let proof = response.json::<serde_json::Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = secure
        .client
        .post(format!("{}/login", secure.api))
        .header("origin", &secure.issuer)
        .header("x-csrf-token", &proof)
        .json(&LoginData {
            username: "isolated-nonexistent".into(),
            password: "wrong password".into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["code"],
        "csrf_cookie_missing"
    );
}

#[tokio::test]
async fn auth_center_removed_im_routes_return_json_not_found() {
    let fixture = local_http_fixture("healthy").await;
    for suffix in ["/im/users", "/im/ws", "/im/files", "/im/conversations"] {
        let response = fixture
            .client
            .get(format!("{}{suffix}", fixture.api))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "removed route {suffix}");
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .contains("application/json")
        );
    }
    let response = fixture
        .client
        .post(format!("{}/im/conversations", fixture.api))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

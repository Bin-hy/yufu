use cobotusr::config::Config;
use cobotusr::error::AppError;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

use super::super::common::*;

use cobotusr::route::auth::{LoginData, RegistryData};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::sync::Arc;

struct LocalHttpFixture {
    _container: testcontainers::ContainerAsync<testcontainers::GenericImage>,
    _rsa: super::oidc::TestRsaFile,
    db: DatabaseConnection,
    root: String,
    api: String,
    client: Client,
    clock: Arc<dyn auth::security::budget::Clock>,
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
    let clock = test_clock(mode != "unhealthy");
    if mode == "hmac" {
        config.oidc.signing_algorithm = "HS256".into();
    }
    if mode == "missing-clock" {
        tokio::spawn(cobotusr::serve(config, listener, shutdown_signal()));
    } else {
        tokio::spawn(cobotusr::serve_with_security_clock(
            config,
            listener,
            shutdown_signal(),
            clock.clone(),
        ));
    }
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
        api,
        client,
        clock,
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
    let registration = fixture
        .client
        .post(format!("{}/register", fixture.api))
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
            &fixture.root,
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
    let wrong_login = fixture
        .client
        .post(format!("{}/login", fixture.api))
        .json(&LoginData {
            username: "local-http".into(),
            password: "wrong".into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_login.status(), 401);
    assert!(!wrong_login.headers().contains_key("set-cookie"));
    let logged = fixture
        .client
        .post(format!("{}/login", fixture.api))
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
    let refreshed = fixture
        .client
        .post(format!("{}/refresh", fixture.api))
        .header("cookie", &original_cookie)
        .header("origin", &fixture.root)
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
    let replay = fixture
        .client
        .post(format!("{}/refresh", fixture.api))
        .header("cookie", &original_cookie)
        .header("origin", &fixture.root)
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
    let response = fixture
        .client
        .post(format!("{}/refresh", fixture.api))
        .header("cookie", format!("refresh_token={legacy}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert!(!response.headers().contains_key("set-cookie"));
    let missing = fixture
        .client
        .post(format!("{}/refresh", fixture.api))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 400);
    let invalid = fixture
        .client
        .post(format!("{}/register", fixture.api))
        .json(&RegistryData {
            username: "empty-password".into(),
            nickname: "test".into(),
            password: "".into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);
    let malformed = fixture
        .client
        .post(format!("{}/login", fixture.api))
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
        let registered = fixture
            .client
            .post(format!("{}/register", fixture.api))
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
        let logged = fixture
            .client
            .post(format!("{}/login", fixture.api))
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

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
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
    let resp_register = client_register
        .post(format!(
            "http://{}{}{}",
            addr.clone(),
            api_prefix,
            uri_register
        ))
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
    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

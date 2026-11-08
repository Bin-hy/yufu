use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cobotusr::config::{BootstrapClient, Config};
use cobotusr::entity::role::RoleEnum;
use reqwest::{Client, Url};
use tokio::net::TcpListener;

use super::super::common::{get_postgres_container, shutdown_signal};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct TestRsaFile(std::path::PathBuf);
impl Drop for TestRsaFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn rsa_test_token(config: &mut Config) -> (String, TestRsaFile) {
    use rsa::pkcs8::EncodePrivateKey;
    static PEM: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let pem = PEM.get_or_init(|| {
        rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)
            .unwrap()
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string()
    });
    let path = std::env::temp_dir().join(format!("cobot-oidc-{}.pem", uuid::Uuid::new_v4()));
    std::fs::write(&path, pem).unwrap();
    config.oidc.signing_algorithm = "RS256".into();
    config.oidc.rsa_private_key_path = path.to_string_lossy().into();
    let signer = auth::token_manager::IdTokenSigner::from_rsa_pem(pem.as_bytes(), 7200).unwrap();
    (
        signer
            .sign_access_token(
                "1",
                &config.oidc.issuer,
                "cobotusr",
                "",
                7200,
                vec![],
                vec![],
            )
            .unwrap(),
        TestRsaFile(path),
    )
}

const CLIENT_ID: &str = "cobotimx-test";
const CONFIDENTIAL_CLIENT_ID: &str = "confidential-pkce-test";
const CONFIDENTIAL_CLIENT_SECRET: &str = "confidential-secret";
const REDIRECT_URI: &str = "http://localhost:5174/oauth/callback";
const POST_LOGOUT_REDIRECT_URI: &str = "http://localhost:5174/login";
const CODE_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CODE_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

async fn assert_subject_consistency(
    client: &Client,
    base_url: &str,
    tokens: &serde_json::Value,
    expected_subject: &str,
) {
    // Signature verification is covered by auth's signer unit tests; inspect
    // the wire payload here so a numeric subject cannot be coerced to a string.
    let id_token = tokens["id_token"].as_str().expect("ID Token should exist");
    let payload = id_token
        .split('.')
        .nth(1)
        .expect("JWT should have a payload");
    let claims: serde_json::Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload)
            .expect("JWT payload should be base64url"),
    )
    .expect("JWT payload should be JSON");
    assert_eq!(claims["sub"].as_str(), Some(expected_subject));

    let userinfo = client
        .get(format!("{base_url}/api/v1/oidc/userinfo"))
        .bearer_auth(
            tokens["access_token"]
                .as_str()
                .expect("Access Token should exist"),
        )
        .send()
        .await
        .expect("UserInfo request should complete")
        .error_for_status()
        .expect("UserInfo request should succeed")
        .json::<serde_json::Value>()
        .await
        .expect("UserInfo response should be JSON");
    assert_eq!(userinfo["sub"].as_str(), Some(expected_subject));
    assert_eq!(userinfo["sub"], claims["sub"]);
}

async fn authorize_code(client: &Client, base_url: &str, access_token: &str) -> String {
    authorize_code_for_client(client, base_url, access_token, CLIENT_ID).await
}

async fn authorize_code_for_client(
    client: &Client,
    base_url: &str,
    access_token: &str,
    client_id: &str,
) -> String {
    let response = client
        .post(format!("{base_url}/api/v1/oidc/authorize/consent"))
        .bearer_auth(access_token)
        .json(&serde_json::json!({
            "client_id": client_id,
            "redirect_uri": REDIRECT_URI,
            "response_type": "code",
            "code_challenge": CODE_CHALLENGE,
            "code_challenge_method": "S256",
            "scope": "openid profile email",
            "state": "test-state",
            "approved": true,
        }))
        .send()
        .await
        .expect("consent request should complete")
        .error_for_status()
        .expect("consent should be accepted")
        .json::<serde_json::Value>()
        .await
        .expect("consent response should be valid JSON");

    let redirect_url = response["redirect_url"]
        .as_str()
        .expect("consent response should include a redirect URL");
    Url::parse(redirect_url)
        .expect("redirect URL should be valid")
        .query_pairs()
        .find_map(|(key, value)| (key == "code").then(|| value.into_owned()))
        .expect("authorization redirect should include a code")
}

#[tokio::test]
async fn test_oidc_authorization_code_pkce_exchange() {
    let (_postgres, database_url) = get_postgres_container()
        .await
        .expect("Postgres test container should start");
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .expect("test listener should bind");
    let address = listener
        .local_addr()
        .expect("listener should have an address");

    let mut config = Config::default();
    config.database.url = database_url.clone();
    config.oidc.issuer = format!("http://{address}");
    // Config::default() derives zero token lifetimes (the `serde(default = ...)`
    // fallbacks only apply to file-based configs); set explicit values so
    // issued refresh tokens do not expire immediately.
    config.auth.exp_seconds = 7200;
    config.auth.refresh_exp_seconds = 604800;
    config.oidc.bootstrap_clients.push(BootstrapClient {
        client_id: CLIENT_ID.to_string(),
        name: "IM OIDC integration test".to_string(),
        redirect_uris: vec![
            REDIRECT_URI.to_string(),
            POST_LOGOUT_REDIRECT_URI.to_string(),
        ],
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
        client_id: CONFIDENTIAL_CLIENT_ID.to_string(),
        name: "Confidential PKCE integration test".to_string(),
        redirect_uris: vec![REDIRECT_URI.to_string()],
        allowed_scopes: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
        ],
        grant_types: vec!["authorization_code".to_string()],
        client_secret: Some(CONFIDENTIAL_CLIENT_SECRET.to_string()),
        public_client: false,
    });
    let (access_token, _rsa_file) = rsa_test_token(&mut config);
    let healthy = Arc::new(AtomicBool::new(true));
    let health = healthy.clone();
    let clock: Arc<dyn auth::security::budget::Clock> = Arc::new(
        auth::security::budget::SystemClock::new(Arc::new(move || health.load(Ordering::SeqCst))),
    );
    tokio::spawn(cobotusr::serve_with_security_clock(
        config,
        listener,
        shutdown_signal(),
        clock.clone(),
    ));

    let client = Client::new();
    let base_url = format!("http://{address}");
    let expected_subject = RoleEnum::Superadmin.default_id().to_string();

    let end_session = client
        .get(format!("{base_url}/oidc/logout"))
        .query(&[
            ("client_id", CLIENT_ID),
            ("post_logout_redirect_uri", POST_LOGOUT_REDIRECT_URI),
        ])
        .send()
        .await
        .expect("end-session request should complete");
    assert_eq!(end_session.status(), StatusCode::OK);
    let db = sea_orm::Database::connect(&database_url).await.unwrap();
    use sea_orm::ConnectionTrait;
    for client_id in [CLIENT_ID, CONFIDENTIAL_CLIENT_ID] {
        db.execute_raw(sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Postgres,
            "INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ($1,$2,'cobotsrv','cobotsrv',TRUE)",
            [base_url.clone().into(), client_id.into()])).await.unwrap();
    }
    assert!(end_session.headers().contains_key("set-cookie"));
    let end_session_body = end_session
        .text()
        .await
        .expect("end-session response should be text");
    assert!(end_session_body.contains(POST_LOGOUT_REDIRECT_URI));
    assert!(end_session_body.contains("name.endsWith('-token-key')"));

    let invalid_end_session = client
        .get(format!("{base_url}/oidc/logout"))
        .query(&[
            ("client_id", CLIENT_ID),
            ("post_logout_redirect_uri", "http://attacker.invalid/logout"),
        ])
        .send()
        .await
        .expect("invalid end-session request should complete");
    assert_eq!(invalid_end_session.status(), StatusCode::BAD_REQUEST);

    let missing_pkce = client
        .post(format!("{base_url}/api/v1/oidc/authorize/consent"))
        .bearer_auth(&access_token)
        .json(&serde_json::json!({
            "client_id": CLIENT_ID,
            "redirect_uri": REDIRECT_URI,
            "response_type": "code",
            "scope": "openid profile email",
            "approved": true,
        }))
        .send()
        .await
        .expect("missing PKCE consent request should complete");
    assert_eq!(missing_pkce.status(), StatusCode::BAD_REQUEST);

    let plain_pkce = client
        .post(format!("{base_url}/api/v1/oidc/authorize/consent"))
        .bearer_auth(&access_token)
        .json(&serde_json::json!({
            "client_id": CLIENT_ID,
            "redirect_uri": REDIRECT_URI,
            "response_type": "code",
            "code_challenge": CODE_VERIFIER,
            "code_challenge_method": "plain",
            "scope": "openid profile email",
            "approved": true,
        }))
        .send()
        .await
        .expect("plain PKCE consent request should complete");
    assert_eq!(plain_pkce.status(), StatusCode::BAD_REQUEST);

    let confidential_code =
        authorize_code_for_client(&client, &base_url, &access_token, CONFIDENTIAL_CLIENT_ID).await;
    let confidential_without_secret = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", confidential_code.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CONFIDENTIAL_CLIENT_ID),
            ("code_verifier", CODE_VERIFIER),
        ])
        .send()
        .await
        .expect("confidential token request should complete");
    assert_eq!(
        confidential_without_secret.status(),
        StatusCode::BAD_REQUEST
    );
    let confidential_without_secret_body = confidential_without_secret
        .text()
        .await
        .expect("confidential error response should be text");
    assert!(
        confidential_without_secret_body.contains("client_secret"),
        "error should mention client_secret, got: {confidential_without_secret_body}"
    );

    // Positive path: confidential client + PKCE + correct secret succeeds.
    let confidential_code_ok =
        authorize_code_for_client(&client, &base_url, &access_token, CONFIDENTIAL_CLIENT_ID).await;
    let confidential_exchanged = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", confidential_code_ok.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CONFIDENTIAL_CLIENT_ID),
            ("client_secret", CONFIDENTIAL_CLIENT_SECRET),
            ("code_verifier", CODE_VERIFIER),
        ])
        .send()
        .await
        .expect("confidential token exchange should complete");
    assert_eq!(confidential_exchanged.status(), StatusCode::OK);
    let confidential_json = confidential_exchanged
        .json::<serde_json::Value>()
        .await
        .expect("confidential token response should be valid JSON");
    assert!(confidential_json["access_token"].as_str().is_some());
    assert!(confidential_json["id_token"].as_str().is_some());
    assert_subject_consistency(&client, &base_url, &confidential_json, &expected_subject).await;

    let code = authorize_code(&client, &base_url, &access_token).await;
    let exchanged = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CLIENT_ID),
            ("code_verifier", CODE_VERIFIER),
        ])
        .send()
        .await
        .expect("token request should complete");
    assert_eq!(exchanged.status(), StatusCode::OK);
    let token_json = exchanged
        .json::<serde_json::Value>()
        .await
        .expect("token response should be valid JSON");
    assert!(token_json["access_token"].as_str().is_some());
    assert!(token_json["id_token"].as_str().is_some());
    assert_subject_consistency(&client, &base_url, &token_json, &expected_subject).await;
    let refresh_token = token_json["refresh_token"]
        .as_str()
        .expect("authorization code exchange should return a refresh token");

    let refreshed = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .expect("public-client refresh request should complete");
    assert_eq!(refreshed.status(), StatusCode::OK);
    let refreshed_json = refreshed
        .json::<serde_json::Value>()
        .await
        .expect("refresh response should be valid JSON");
    assert_subject_consistency(&client, &base_url, &refreshed_json, &expected_subject).await;
    let rotated_refresh_token = refreshed_json["refresh_token"]
        .as_str()
        .expect("refresh response should rotate the refresh token");

    let revocation = client
        .post(format!("{base_url}/oidc/revoke"))
        .form(&[
            ("token", rotated_refresh_token),
            ("token_type_hint", "refresh_token"),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .expect("public-client revocation request should complete");
    assert_eq!(revocation.status(), StatusCode::OK);

    let revoked_refresh = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", rotated_refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .expect("revoked refresh request should complete");
    assert_eq!(revoked_refresh.status(), StatusCode::UNAUTHORIZED);

    let invalid_code = authorize_code(&client, &base_url, &access_token).await;
    let invalid_exchange = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", invalid_code.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CLIENT_ID),
            ("code_verifier", "invalid-verifier"),
        ])
        .send()
        .await
        .expect("invalid token request should complete");
    assert_eq!(invalid_exchange.status(), StatusCode::BAD_REQUEST);
    let retry = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", invalid_code.as_str()),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", CODE_VERIFIER),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        retry.status(),
        StatusCode::OK,
        "invalid PKCE must not consume code"
    );
    let tokens: serde_json::Value = retry.json().await.unwrap();
    let db = sea_orm::Database::connect(&database_url).await.unwrap();
    let checker = Arc::new(
        cobotusr::service::security::lifecycle::CredentialLifecycle::new(
            Arc::new(
                cobotusr::service::security::repository::PgSecurityRepository::new(
                    db.clone(),
                    clock.clone(),
                ),
            ),
            clock.clone(),
        ),
    );
    use auth::security::{
        credential::{CredentialExpectation, CredentialProfile, StrictAccessState},
        identity::{PrincipalKind, TokenUse},
    };
    let jwks = Arc::new(
        auth::jwks::JwksClient::new(
            format!("{base_url}/.well-known/jwks.json"),
            Some("cobotsrv".into()),
        )
        .await
        .unwrap(),
    );
    let strict = StrictAccessState::new(
        jwks,
        CredentialExpectation::new(
            &base_url,
            "cobotsrv",
            "cobotsrv",
            vec![CredentialProfile::new(PrincipalKind::User, TokenUse::UserSession).unwrap()],
        )
        .unwrap(),
        clock.clone(),
        checker,
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", tokens["access_token"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    strict.authenticate(&headers).await.unwrap();
    healthy.store(false, Ordering::SeqCst);
    let unavailable = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", tokens["refresh_token"].as_str().unwrap()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    healthy.store(true, Ordering::SeqCst);
    let rotated = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", tokens["refresh_token"].as_str().unwrap()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(rotated.status(), StatusCode::OK);
    let replay = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", tokens["refresh_token"].as_str().unwrap()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

async fn assert_session_configuration_failure(mode: &str) {
    use cobotusr::service::oidc::{CreateAuthCodeParams, OidcService};
    use sea_orm::ConnectionTrait;
    let (_container, database_url) = get_postgres_container().await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let mut config = Config::default();
    config.database.url = database_url.clone();
    config.oidc.issuer = base_url.clone();
    config.auth.exp_seconds = 300;
    config.auth.refresh_exp_seconds = 3600;
    let (_token, _rsa_file) = rsa_test_token(&mut config);
    let healthy = mode != "unhealthy";
    if mode == "hmac" {
        config.oidc.signing_algorithm = "HS256".into();
    }
    if mode == "missing-clock" {
        tokio::spawn(cobotusr::serve(config, listener, shutdown_signal()));
    } else {
        let clock = Arc::new(auth::security::budget::SystemClock::new(Arc::new(
            move || healthy,
        )));
        tokio::spawn(cobotusr::serve_with_security_clock(
            config,
            listener,
            shutdown_signal(),
            clock,
        ));
    }
    let client = Client::new();
    client
        .get(format!("{base_url}/.well-known/openid-configuration"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let db = sea_orm::Database::connect(&database_url).await.unwrap();
    OidcService::register_client(
        &db,
        1,
        CLIENT_ID.into(),
        "config-test".into(),
        vec![REDIRECT_URI.into()],
        vec!["openid".into()],
        vec!["authorization_code".into(), "refresh_token".into()],
        true,
    )
    .await
    .unwrap();
    db.execute_raw(sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Postgres,
        "INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ($1,$2,'cobotsrv','cobotsrv',TRUE)", [base_url.clone().into(), CLIENT_ID.into()])).await.unwrap();
    let code = OidcService::create_authorization_code(
        &db,
        CreateAuthCodeParams {
            user_id: 1,
            client_id: CLIENT_ID.into(),
            redirect_uri: Some(REDIRECT_URI.into()),
            code_challenge: Some(CODE_CHALLENGE.into()),
            code_challenge_method: "S256".into(),
            scope: Some("openid".into()),
            state: None,
            nonce: None,
        },
    )
    .await
    .unwrap();
    let response = client
        .post(format!("{base_url}/oidc/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", CODE_VERIFIER),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{mode}");
    let counts = db.query_one_raw(sea_orm::Statement::from_string(sea_orm::DbBackend::Postgres,
        "SELECT (SELECT COUNT(*) FROM oauth_authorization_code WHERE used) AS used, (SELECT COUNT(*) FROM security_credential) AS issued, (SELECT COUNT(*) FROM oauth_token) AS legacy".to_owned())).await.unwrap().unwrap();
    for column in ["used", "issued", "legacy"] {
        assert_eq!(counts.try_get::<i64>("", column).unwrap(), 0);
    }
}

#[tokio::test]
async fn security_t21_http_missing_clock_preserves_authorization_code() {
    assert_session_configuration_failure("missing-clock").await;
}
#[tokio::test]
async fn security_t21_http_unhealthy_clock_preserves_authorization_code() {
    assert_session_configuration_failure("unhealthy").await;
}
#[tokio::test]
async fn security_t21_http_hmac_signer_preserves_authorization_code() {
    assert_session_configuration_failure("hmac").await;
}

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::http::StatusCode;
use cobotusr::config::{BootstrapClient, Config};
use reqwest::{Client, Url};
use tokio::net::TcpListener;

use super::super::common::{get_postgres_container, get_test_token, shutdown_signal};

const CLIENT_ID: &str = "cobotimx-test";
const CONFIDENTIAL_CLIENT_ID: &str = "confidential-pkce-test";
const CONFIDENTIAL_CLIENT_SECRET: &str = "confidential-secret";
const REDIRECT_URI: &str = "http://localhost:5174/oauth/callback";
const POST_LOGOUT_REDIRECT_URI: &str = "http://localhost:5174/login";
const CODE_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CODE_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

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
    config.database.url = database_url;
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
    let access_token = get_test_token(&mut config);
    tokio::spawn(cobotusr::serve(config, listener, shutdown_signal()));

    let client = Client::new();
    let base_url = format!("http://{address}");

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
}

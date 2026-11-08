use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::http::StatusCode;
use cobotusr::config::{BootstrapClient, Config};
use reqwest::Client;
use testcontainers::{ContainerAsync, GenericImage};
use tokio::net::TcpListener;

use super::super::common::{get_postgres_container, get_test_token, shutdown_signal};

const SPA_CLIENT_ID: &str = "resource-app-spa-test";
const MACHINE_CLIENT_ID: &str = "resource-app-sync-test";
const MACHINE_CLIENT_SECRET: &str = "machine-secret";
const PERMISSION_NAME: &str = "device:read";

/// Boot a cobotusr with a public SPA client + a confidential machine client,
/// mirroring the production resource-app/resource-app-sync split. Returns the Postgres
/// container (must stay alive for the server's lifetime), base URL and an
/// admin access token for verifying results via the permission API.
async fn start_server() -> (ContainerAsync<GenericImage>, String, String) {
    let (postgres, database_url) = get_postgres_container()
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
    config.auth.exp_seconds = 7200;
    config.auth.refresh_exp_seconds = 604800;

    config.oidc.bootstrap_clients.push(BootstrapClient {
        client_id: SPA_CLIENT_ID.to_string(),
        name: "SPA test client".to_string(),
        redirect_uris: vec!["http://localhost:4001/oauth/callback".to_string()],
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
        client_id: MACHINE_CLIENT_ID.to_string(),
        name: "Machine test client".to_string(),
        redirect_uris: vec![],
        allowed_scopes: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
        ],
        grant_types: vec!["client_credentials".to_string()],
        client_secret: Some(MACHINE_CLIENT_SECRET.to_string()),
        public_client: false,
    });

    let access_token = get_test_token(&mut config);
    tokio::spawn(cobotusr::serve(config, listener, shutdown_signal()));
    (postgres, format!("http://{address}"), access_token)
}

async fn sync_permissions(
    client: &Client,
    base_url: &str,
    body: serde_json::Value,
) -> reqwest::Response {
    client
        .post(format!("{base_url}/api/v1/application/sync-permissions"))
        .json(&body)
        .send()
        .await
        .expect("sync-permissions request should complete")
}

#[tokio::test]
async fn test_sync_permissions_app_key_proxy_and_guards() {
    let (_postgres, base_url, access_token) = start_server().await;
    let client = Client::new();

    // ── 1. machine client + app_key = SPA client → permissions land on the
    //       SPA client's application (the one PDP / require_perm! checks).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": MACHINE_CLIENT_SECRET,
            "app_key": SPA_CLIENT_ID,
            "permissions": [{
                "name": PERMISSION_NAME,
                "action": "read",
                "resource_type": "device",
            }],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("sync response should be JSON");
    let app_id = body["application_id"]
        .as_i64()
        .expect("sync response should carry application_id");
    assert_eq!(body["synced"], 1);

    let perms = client
        .get(format!("{base_url}/api/v1/permission"))
        .query(&[
            ("application_id", app_id.to_string()),
            ("page_size", "50".to_string()),
            ("page_number", "1".to_string()),
        ])
        .bearer_auth(&access_token)
        .send()
        .await
        .expect("permission list request should complete")
        .error_for_status()
        .expect("permission list should be authorized");
    let perms_body: serde_json::Value = perms.json().await.expect("permission list JSON");
    let names: Vec<&str> = perms_body["data_list"]
        .as_array()
        .expect("data_list should be an array")
        .iter()
        .map(|p| p["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        names.contains(&PERMISSION_NAME),
        "permission should be registered under the app_key application, got: {names:?}"
    );

    // ── 2. unknown app_key → 400 (explicit app_keys must already exist).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": MACHINE_CLIENT_SECRET,
            "app_key": "no-such-application",
            "permissions": [],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // ── 3. no app_key → defaults to the authenticating client's own
    //       application (legacy behaviour).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": MACHINE_CLIENT_SECRET,
            "permissions": [{
                "name": "mcp:access",
                "action": "access",
                "resource_type": "mcp",
            }],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("sync response should be JSON");
    assert_eq!(body["synced"], 1);

    // ── 4. public client cannot authenticate (no secret to verify).
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": SPA_CLIENT_ID,
            "client_secret": "anything",
            "permissions": [],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // ── 5. wrong secret on the machine client → 401.
    let resp = sync_permissions(
        &client,
        &base_url,
        serde_json::json!({
            "client_id": MACHINE_CLIENT_ID,
            "client_secret": "wrong-secret",
            "permissions": [],
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

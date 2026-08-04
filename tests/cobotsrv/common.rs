use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{Json, Router, routing::get};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use sea_orm::{Database, DatabaseConnection};
use serde_json::{Value, json};
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use auth::claims::AccessTokenClaims;

pub struct CobotsrvTestCtx {
    /// Keep Postgres container alive for the test duration.
    pub _db: ContainerAsync<GenericImage>,
    /// Stop the mock JWKS server when context is dropped.
    pub _jwks_stop: oneshot::Sender<()>,
    /// Base URL of the running cobotsrv (e.g. "http://127.0.0.1:PORT").
    pub base: String,
    /// A valid Bearer token with mcp:access + mcp:admin + task:execute + device:read.
    pub token: String,
    /// PKCS#8 PEM of the signing key — mint custom-permission tokens with
    /// [`sign_test_token`].
    pub signing_pem: Vec<u8>,
    /// Direct database connection for inserting test fixtures.
    pub db: DatabaseConnection,
}

impl CobotsrvTestCtx {
    /// POST /mcp helper: sends a JSON-RPC request and returns the parsed response.
    pub async fn mcp_post(&self, body: Value) -> (reqwest::StatusCode, Value) {
        self.mcp_post_with(&self.token, body).await
    }

    /// POST /mcp with an explicit token.
    pub async fn mcp_post_with(&self, token: &str, body: Value) -> (reqwest::StatusCode, Value) {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/mcp", self.base))
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
            .expect("mcp post failed");
        let status = resp.status();
        let json: Value = resp.json().await.expect("response is not json");
        (status, json)
    }

    /// Authenticated GET helper. Returns (status, parsed-json-if-any).
    pub async fn get_authed(&self, path: &str) -> (reqwest::StatusCode, Value) {
        self.get_authed_with(&self.token, path).await
    }

    /// Authenticated GET with an explicit token.
    pub async fn get_authed_with(&self, token: &str, path: &str) -> (reqwest::StatusCode, Value) {
        let resp = reqwest::Client::new()
            .get(format!("{}{}", self.base, path))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("get failed");
        let status = resp.status();
        let json = resp.json().await.unwrap_or(Value::Null);
        (status, json)
    }
}

/// Start a fresh cobotsrv backed by an isolated Postgres container.
///
/// Internals:
/// 1. Spin up Postgres 18-alpine via testcontainers.
/// 2. Generate a throwaway RSA-2048 key pair.
/// 3. Serve a mock JWKS endpoint on a random port.
/// 4. Sign a long-lived test token (RS256, aud="cobotsrv").
/// 5. Start cobotsrv with MQTT disabled and permission checks off
///    (so token-embedded permissions are used directly).
pub async fn start() -> CobotsrvTestCtx {
    // ── 1. Postgres ──────────────────────────────────────────────────────────
    let db_name = format!("test_cobotsrv_{}", uuid::Uuid::new_v4().simple());
    let image = GenericImage::new("postgres", "18-alpine")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", db_name.clone());
    let container = image.start().await.expect("postgres start failed");
    let pg_port = container.get_host_port_ipv4(5432.tcp()).await.unwrap();
    let db_url = format!(
        "postgres://postgres:postgres@localhost:{}/{}",
        pg_port, db_name
    );

    // ── 2. RSA key pair ──────────────────────────────────────────────────────
    let mut rng = rand::rngs::OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("rsa keygen failed");
    let n_b64 = URL_SAFE_NO_PAD.encode(private_key.n().to_bytes_be());
    let e_b64 = URL_SAFE_NO_PAD.encode(private_key.e().to_bytes_be());

    // ── 3. Mock JWKS server ───────────────────────────────────────────────────
    let jwks = json!({
        "keys": [{"kid": "test-key", "n": n_b64, "e": e_b64}]
    });
    let (jwks_addr, _jwks_stop) = start_jwks_server(jwks).await;
    let jwks_url = format!("http://{}/jwks.json", jwks_addr);

    // ── 4. Test token ─────────────────────────────────────────────────────────
    let pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .expect("pem encode failed");
    let token = sign_test_token(
        pem.as_bytes(),
        &["mcp:access", "mcp:admin", "task:execute", "device:read"],
    );

    // ── 5. Start cobotsrv ─────────────────────────────────────────────────────
    let mut cfg = cobotsrv::config::Config::default();
    cfg.database.url = db_url.clone();
    cfg.auth.jwks_url = jwks_url;
    cfg.auth.expected_audience = "cobotsrv".to_string();
    cfg.auth.require_permissions = false; // use token-embedded permissions
    cfg.mqtt = None; // no MQTT in tests

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());

    tokio::spawn(cobotsrv::serve(cfg, listener, futures::future::pending()));

    // Wait for the server to be ready: poll /hello until it responds or 10s elapses.
    let health_url = format!("{}/hello", base);
    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if client.get(&health_url).send().await.is_ok() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!("cobotsrv did not become ready within 10 seconds");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // ── 6. Direct DB connection for fixture insertion ─────────────────────────
    let db = Database::connect(&db_url)
        .await
        .expect("direct db connect failed");

    CobotsrvTestCtx {
        _db: container,
        _jwks_stop,
        base,
        token,
        signing_pem: pem.as_bytes().to_vec(),
        db,
    }
}

/// Start a tiny axum server that serves a static JWKS JSON at /jwks.json.
/// Returns (addr, stop_sender). Drop the sender to shut the server down.
async fn start_jwks_server(jwks: Value) -> (std::net::SocketAddr, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx, rx) = oneshot::channel::<()>();

    let app = Router::new().route(
        "/jwks.json",
        get(move || {
            let jwks = jwks.clone();
            async move { Json(jwks) }
        }),
    );

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .ok();
    });

    (addr, tx)
}

/// Sign an RS256 JWT with the given PEM private key and permission set.
/// Claims: aud="cobotsrv", 100yr expiry.
pub fn sign_test_token(pem: &[u8], permissions: &[&str]) -> String {
    let encoding_key = EncodingKey::from_rsa_pem(pem).expect("encoding key from pem");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key".to_string());

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = AccessTokenClaims {
        iss: "test".to_string(),
        sub: "1".to_string(),
        aud: "cobotsrv".to_string(),
        iat: now,
        exp: now + 60 * 60 * 24 * 365 * 100,
        jti: uuid::Uuid::new_v4().to_string(),
        scope: String::new(),
        roles: vec![],
        permissions: permissions.iter().map(|p| p.to_string()).collect(),
    };

    encode(&header, &claims, &encoding_key).expect("token sign failed")
}

/// Insert a test device directly into the database, bypassing MQTT registration.
pub async fn insert_test_device(
    db: &DatabaseConnection,
    cobotlet_alias: &str,
    device_id: &str,
    capability_name: &str,
    online: bool,
) {
    use cobotsrv::service::device::{DeviceService, UpsertDevice};
    let now = chrono::Utc::now().naive_utc();
    DeviceService::upsert(
        db,
        UpsertDevice {
            cobotlet_alias,
            device_id,
            service_url: "http://test-device:9000".to_string(),
            capabilities: serde_json::json!([{
                "name": capability_name,
                "method": "POST",
                "path": format!("/{}", capability_name),
                "input_schema": {"type": "object", "properties": {"x": {"type": "number"}}},
            }]),
            state: if online { "ready" } else { "offline" }.to_string(),
            last_heartbeat: now,
            registered_at: now,
        },
    )
    .await
    .expect("insert test device failed");
}

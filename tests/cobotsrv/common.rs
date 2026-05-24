use anyhow::{Context, anyhow};
use auth::token_manager::TokenManager;
use cobotsrv::config::Config;
use cobotsrv::entity::role::RoleEnum;
use cobotsrv::error::AppError;
use cobotsrv::route::auth::RegistryData;
use reqwest::Client;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::net::TcpListener;

pub async fn shutdown_signal() {
    let _str = signal::wait_for_stop_signal().await;
}

#[allow(dead_code)]
pub async fn get_postgres_container()
-> Result<(testcontainers::ContainerAsync<Postgres>, String), AppError> {
    let db_name = format!("test_db_{}", uuid::Uuid::new_v4());
    let image_postgres = Postgres::default().with_db_name(&db_name);
    let container = image_postgres.start().await?;

    let host_port = container.get_host_port_ipv4(5432.tcp()).await?;
    let db_url = format!(
        "postgres://postgres:postgres@localhost:{}/{}",
        host_port, db_name
    );
    Ok((container, db_url))
}

#[allow(dead_code)]
pub fn get_test_token(secret: String) -> String {
    let token_manager = TokenManager {
        secret,
        expire_seconds: 60 * 60 * 24 * 365 * 100,
    };
    token_manager
        .generate_token(RoleEnum::Superadmin.default_id())
        .unwrap()
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestArtifacts {
    pub root_dir: String,
    pub rsa_private_key_path: String,
    pub storage_root_dir: String,
}

#[allow(dead_code)]
pub fn configure_isolated_test_paths(cfg: &mut Config) -> TestArtifacts {
    let root_dir = format!("target/test-artifacts/{}", uuid::Uuid::new_v4());
    let rsa_private_key_path = format!("{root_dir}/oidc_rsa_private.pem");
    let storage_root_dir = format!("{root_dir}/storage");

    cfg.oidc.rsa_private_key_path = rsa_private_key_path.clone();
    cfg.auth.rsa_private_key_path = rsa_private_key_path.clone();
    cfg.storage.root_dir = storage_root_dir.clone();

    TestArtifacts {
        root_dir,
        rsa_private_key_path,
        storage_root_dir,
    }
}

#[allow(dead_code)]
pub fn build_test_config(db_url: String) -> Config {
    let mut cfg = Config::default();
    cfg.database.url = db_url;
    configure_isolated_test_paths(&mut cfg);
    cfg
}

pub struct TestServer {
    _postgres: testcontainers::ContainerAsync<Postgres>,
    pub addr: SocketAddr,
    pub api_prefix: String,
    pub auth_secret: String,
    pub client: Client,
}

impl TestServer {
    pub fn api_url(&self, path: &str) -> String {
        format!("http://{}{}{}", self.addr, self.api_prefix, path)
    }

    pub fn ws_url(&self, path: &str) -> String {
        format!("ws://{}{}{}", self.addr, self.api_prefix, path)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestUser {
    pub id: i64,
    pub username: String,
    pub password: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
struct AuthTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiEnvelope<T> {
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiEnvelope<T> {
    pub fn into_data(self) -> anyhow::Result<T> {
        self.data
            .ok_or_else(|| anyhow!("response data is missing, message: {}", self.message))
    }
}

#[allow(dead_code)]
pub fn bearer_token(token: &str) -> String {
    format!("Bearer {}", token)
}

#[allow(dead_code)]
pub async fn spawn_test_server() -> anyhow::Result<TestServer> {
    let (postgres, db_url) = get_postgres_container()
        .await
        .map_err(|err| anyhow!("failed to start postgres test container: {:?}", err))?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let cfg = build_test_config(db_url);
    let api_prefix = cfg.http.api_prefix.clone();
    let auth_secret = cfg.auth.secret.clone();

    tokio::spawn(cobotsrv::serve(cfg, listener, shutdown_signal()));

    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;
    wait_for_server_ready(&client, addr).await?;

    Ok(TestServer {
        _postgres: postgres,
        addr,
        api_prefix,
        auth_secret,
        client,
    })
}

#[allow(dead_code)]
pub async fn register_user(server: &TestServer, prefix: &str) -> anyhow::Result<TestUser> {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let username = format!("{prefix}_{suffix}");
    let password = format!("pwd_{suffix}");

    let response = server
        .client
        .post(server.api_url("/register"))
        .json(&RegistryData {
            username: username.clone(),
            nickname: username.clone(),
            password: password.clone(),
        })
        .send()
        .await?
        .error_for_status()?;

    let auth: AuthTokenResponse = response.json().await?;
    let token_manager = TokenManager::new(server.auth_secret.clone(), 60);
    let claims = token_manager
        .verify_token::<i32>(&auth.access_token)
        .map_err(|err| anyhow!("failed to decode access token: {:?}", err))?;

    Ok(TestUser {
        id: i64::from(claims.id),
        username,
        password,
        token: auth.access_token,
    })
}

async fn wait_for_server_ready(client: &Client, addr: SocketAddr) -> anyhow::Result<()> {
    let url = format!("http://{addr}/error?code=204");

    for _ in 0..50 {
        match client.get(&url).send().await {
            Ok(response) if response.status().as_u16() == 204 => return Ok(()),
            Ok(_) | Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    Err(anyhow!("server did not become ready at {url}"))
        .context("cobotsrv startup readiness check failed")
}

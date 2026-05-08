use std::{env, net::SocketAddr, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub http: Http,
    #[serde(default)]
    pub log: Log,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub database: Database,

    #[cfg(feature = "net4mqtt")]
    #[serde(default)]
    pub net4mqtt: Option<Net4mqtt>,

    #[serde(default)]
    pub mqtt: Option<Mqtt>,

    #[serde(default)]
    pub task: Task,

    #[serde(default)]
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Mqtt {
    pub url: String,
    #[serde(default = "default_mqtt_client_id")]
    pub client_id: String,
    #[serde(default = "default_mqtt_topic_prefix")]
    pub topic_prefix: String,
    #[serde(default = "default_mqtt_keep_alive_secs")]
    pub keep_alive_secs: u64,
}

fn default_mqtt_client_id() -> String {
    String::from("cobotrix")
}

fn default_mqtt_topic_prefix() -> String {
    String::from("cobotrix")
}

fn default_mqtt_keep_alive_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    #[serde(default = "default_task_wait_timeout_ms")]
    pub wait_timeout_ms: u64,
    #[serde(default = "default_task_result_cache_ttl_secs")]
    pub result_cache_ttl_secs: u64,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            wait_timeout_ms: default_task_wait_timeout_ms(),
            result_cache_ttl_secs: default_task_result_cache_ttl_secs(),
        }
    }
}

fn default_task_wait_timeout_ms() -> u64 {
    5000
}

fn default_task_result_cache_ttl_secs() -> u64 {
    300
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub url: String,
}

#[cfg(feature = "net4mqtt")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Net4mqtt {
    #[serde(default)]
    pub mqtt_url: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default = "default_net4mqtt_listen")]
    pub listen: SocketAddr,
    #[serde(default = "default_net4mqtt_domain")]
    pub domain: String,
}

#[cfg(feature = "net4mqtt")]
impl Net4mqtt {
    pub fn validate(&mut self) {
        self.mqtt_url = self.mqtt_url.replace("{alias}", &self.alias)
    }
}

#[cfg(feature = "net4mqtt")]
impl Default for Net4mqtt {
    fn default() -> Self {
        Self {
            mqtt_url: String::new(),
            alias: String::new(),
            listen: default_net4mqtt_listen(),
            domain: default_net4mqtt_domain(),
        }
    }
}

#[cfg(feature = "net4mqtt")]
fn default_net4mqtt_listen() -> SocketAddr {
    SocketAddr::from_str("0.0.0.0:1077").expect("invalid listen socks address")
}

#[cfg(feature = "net4mqtt")]
fn default_net4mqtt_domain() -> String {
    String::from("net4mqtt.local")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Http {
    #[serde(default = "default_http_listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub public: String,
    #[serde(default = "default_api_prefix")]
    pub api_prefix: String,
}

impl Default for Http {
    fn default() -> Self {
        Self {
            listen: default_http_listen(),
            public: Default::default(),
            api_prefix: default_api_prefix(),
        }
    }
}

fn default_http_listen() -> SocketAddr {
    SocketAddr::from_str(&format!(
        "0.0.0.0:{}",
        env::var("PORT").unwrap_or(String::from("4001"))
    ))
    .expect("invalid listen address")
}

fn default_api_prefix() -> String {
    "/api/v1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_log_level() -> String {
    env::var("LOG_LEVEL").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    })
}

/// JWT verification settings. Must share `secret` and `exp_seconds` with cobotsrv
/// (cobotrix only verifies tokens, never issues them in Phase 0).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Auth {
    #[serde(default = "default_auth_secret")]
    pub secret: String,
    #[serde(default = "default_auth_expire_seconds")]
    pub exp_seconds: i64,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            secret: default_auth_secret(),
            exp_seconds: default_auth_expire_seconds(),
        }
    }
}

fn default_auth_secret() -> String {
    env::var("AUTH_SECRET")
        .unwrap_or_else(|_| "qwertyuiop.asdfghjkl.zxcvbnm.1234567890-=".to_string())
}

fn default_auth_expire_seconds() -> i64 {
    7200
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Database {
    #[serde(default = "default_db_url")]
    pub url: String,
    #[serde(default = "default_db_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_db_connect_timeout")]
    pub connect_timeout: u64,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            url: default_db_url(),
            max_connections: default_db_max_connections(),
            connect_timeout: default_db_connect_timeout(),
        }
    }
}

fn default_db_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string())
}

fn default_db_max_connections() -> u32 {
    10
}

fn default_db_connect_timeout() -> u64 {
    30
}

use std::{env, net::SocketAddr, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub http: Http,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub oidc: Oidc,
    #[serde(default)]
    pub log: Log,
    #[serde(default)]
    pub database: Database,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Http {
    #[serde(default = "default_http_listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub public: String,
    #[serde(default = "default_api_prefix")]
    pub api_prefix: String,
    /// Backend URL for cobotrix management center (auth proxy target).
    #[serde(default = "default_cobotrix_url")]
    pub cobotrix_url: String,
}

fn default_cobotrix_url() -> String {
    "http://127.0.0.1:4001".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Auth {
    #[serde(default = "default_auth_secret")]
    pub secret: String,
    #[serde(default)]
    pub tokens: Vec<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default = "default_auth_expire_seconds")]
    pub exp_seconds: i64, // set token exp time, unit :seconds, default 2h
    #[serde(default = "default_refresh_expire_seconds")]
    pub refresh_exp_seconds: i64,
    #[serde(default = "default_origins")]
    pub origins: Vec<String>,
    #[serde(default = "default_super_admin")]
    pub super_admin: Account,
    /// Path to RSA private key PEM file for OIDC ID Token signing.
    /// Defaults to `conf/oidc_rsa_private.pem`. If the file does not exist, auto-generates.
    #[serde(default = "default_rsa_private_key_path")]
    pub rsa_private_key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    #[serde(default = "default_log_level")]
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_http_listen() -> SocketAddr {
    SocketAddr::from_str(&format!(
        "0.0.0.0:{}",
        env::var("PORT").unwrap_or(String::from("4000"))
    ))
    .expect("invalid listen address")
}

fn default_api_prefix() -> String {
    "/api/v1".to_string()
}

impl Default for Http {
    fn default() -> Self {
        Self {
            listen: default_http_listen(),
            public: Default::default(),
            api_prefix: default_api_prefix(),
            cobotrix_url: default_cobotrix_url(),
        }
    }
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

fn default_auth_secret() -> String {
    env::var("AUTH_SECRET")
        .unwrap_or_else(|_| "qwertyuiop.asdfghjkl.zxcvbnm.1234567890-=".to_string())
}

fn default_auth_expire_seconds() -> i64 {
    7200
}

fn default_refresh_expire_seconds() -> i64 {
    7 * 24 * 3600
}

fn default_super_admin() -> Account {
    Account {
        username: "sadmin".to_string(),
        password: "sadmin".to_string(),
    }
}

fn default_origins() -> Vec<String> {
    vec!["http://localhost:5173".to_string()]
}

fn default_rsa_private_key_path() -> String {
    "conf/oidc_rsa_private.local.pem".to_string()
}

// ---------------------------------------------------------------------------
// OIDC Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oidc {
    /// Public-facing issuer URL for OIDC discovery document.
    /// If empty, derived from `http.listen` with wildcard addresses replaced by localhost.
    /// Example: "https://auth.example.com"
    #[serde(default)]
    pub issuer: String,

    /// Path to RSA private key PEM file for ID Token signing (RS256).
    /// If the file does not exist, a 2048-bit key pair will be auto-generated.
    #[serde(default = "default_rsa_private_key_path")]
    pub rsa_private_key_path: String,

    /// Access token lifetime in seconds (default: uses auth.exp_seconds for backward compat).
    /// If set, overrides auth.exp_seconds for OIDC tokens specifically.
    #[serde(default)]
    pub access_token_expire_seconds: Option<i64>,

    /// Refresh token lifetime in seconds (default: uses auth.refresh_exp_seconds for backward compat).
    #[serde(default)]
    pub refresh_token_expire_seconds: Option<i64>,

    /// Default email domain used when a user has no email set.
    /// UserInfo will return "{username}@{email_domain}".
    #[serde(default = "default_oidc_email_domain")]
    pub email_domain: String,
}

impl Default for Oidc {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            rsa_private_key_path: default_rsa_private_key_path(),
            access_token_expire_seconds: None,
            refresh_token_expire_seconds: None,
            email_domain: default_oidc_email_domain(),
        }
    }
}

fn default_oidc_email_domain() -> String {
    "cobotrix.local".to_string()
}

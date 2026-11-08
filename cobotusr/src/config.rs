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
    #[serde(default)]
    pub security_clock: SecurityClock,
}

/// The deployment's trusted NTP sources. Clock checks never adjust the OS clock.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SecurityClock {
    pub servers: Vec<String>,
    pub min_agreeing_sources: usize,
    pub poll_interval_seconds: u64,
    pub sample_ttl_seconds: u64,
    pub timeout_millis: u64,
}

impl Default for SecurityClock {
    fn default() -> Self {
        Self {
            servers: vec![
                "time.apple.com:123".into(),
                "time.cloudflare.com:123".into(),
                "ntp.aliyun.com:123".into(),
            ],
            min_agreeing_sources: 2,
            poll_interval_seconds: 10,
            sample_ttl_seconds: 30,
            timeout_millis: 1500,
        }
    }
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Auth {
    /// Explicit local HTTP development mode; listener and issuer must be loopback.
    #[serde(default)]
    pub allow_insecure_loopback_cookies: bool,
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

impl Config {
    pub fn validate_browser_cookie_policy(&self) -> anyhow::Result<()> {
        if !self.auth.allow_insecure_loopback_cookies {
            return Ok(());
        }
        anyhow::ensure!(
            self.http.listen.ip().is_loopback(),
            "insecure development cookies require a loopback HTTP listener"
        );
        let issuer = if !self.oidc.issuer.is_empty() {
            self.oidc.issuer.clone()
        } else if !self.http.public.is_empty() {
            self.http.public.clone()
        } else {
            format!("http://{}", self.http.listen)
        };
        let url = url::Url::parse(&issuer)?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https"),
            "invalid loopback issuer scheme"
        );
        anyhow::ensure!(
            url.host_str().is_some_and(is_loopback_host),
            "insecure development cookies require a loopback issuer"
        );
        Ok(())
    }
}

pub(crate) fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
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

    /// Algorithm used to sign access tokens. Defaults to RS256 (asymmetric, JWKS-friendly).
    /// Use "HS256" for tests / local dev where loading an RSA key is overhead — falls back
    /// to `auth.secret` as the HMAC key. Production should always be RS256.
    #[serde(default = "default_signing_algorithm")]
    pub signing_algorithm: String,

    /// OIDC clients to auto-register at startup. Existing entries (matched by `client_id`)
    /// are left untouched — secrets are NEVER overwritten by config to avoid silent
    /// rotation. Use the admin API to rotate secrets explicitly.
    #[serde(default)]
    pub bootstrap_clients: Vec<BootstrapClient>,

    /// Maximum lifetime (days) for long-lived MCP access tokens minted via
    /// `POST /api/v1/oidc/mcp-tokens`. Requested TTL is clamped to this. Default 90.
    #[serde(default = "default_mcp_token_max_ttl_days")]
    pub mcp_token_max_ttl_days: u32,
}

/// One row in `[[oidc.bootstrap_clients]]`. See `Oidc::bootstrap_clients`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapClient {
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    #[serde(default = "default_bootstrap_scopes")]
    pub allowed_scopes: Vec<String>,
    #[serde(default = "default_bootstrap_grants")]
    pub grant_types: Vec<String>,
    /// If absent, a random secret is generated on first run and printed once
    /// in the startup log so the operator can capture it. Set explicitly for
    /// stable dev/test secrets.
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Public SPA clients authenticate authorization-code exchanges with PKCE
    /// and never receive a client secret. They may use their client_id for
    /// refresh-token and revocation requests.
    #[serde(default)]
    pub public_client: bool,
}

fn default_bootstrap_scopes() -> Vec<String> {
    vec!["openid".into(), "profile".into(), "email".into()]
}

fn default_bootstrap_grants() -> Vec<String> {
    vec!["authorization_code".into(), "refresh_token".into()]
}

fn default_signing_algorithm() -> String {
    "RS256".to_string()
}

fn default_mcp_token_max_ttl_days() -> u32 {
    90
}

impl Default for Oidc {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            rsa_private_key_path: default_rsa_private_key_path(),
            access_token_expire_seconds: None,
            refresh_token_expire_seconds: None,
            email_domain: default_oidc_email_domain(),
            signing_algorithm: default_signing_algorithm(),
            bootstrap_clients: Vec::new(),
            mcp_token_max_ttl_days: default_mcp_token_max_ttl_days(),
        }
    }
}

fn default_oidc_email_domain() -> String {
    "cobotrix.local".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insecure_cookie_mode_requires_loopback_listener_and_issuer() {
        let mut config = Config::default();
        assert!(!config.auth.allow_insecure_loopback_cookies);
        assert!(config.validate_browser_cookie_policy().is_ok());
        config.auth.allow_insecure_loopback_cookies = true;
        config.http.listen = "0.0.0.0:4000".parse().unwrap();
        assert!(config.validate_browser_cookie_policy().is_err());
        config.http.listen = "127.0.0.1:4000".parse().unwrap();
        assert!(config.validate_browser_cookie_policy().is_ok());
        for issuer in [
            "http://localhost:4000",
            "http://127.0.0.2:4000",
            "http://[::1]:4000",
        ] {
            config.oidc.issuer = issuer.into();
            assert!(config.validate_browser_cookie_policy().is_ok(), "{issuer}");
        }
        for issuer in [
            "https://auth.example.test",
            "http://localhost.example.test",
            "http://192.168.1.2:4000",
            "file:///tmp/auth",
        ] {
            config.oidc.issuer = issuer.into();
            assert!(config.validate_browser_cookie_policy().is_err(), "{issuer}");
        }
        config.oidc.issuer.clear();
        config.http.public = "https://auth.example.test".into();
        assert!(config.validate_browser_cookie_policy().is_err());
        config.http.public.clear();
        config.http.listen = "[::]:4000".parse().unwrap();
        assert!(config.validate_browser_cookie_policy().is_err());
    }
}

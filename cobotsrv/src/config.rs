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

    #[cfg(feature = "easytier")]
    #[serde(default)]
    pub easytier: Option<EasytierConfig>,

    #[serde(default)]
    pub mqtt: Option<Mqtt>,

    #[serde(default)]
    pub task: Task,

    #[serde(default)]
    pub nodes: Vec<Node>,

    /// Permissions that cobotsrv requires. At startup cobotsrv syncs these to
    /// cobotusr so the admin can assign them to roles without manual creation.
    /// Each entry: `{ name, action, resource_type?, resource_pattern? }`
    #[serde(default)]
    pub permissions: Vec<PermissionSpec>,

    /// MCP Server configuration.
    #[serde(default)]
    pub mcp: Mcp,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Mcp {
    /// Maximum time (ms) to wait for a tools/call result from cobotlet via MQTT.
    #[serde(default = "default_mcp_call_timeout_ms")]
    pub call_timeout_ms: u64,
}

impl Default for Mcp {
    fn default() -> Self {
        Self {
            call_timeout_ms: default_mcp_call_timeout_ms(),
        }
    }
}

fn default_mcp_call_timeout_ms() -> u64 {
    30_000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PermissionSpec {
    /// Canonical name, e.g. "device:read", "task:execute"
    pub name: String,
    /// Verb / capability, e.g. "read", "submit"
    pub action: String,
    /// Object class, e.g. "device", "task". None = action only.
    #[serde(default)]
    pub resource_type: Option<String>,
    /// Glob pattern, e.g. "*", "cobotlet-0:*". Default: "*"
    #[serde(default = "default_resource_pattern")]
    pub resource_pattern: Option<String>,
}

fn default_resource_pattern() -> Option<String> {
    Some("*".to_string())
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

#[cfg(feature = "easytier")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EasytierConfig {
    #[serde(default)]
    pub network_name: String,
    #[serde(default)]
    pub network_secret: String,
    #[serde(default)]
    pub instance_name: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub ipv4: String,
    #[serde(default)]
    pub ipv6: String,
    #[serde(default)]
    pub listeners: Vec<String>,
    #[serde(default)]
    pub peers: Vec<String>,
    /// Shared secret for cobotlet registration/config-fetch.
    #[serde(default)]
    pub register_token: String,
    /// Public endpoint of this cobotsrv, used as the peer address
    /// in configs distributed to cobotlets.
    #[serde(default)]
    pub public_endpoint: String,
    /// IP pool configuration for the easytier virtual network.
    #[serde(default)]
    pub ip_pool: IpPoolConfig,
}

/// IP pool configuration for easytier virtual network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpPoolConfig {
    /// Subnet prefix (e.g. "10.100" → "10.100.0.0/24").
    /// Leave empty to auto-derive from `easytier.ipv4`.
    #[serde(default)]
    pub prefix: String,
    /// First assignable host octet (default 10)
    #[serde(default = "default_ip_pool_start")]
    pub start: u8,
    /// Last assignable host octet (default 250)
    #[serde(default = "default_ip_pool_end")]
    pub end: u8,
    /// Lease duration in days
    #[serde(default = "default_lease_days")]
    pub lease_days: u32,
}

fn default_ip_pool_start() -> u8 {
    10
}
fn default_ip_pool_end() -> u8 {
    250
}
fn default_lease_days() -> u32 {
    30
}

impl Default for IpPoolConfig {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            start: default_ip_pool_start(),
            end: default_ip_pool_end(),
            lease_days: default_lease_days(),
        }
    }
}

impl IpPoolConfig {
    /// Resolve the effective pool prefix.
    /// If `prefix` is explicitly set, use it; otherwise derive from cobotsrv's own IPv4.
    pub fn resolve_prefix(&self, own_ipv4: &str) -> String {
        if !self.prefix.is_empty() {
            return self.prefix.clone();
        }
        // Extract the first 3 octets from cobotsrv's IPv4.
        // "10.126.126.1" → "10.126.126"
        if let Some(pos) = own_ipv4.rfind('.') {
            own_ipv4[..pos].to_string()
        } else {
            "10.100.0".to_string()
        }
    }
}

#[cfg(feature = "easytier")]
impl EasytierConfig {
    pub fn into_cobotnet_config(self) -> cobotnet::Config {
        cobotnet::Config {
            network_name: self.network_name,
            network_secret: self.network_secret,
            instance_name: self.instance_name,
            hostname: self.hostname,
            ipv4: self.ipv4,
            ipv6: self.ipv6,
            listeners: self.listeners,
            peers: self.peers,
        }
    }

    pub fn validate(&mut self) {
        if self.instance_name.is_empty() {
            self.instance_name = uuid::Uuid::new_v4().to_string().chars().take(8).collect();
        }
        if self.hostname.is_empty() {
            self.hostname = self.instance_name.clone();
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Http {
    #[serde(default = "default_http_listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub public: String,
    /// Allowed CORS origins. Default: `["*"]`.
    /// Set to specific origins if cobotsrv is directly exposed.
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,
}

impl Default for Http {
    fn default() -> Self {
        Self {
            listen: default_http_listen(),
            public: Default::default(),
            cors_origins: default_cors_origins(),
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

fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
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

/// JWT verification settings. cobotsrv is an OIDC client of cobotusr: it pulls
/// cobotusr's public key from the JWKS endpoint at startup and uses it to verify
/// every incoming Bearer token. No shared secret — Phase 1 (RS256) of the auth
/// roadmap.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Auth {
    /// OIDC client_id registered at cobotusr. Used for client_credentials grant
    /// when syncing permissions at startup. Default: `"cobotsrv"`.
    #[serde(default = "default_client_id")]
    pub client_id: String,
    /// OIDC client_secret matching cobotusr's bootstrap client config.
    /// Required for client_credentials grant.
    #[serde(default)]
    pub client_secret: String,
    /// JWKS endpoint URL. Default: `{cobotusr_base}/.well-known/jwks.json`.
    #[serde(default = "default_jwks_url")]
    pub jwks_url: String,
    /// If set, only tokens with `aud` matching this value are accepted.
    /// Defaults to `"cobotsrv"` — cobotsrv only accepts tokens issued for itself.
    #[serde(default = "default_expected_audience")]
    pub expected_audience: String,
    /// JWKS background refresh interval in seconds (default: 5 min).
    #[serde(default = "default_jwks_refresh_secs")]
    pub jwks_refresh_secs: u64,
    /// When true, route-level `require_perm!` layers are applied on protected routes.
    /// Default true — cobotsrv enforces permissions based on token claims.
    #[serde(default = "default_require_permissions")]
    pub require_permissions: bool,
}

impl Auth {
    /// Derive cobotusr's base URL from the JWKS URL by stripping the
    /// `/.well-known/jwks.json` suffix. Used by `EnforceClient` to call
    /// cobotusr's PDP enforce endpoint.
    pub fn cobotusr_base_url(&self) -> String {
        self.jwks_url
            .trim_end_matches("/.well-known/jwks.json")
            .to_string()
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            client_id: default_client_id(),
            client_secret: String::new(),
            jwks_url: default_jwks_url(),
            expected_audience: default_expected_audience(),
            jwks_refresh_secs: default_jwks_refresh_secs(),
            require_permissions: default_require_permissions(),
        }
    }
}

fn default_jwks_url() -> String {
    env::var("COBOTSRV_JWKS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4000/.well-known/jwks.json".to_string())
}

fn default_client_id() -> String {
    "cobotsrv".to_string()
}

fn default_expected_audience() -> String {
    "cobotsrv".to_string()
}

fn default_require_permissions() -> bool {
    true
}

fn default_jwks_refresh_secs() -> u64 {
    300
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

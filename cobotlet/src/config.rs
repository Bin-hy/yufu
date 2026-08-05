use std::collections::HashMap;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

fn default_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 3080))
}

fn default_log_level() -> String {
    String::from("info")
}

fn default_heartbeat_timeout_secs() -> u64 {
    30
}

fn default_request_timeout_ms() -> u64 {
    5000
}

fn default_watchdog_interval_secs() -> u64 {
    5
}

fn default_keep_alive_secs() -> u64 {
    30
}

fn default_topic_prefix() -> String {
    String::from("cobotrix")
}

fn default_client_id() -> String {
    String::from("{alias}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http {
    #[serde(default = "default_listen")]
    pub listen: SocketAddr,
}

impl Default for Http {
    fn default() -> Self {
        Self {
            listen: default_listen(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Log {
    #[serde(default = "default_log_level")]
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Device {
    #[serde(default = "default_heartbeat_timeout_secs")]
    pub heartbeat_timeout_secs: u64,
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(default = "default_watchdog_interval_secs")]
    pub watchdog_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mqtt {
    pub url: String,
    pub alias: String,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
    #[serde(default = "default_keep_alive_secs")]
    pub keep_alive_secs: u64,
}

impl Mqtt {
    pub fn resolved_client_id(&self) -> String {
        self.client_id.replace("{alias}", &self.alias)
    }

    pub fn topic_root(&self) -> String {
        format!("{}/cobotlets/{}", self.topic_prefix, self.alias)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Net4mqtt {
    pub alias: String,
    #[serde(default)]
    pub mqtt_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ControlTemplate {
    #[default]
    Dpad,
    DualStick,
    ButtonMatrix,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    #[default]
    PressRelease,
    Continuous,
    FireOnce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlBinding {
    pub capability: String,
    #[serde(default)]
    pub mode: InteractionMode,
    #[serde(default)]
    pub interval_ms: Option<u64>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    #[serde(default)]
    pub release_params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ControlPreset {
    pub template: ControlTemplate,
    #[serde(default)]
    pub mapping: HashMap<String, ControlBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub http: Http,
    #[serde(default)]
    pub log: Log,
    #[serde(default)]
    pub storage: Option<Storage>,
    #[serde(default)]
    pub device: Device,
    #[serde(default)]
    pub mqtt: Option<Mqtt>,
    #[serde(default)]
    pub net4mqtt: Option<Net4mqtt>,
    #[serde(default)]
    pub control_preset: Option<ControlPreset>,
    #[cfg(feature = "easytier")]
    #[serde(default)]
    pub easytier: Option<EasytierConfig>,
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
    /// cobotsrv config distribution endpoint.
    /// If set and network_name is empty, cobotlet will auto-fetch
    /// easytier config from cobotsrv instead of using local config.
    #[serde(default)]
    pub server_url: Option<String>,
    /// Device serial number used to identify this device to cobotsrv.
    #[serde(default)]
    pub robot_sn: Option<String>,
    /// Shared secret to authenticate with cobotsrv config endpoint.
    #[serde(default)]
    pub register_token: Option<String>,
}

#[cfg(feature = "easytier")]
impl EasytierConfig {
    /// Whether this config should skip local config and fetch from cobotsrv instead.
    pub fn should_fetch_remote(&self) -> bool {
        self.network_name.is_empty() && self.server_url.is_some()
    }

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

impl Default for Config {
    fn default() -> Self {
        Self {
            http: Http {
                listen: default_listen(),
            },
            log: Log {
                level: default_log_level(),
            },
            storage: None,
            device: Device {
                heartbeat_timeout_secs: default_heartbeat_timeout_secs(),
                request_timeout_ms: default_request_timeout_ms(),
                watchdog_interval_secs: default_watchdog_interval_secs(),
            },
            mqtt: None,
            net4mqtt: None,
            control_preset: None,
            #[cfg(feature = "easytier")]
            easytier: None,
        }
    }
}

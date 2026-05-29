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
        }
    }
}

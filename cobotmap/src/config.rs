use crate::ProxyConfig;
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub http: Http,
    #[serde(default)]
    pub maps: Maps,
    #[serde(default)]
    pub proxy: Proxy,
    #[serde(default)]
    pub log: Log,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Maps {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub input: Option<PathBuf>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Proxy {
    #[serde(default)]
    pub url: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
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

fn default_listen() -> SocketAddr {
    "127.0.0.1:8080"
        .parse()
        .expect("invalid cobotmap listen address")
}

fn default_log_level() -> String {
    std::env::var("LOG_LEVEL").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    })
}

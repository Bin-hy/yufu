use serde::{Deserialize, Serialize};

fn default_cobotlet_url() -> String {
    String::from("http://localhost:3080")
}

fn default_method() -> String {
    String::from("POST")
}

fn default_log_level() -> String {
    String::from("warn")
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

#[derive(Debug, Clone, Deserialize)]
pub struct Cobotlet {
    #[serde(default = "default_cobotlet_url")]
    pub url: String,
}

impl Default for Cobotlet {
    fn default() -> Self {
        Self {
            url: default_cobotlet_url(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityConfig {
    pub name: String,
    #[serde(default = "default_method")]
    pub method: String,
    pub path: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    pub id: Option<String>,
    pub service_url: String,
    #[serde(default)]
    pub capabilities: Vec<CapabilityConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub log: Log,
    #[serde(default)]
    pub cobotlet: Cobotlet,
    pub device: Device,
}

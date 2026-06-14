use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
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
}

impl Config {
    pub fn validate(&mut self) {
        if self.instance_name.is_empty() {
            self.instance_name = uuid::Uuid::new_v4().to_string().chars().take(8).collect();
        }
        if self.hostname.is_empty() {
            self.hostname = self.instance_name.clone();
        }
    }
}

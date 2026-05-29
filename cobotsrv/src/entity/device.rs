use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::IdType;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "device")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    pub cobotlet_alias: String,
    pub device_id: String,
    pub service_url: String,
    /// Capabilities serialized as JSON.
    pub capabilities: Json,
    pub online: bool,
    pub last_heartbeat: DateTime,
    pub registered_at: DateTime,
    pub update_time: DateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_method")]
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, alias = "schema")]
    pub input_schema: Option<serde_json::Value>,
}

fn default_method() -> String {
    String::from("POST")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceVO {
    pub id: IdType,
    pub cobotlet_alias: String,
    pub device_id: String,
    pub service_url: String,
    pub capabilities: Vec<Capability>,
    pub online: bool,
    pub last_heartbeat: String,
    pub registered_at: String,
}

impl From<Model> for DeviceVO {
    fn from(m: Model) -> Self {
        let capabilities: Vec<Capability> =
            serde_json::from_value(m.capabilities).unwrap_or_default();
        Self {
            id: m.id,
            cobotlet_alias: m.cobotlet_alias,
            device_id: m.device_id,
            service_url: m.service_url,
            capabilities,
            online: m.online,
            last_heartbeat: m.last_heartbeat.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            registered_at: m.registered_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}

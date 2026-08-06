use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::IdType;

/// Audit record for an MCP `tools/call` invocation.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "mcp_call_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    pub created_at: DateTime,
    /// JWT `sub` (user id) of the caller.
    pub subject: String,
    /// JSON-RPC method, e.g. "tools/call".
    pub method: String,
    pub tool_name: Option<String>,
    /// Error message if the call failed. `None` means success.
    /// This single field encodes both error status and detail.
    pub error_msg: Option<String>,
    pub duration_ms: i64,
    pub request_id: Option<String>,
    pub input_params: Option<Json>,
    pub status_code: Option<i32>,
    pub device_id: Option<String>,
    pub cobotlet_alias: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpCallLogVO {
    pub id: IdType,
    pub created_at: String,
    pub subject: String,
    pub method: String,
    pub tool_name: Option<String>,
    pub error_msg: Option<String>,
    pub duration_ms: i64,
    pub request_id: Option<String>,
    pub input_params: Option<serde_json::Value>,
    pub status_code: Option<i32>,
    pub device_id: Option<String>,
    pub cobotlet_alias: Option<String>,
    pub source: Option<String>,
}

impl From<Model> for McpCallLogVO {
    fn from(m: Model) -> Self {
        Self {
            id: m.id,
            created_at: m
                .created_at
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            subject: m.subject,
            method: m.method,
            tool_name: m.tool_name,
            error_msg: m.error_msg,
            duration_ms: m.duration_ms,
            request_id: m.request_id,
            input_params: m.input_params,
            status_code: m.status_code,
            device_id: m.device_id,
            cobotlet_alias: m.cobotlet_alias,
            source: m.source,
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}

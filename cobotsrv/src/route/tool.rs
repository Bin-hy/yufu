use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::Serialize;

use crate::AppState;
use crate::entity::device::Capability;
use crate::error::AppError;
use crate::result::Result;
use crate::service::device::DeviceService;
use auth::require_perm;

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/tools", get(list_tools).layer(require_perm!("device:read")))
        .route("/tools/{name}", get(get_tool).layer(require_perm!("device:read")))
}

#[derive(Debug, Serialize)]
pub struct ToolProvider {
    pub cobotlet_alias: String,
    pub device_id: String,
    pub online: bool,
}

#[derive(Debug, Serialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub schema: Option<serde_json::Value>,
    pub providers: Vec<ToolProvider>,
}

#[derive(Debug, Serialize)]
pub struct ToolListResponse {
    pub tools: Vec<ToolDescriptor>,
}

async fn list_tools(State(state): State<AppState>) -> Result<Json<ToolListResponse>> {
    let devices = DeviceService::list(&state.database.connection).await?;
    let mut seen: HashMap<String, ToolDescriptor> = HashMap::new();
    for device in &devices {
        let caps: Vec<Capability> =
            serde_json::from_value(device.capabilities.clone()).unwrap_or_default();
        for cap in caps {
            let entry = seen.entry(cap.name.clone()).or_insert_with(|| ToolDescriptor {
                name: cap.name.clone(),
                method: cap.method.clone(),
                path: cap.path.clone(),
                tags: cap.tags.clone(),
                schema: cap.schema.clone(),
                providers: Vec::new(),
            });
            entry.providers.push(ToolProvider {
                cobotlet_alias: device.cobotlet_alias.clone(),
                device_id: device.device_id.clone(),
                online: device.online,
            });
        }
    }
    let mut tools: Vec<ToolDescriptor> = seen.into_values().collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(ToolListResponse { tools }))
}

async fn get_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ToolDescriptor>> {
    let devices = DeviceService::list_filtered(
        &state.database.connection,
        None,
        None,
        Some(&name),
    )
    .await?;

    if devices.is_empty() {
        return Err(AppError::not_found(format!("tool '{name}' not found")));
    }

    // Use the first occurrence for the tool's own metadata.
    let mut descriptor: Option<ToolDescriptor> = None;
    for device in &devices {
        let caps: Vec<Capability> =
            serde_json::from_value(device.capabilities.clone()).unwrap_or_default();
        for cap in caps {
            if cap.name != name {
                continue;
            }
            let d = descriptor.get_or_insert_with(|| ToolDescriptor {
                name: cap.name.clone(),
                method: cap.method.clone(),
                path: cap.path.clone(),
                tags: cap.tags.clone(),
                schema: cap.schema.clone(),
                providers: Vec::new(),
            });
            d.providers.push(ToolProvider {
                cobotlet_alias: device.cobotlet_alias.clone(),
                device_id: device.device_id.clone(),
                online: device.online,
            });
        }
    }

    Ok(Json(descriptor.unwrap()))
}

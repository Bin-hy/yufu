use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Deserialize;

use crate::AppState;
use crate::entity::device::DeviceVO;
use crate::error::AppError;
use crate::result::Result;
use crate::service::device::DeviceService;
use auth::require_perm;

#[derive(Debug, Default, Deserialize)]
struct DeviceQuery {
    pub online: Option<bool>,
    pub capability: Option<String>,
}

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/devices", get(list).layer(require_perm!("device:read")))
        .route(
            "/devices/{cobotlet_alias}",
            get(list_by_alias).layer(require_perm!("device:read")),
        )
        .route(
            "/devices/{cobotlet_alias}/{device_id}",
            get(get_one).layer(require_perm!("device:read")),
        )
}

async fn list(
    State(state): State<AppState>,
    Query(q): Query<DeviceQuery>,
) -> Result<Json<Vec<DeviceVO>>> {
    let devices = DeviceService::list_filtered(
        &state.database.connection,
        None,
        q.online,
        q.capability.as_deref(),
    )
    .await?;
    Ok(Json(devices.into_iter().map(DeviceVO::from).collect()))
}

async fn list_by_alias(
    State(state): State<AppState>,
    Path(alias): Path<String>,
    Query(q): Query<DeviceQuery>,
) -> Result<Json<Vec<DeviceVO>>> {
    let devices = DeviceService::list_filtered(
        &state.database.connection,
        Some(&alias),
        q.online,
        q.capability.as_deref(),
    )
    .await?;
    Ok(Json(devices.into_iter().map(DeviceVO::from).collect()))
}

async fn get_one(
    State(state): State<AppState>,
    Path((alias, device_id)): Path<(String, String)>,
) -> Result<Json<DeviceVO>> {
    match DeviceService::find(&state.database.connection, &alias, &device_id).await? {
        Some(d) => Ok(Json(DeviceVO::from(d))),
        None => Err(AppError::not_found(format!(
            "device {alias}/{device_id} not found"
        ))),
    }
}

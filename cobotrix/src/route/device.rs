use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::AppState;
use crate::entity::device::DeviceVO;
use crate::error::AppError;
use crate::result::Result;
use crate::service::device::DeviceService;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/devices", get(list))
        .route("/devices/{cobotlet_alias}", get(list_by_alias))
        .route("/devices/{cobotlet_alias}/{device_id}", get(get_one))
}

async fn list(State(state): State<AppState>) -> Result<Json<Vec<DeviceVO>>> {
    let devices = DeviceService::list(&state.database.connection).await?;
    Ok(Json(devices.into_iter().map(DeviceVO::from).collect()))
}

async fn list_by_alias(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Result<Json<Vec<DeviceVO>>> {
    let devices = DeviceService::list_by_alias(&state.database.connection, &alias).await?;
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

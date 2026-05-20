use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::result::Result;
use crate::store::{Capability, Device};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub service_url: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub id: String,
}

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/devices", post(register).get(list))
        .route("/devices/{id}", get(get_device).delete(unregister))
        .route("/devices/{id}/heartbeat", put(heartbeat))
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>)> {
    if req.service_url.is_empty() {
        return Err(AppError::throw("service_url is required"));
    }
    let now = Utc::now();
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let device = Device {
        id: id.clone(),
        service_url: req.service_url,
        capabilities: req.capabilities,
        last_heartbeat: now,
        registered_at: now,
        online: true,
    };
    let device = state.store.register(device)?;
    state.events.publish_online(&device);
    Ok((StatusCode::CREATED, Json(RegisterResponse { id })))
}

async fn heartbeat(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    match state.store.heartbeat(&id)? {
        Some((device, was_offline)) => {
            if was_offline {
                state.events.publish_online(&device);
            } else {
                state.events.publish_heartbeat(&device.id);
            }
            Ok(StatusCode::NO_CONTENT)
        }
        None => Err(AppError::not_found(format!("device {id} not found"))),
    }
}

async fn list(State(state): State<AppState>) -> Json<Vec<Device>> {
    Json(state.store.list())
}

async fn get_device(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Device>> {
    match state.store.get(&id) {
        Some(d) => Ok(Json(d)),
        None => Err(AppError::not_found(format!("device {id} not found"))),
    }
}

async fn unregister(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    match state.store.remove(&id)? {
        Some(device) => {
            state.events.publish_offline(&device);
            Ok(StatusCode::NO_CONTENT)
        }
        None => Err(AppError::not_found(format!("device {id} not found"))),
    }
}

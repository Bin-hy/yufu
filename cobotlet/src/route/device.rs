use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post, put},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::result::Result;
use crate::store::{Capability, Device, DeviceState};

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

#[derive(Debug, Serialize)]
pub struct HeartbeatResponse {
    pub state: &'static str,
    pub restored: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub state: Option<String>,
    pub capability: Option<String>,
}

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/devices", post(register).get(list))
        .route("/devices/{id}", get(get_device).delete(unregister))
        .route("/devices/{id}/heartbeat", put(heartbeat))
        .route("/devices/{id}/capabilities", patch(update_capabilities))
        .route("/devices/{id}/state", patch(set_state))
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
        state: DeviceState::Registering,
    };
    let device = state.store.register(device)?;
    state.events.publish_online(&device);
    Ok((StatusCode::CREATED, Json(RegisterResponse { id })))
}

async fn heartbeat(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<HeartbeatResponse>> {
    match state.store.heartbeat(&id)? {
        Some((device, old_state)) => {
            let restored = matches!(
                old_state,
                DeviceState::Registering | DeviceState::Offline | DeviceState::Error { .. }
            );
            if restored {
                state.events.publish_online(&device);
            } else {
                state.events.publish_heartbeat(&device.id);
            }
            Ok(Json(HeartbeatResponse {
                state: device.state.as_str(),
                restored,
            }))
        }
        None => Err(AppError::not_found(format!("device '{id}' not found"))),
    }
}

async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Json<Vec<Device>> {
    let devices = state.store.list();
    let filtered = devices
        .into_iter()
        .filter(|d| {
            q.state
                .as_ref()
                .is_none_or(|s| d.state.as_str() == s.as_str())
        })
        .filter(|d| {
            q.capability
                .as_ref()
                .is_none_or(|cap| d.capabilities.iter().any(|c| &c.name == cap))
        })
        .collect();
    Json(filtered)
}

async fn get_device(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Device>> {
    match state.store.get(&id) {
        Some(d) => Ok(Json(d)),
        None => Err(AppError::not_found(format!("device '{id}' not found"))),
    }
}

async fn update_capabilities(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(caps): Json<Vec<Capability>>,
) -> Result<Json<Device>> {
    match state.store.update_capabilities(&id, caps)? {
        Some(device) => {
            state.events.publish_online(&device);
            Ok(Json(device))
        }
        None => Err(AppError::not_found(format!("device '{id}' not found"))),
    }
}

async fn unregister(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    match state.store.remove(&id)? {
        Some(device) => {
            state.events.publish_offline(&device);
            Ok(StatusCode::NO_CONTENT)
        }
        None => Err(AppError::not_found(format!("device '{id}' not found"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetStateRequest {
    pub state: String,
}

pub async fn set_state(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetStateRequest>,
) -> Result<Json<Device>> {
    let target = match req.state.as_str() {
        "maintenance" => DeviceState::Maintenance,
        "ready" => DeviceState::Ready,
        _ => return Err(AppError::throw("state must be 'maintenance' or 'ready'")),
    };
    match state.store.transition(&id, target) {
        Ok(Some(device)) => Ok(Json(device)),
        Ok(None) => Err(AppError::not_found(format!("device '{id}' not found"))),
        Err(e) => Err(AppError::throw(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use chrono::Utc;
    use http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    use crate::events::NoopSink;
    use crate::route::AppState;
    use crate::store::{Capability, Device, DeviceState, DeviceStore};

    fn make_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(redb::Database::create(dir.path().join("test.redb")).unwrap());
        let store = Arc::new(DeviceStore::new(db.clone()).unwrap());
        let task_store = Arc::new(crate::store::TaskStore::new(db));
        let state = AppState {
            config: crate::config::Config::default(),
            store,
            task_store,
            http_client: reqwest::Client::new(),
            events: Arc::new(NoopSink),
        };
        (state, dir)
    }

    fn registered_device(state: &AppState, id: &str, caps: &[&str]) -> Device {
        let now = Utc::now();
        let device = Device {
            id: id.to_string(),
            service_url: "http://robot:9000".to_string(),
            capabilities: caps
                .iter()
                .map(|name| Capability {
                    name: name.to_string(),
                    description: None,
                    method: "POST".to_string(),
                    path: format!("/{name}"),
                    tags: vec![],
                    input_schema: None,
                })
                .collect(),
            last_heartbeat: now,
            registered_at: now,
            state: DeviceState::Ready,
        };
        state.store.register(device.clone()).unwrap();
        device
    }

    #[tokio::test]
    async fn patch_capabilities_updates_device() {
        let (state, _dir) = make_state();
        registered_device(&state, "d1", &["move"]);

        let app = Router::new().merge(route()).with_state(state.clone());
        let new_caps = serde_json::json!([
            { "name": "grip", "path": "/grip", "method": "POST" }
        ]);
        let req = Request::builder()
            .method(Method::PATCH)
            .uri("/devices/d1/capabilities")
            .header("content-type", "application/json")
            .body(Body::from(new_caps.to_string()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let caps = json["capabilities"].as_array().unwrap();
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0]["name"], "grip");

        // Old capability is gone from the index.
        assert!(state.store.find_by_capability("move").is_empty());
        assert_eq!(state.store.find_by_capability("grip").len(), 1);
    }

    #[tokio::test]
    async fn patch_capabilities_unknown_device_returns_404() {
        let (state, _dir) = make_state();
        let app = Router::new().merge(route()).with_state(state);
        let req = Request::builder()
            .method(Method::PATCH)
            .uri("/devices/no-such-device/capabilities")
            .header("content-type", "application/json")
            .body(Body::from("[]"))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_devices_filters_by_state() {
        let (state, _dir) = make_state();
        registered_device(&state, "ready-dev", &["move"]);
        let now = Utc::now();
        state
            .store
            .register(Device {
                id: "offline-dev".to_string(),
                service_url: "http://offline:9000".to_string(),
                capabilities: vec![],
                last_heartbeat: now,
                registered_at: now,
                state: DeviceState::Offline,
            })
            .unwrap();

        let app = Router::new().merge(route()).with_state(state);
        let req = Request::builder()
            .uri("/devices?state=ready")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let devices = json.as_array().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0]["id"], "ready-dev");
    }

    #[tokio::test]
    async fn list_devices_filters_by_capability() {
        let (state, _dir) = make_state();
        registered_device(&state, "d-move", &["move"]);
        registered_device(&state, "d-grip", &["grip"]);

        let app = Router::new().merge(route()).with_state(state);
        let req = Request::builder()
            .uri("/devices?capability=move")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let devices = json.as_array().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0]["id"], "d-move");
    }
}

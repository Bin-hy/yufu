use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::mqtt::{Task, TaskResult};
use crate::result::Result;
<<<<<<< HEAD
use auth::require_perm;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", post(submit).layer(require_perm!("task:execute")))
        .route(
            "/tasks/{task_id}",
            get(get_result).layer(require_perm!("task:read")),
        )
=======

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", post(submit))
        .route("/tasks/{task_id}", get(get_result))
>>>>>>> fa2da33 (feat(cobotrix): add management center crate with self-verifying JWT auth)
}

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub cobotlet_alias: String,
    #[serde(default)]
    pub task_id: Option<String>,
    pub capability_name: String,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SubmitResponse {
    Sync(TaskResult),
    Async { task_id: String },
}

#[derive(Debug, Deserialize)]
pub struct SubmitQuery {
    #[serde(default = "default_wait")]
    pub wait: bool,
}

fn default_wait() -> bool {
    true
}

async fn submit(
    State(state): State<AppState>,
    Query(q): Query<SubmitQuery>,
    Json(req): Json<SubmitRequest>,
) -> Result<(StatusCode, Json<SubmitResponse>)> {
    let mqtt = state
        .mqtt
        .as_ref()
        .ok_or_else(|| AppError::throw("mqtt is not configured"))?;

    let task_id = req.task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let task = Task {
        id: task_id.clone(),
        capability_name: req.capability_name,
        device_id: req.device_id,
        headers: req.headers,
        body: req.body,
        timeout_ms: req.timeout_ms,
    };

    if q.wait {
        let timeout =
            Duration::from_millis(req.timeout_ms.unwrap_or(state.config.task.wait_timeout_ms));
        match mqtt.submit(&req.cobotlet_alias, task, timeout).await {
            Ok(result) => Ok((StatusCode::OK, Json(SubmitResponse::Sync(result)))),
            Err(e) => Err(AppError::throw(e.to_string())),
        }
    } else {
        mqtt.submit_async(&req.cobotlet_alias, task)
            .await
            .map_err(|e| AppError::throw(e.to_string()))?;
        Ok((
            StatusCode::ACCEPTED,
            Json(SubmitResponse::Async { task_id }),
        ))
    }
}

async fn get_result(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>> {
    let mqtt = state
        .mqtt
        .as_ref()
        .ok_or_else(|| AppError::throw("mqtt is not configured"))?;
    match mqtt.peek_result(&task_id) {
        Some(r) => Ok(Json(r)),
        None => Err(AppError::not_found(format!(
            "task result {task_id} not yet available or already evicted"
        ))),
    }
}

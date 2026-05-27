use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::mqtt::{Task, TaskPollResult, TaskResult};
use crate::result::Result;
use auth::require_perm;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", post(submit).layer(require_perm!("task:execute")))
        .route(
            "/tasks/{task_id}",
            get(poll).layer(require_perm!("task:read")),
        )
}

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub cobotlet_alias: String,
    #[serde(default)]
    pub task_id: Option<String>,
    /// MCP-style tool name. Accepts legacy `capability_name` for backward compat.
    #[serde(alias = "capability_name")]
    pub name: String,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// MCP-style tool input. Accepts legacy `body` for backward compat.
    #[serde(default, alias = "body")]
    pub input: Option<serde_json::Value>,
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
        name: req.name.clone(),
        device_id: req.device_id,
        headers: req.headers,
        input: req.input,
        timeout_ms: req.timeout_ms,
    };

    if q.wait {
        let timeout_ms = req.timeout_ms.unwrap_or(state.config.task.wait_timeout_ms);
        let timeout = Duration::from_millis(timeout_ms);
        match mqtt.submit(&req.cobotlet_alias, task, timeout).await {
            Ok(result) => Ok((StatusCode::OK, Json(SubmitResponse::Sync(result)))),
            Err(e) if e.to_string() == "timeout" => Err(AppError::throw(format!(
                "cobotlet '{}' did not respond within {}ms (capability: '{}'). \
                 Verify the cobotlet is online and the capability is available.",
                req.cobotlet_alias, timeout_ms, req.name
            ))),
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

/// Poll the result of a previously submitted task.
///
/// Returns:
/// - `200 OK` with `TaskResult` when the result is ready.
/// - `202 Accepted` with `{"status": "pending", "task_id": "..."}` while still processing.
/// - `404 Not Found` if the task ID is unknown or result was already evicted.
async fn poll(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse> {
    let mqtt = state
        .mqtt
        .as_ref()
        .ok_or_else(|| AppError::throw("mqtt is not configured"))?;
    match mqtt.poll_result(&task_id) {
        TaskPollResult::Ready(r) => Ok((StatusCode::OK, Json(serde_json::to_value(r).unwrap())).into_response()),
        TaskPollResult::Pending => Ok((
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "status": "pending", "task_id": task_id })),
        )
            .into_response()),
        TaskPollResult::NotFound => Err(AppError::not_found(format!(
            "task '{task_id}' not found — it may never have been submitted, \
             or the result expired after the cache window."
        ))),
    }
}

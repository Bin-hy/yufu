use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;

use crate::AppState;
use crate::dispatcher::{Task, TaskResult, dispatch};
use crate::error::AppError;
use crate::result::Result;
use crate::store::{TaskRecord, TaskState};

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/tasks", post(submit).get(list_tasks).delete(delete_all))
        .route("/tasks/{id}", get(get_task).delete(delete_one))
}

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    state: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn submit(State(state): State<AppState>, Json(task): Json<Task>) -> Result<Json<TaskResult>> {
    if let Err(e) = state.task_store.enqueue(&task) {
        tracing::warn!(task_id = task.id, error = %e, "task enqueue failed");
    }
    let result = dispatch(
        &state.store,
        &state.task_store,
        &state.http_client,
        state.config.device.request_timeout_ms,
        task,
    )
    .await;
    state.events.publish_task_result(&result);
    Ok(Json(result))
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<TaskRecord>>> {
    let filter = q.state.as_deref().and_then(|s| match s {
        "queued" => Some(TaskState::Queued),
        "dispatched" => Some(TaskState::Dispatched),
        "completed" => Some(TaskState::Completed),
        "failed" => Some(TaskState::Failed),
        _ => None,
    });
    // Cap the page size — the store scans the whole table in memory, so an
    // unbounded limit from an unauthenticated local HTTP client would be a
    // cheap amplification vector.
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0);
    let records = state.task_store.list(filter.as_ref(), limit, offset)?;
    Ok(Json(records))
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TaskRecord>> {
    match state.task_store.get(&id)? {
        Some(record) => Ok(Json(record)),
        None => Err(AppError::not_found(format!("task '{id}' not found"))),
    }
}

#[derive(serde::Serialize)]
struct DeleteResponse {
    deleted: u64,
}

async fn delete_all(State(state): State<AppState>) -> Result<Json<DeleteResponse>> {
    let deleted = state.task_store.delete_finished()?;
    Ok(Json(DeleteResponse { deleted }))
}

async fn delete_one(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    if state.task_store.delete(&id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found(format!("task '{id}' not found")))
    }
}

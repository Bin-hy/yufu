use axum::{Json, Router, extract::State, routing::post};

use crate::AppState;
use crate::dispatcher::{Task, TaskResult, dispatch};
use crate::result::Result;

pub fn route() -> Router<AppState> {
    Router::new().route("/tasks", post(submit))
}

async fn submit(State(state): State<AppState>, Json(task): Json<Task>) -> Result<Json<TaskResult>> {
    let result = dispatch(
        &state.store,
        &state.http_client,
        state.config.device.request_timeout_ms,
        task,
    )
    .await;
    state.events.publish_task_result(&result);
    Ok(Json(result))
}

use axum::{Json, Router, extract::State, routing::get};
use auth::require_perm;
use serde_json::Value;

use crate::AppState;

use super::{
    handler,
    protocol::{JsonRpcRequest, JsonRpcResponse, error_codes},
    sse,
};

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/mcp", get(sse::sse_handler).post(handle_post))
        .layer(require_perm!("mcp:access"))
}

async fn handle_post(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Json<JsonRpcResponse> {
    // JSON-RPC 2.0 batch requests (arrays) are not supported.
    if body.is_array() {
        return Json(JsonRpcResponse::err(
            None,
            error_codes::INVALID_REQUEST,
            "batch requests are not supported",
        ));
    }

    match serde_json::from_value::<JsonRpcRequest>(body) {
        Ok(req) => Json(handler::dispatch(&state, req).await),
        Err(e) => Json(JsonRpcResponse::err(
            None,
            error_codes::INVALID_REQUEST,
            format!("invalid request: {e}"),
        )),
    }
}

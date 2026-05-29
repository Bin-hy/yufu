use crate::service::permission::{EnforceRequest, EnforceResponse};
use crate::{AppState, result};
use axum::Router;
use axum::{Json, extract::State, routing::post};

/// POST /api/v1/enforce — PDP (Policy Decision Point).
///
/// Callers send `{ application, subject, action, resource_type?, resource_id? }`
/// and receive `{ allow: bool, reason: String }`.
///
/// Caching: callers SHOULD cache positive results locally for a short period
/// (default 5 s) to limit PDP load. Negative results SHOULD NOT be cached.
async fn enforce_handler(
    State(state): State<AppState>,
    Json(req): Json<EnforceRequest>,
) -> result::Result<Json<EnforceResponse>> {
    let resp = crate::service::permission::enforce(&state.database.connection, &req).await?;
    Ok(Json(resp))
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route("/enforce", post(enforce_handler))
}

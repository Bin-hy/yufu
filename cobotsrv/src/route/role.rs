use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::result::Result;
// req
#[derive(Serialize, Deserialize)]
struct RoleUpdateReq {
    role: String,
    group: String,
}

// resp
#[derive(Serialize, Deserialize)]
struct RoleUpdateResp {
    role: String,
    group: String,
}

pub async fn get_auth_route() -> Router<AppState> {
    Router::new().route("/role", post(update_role))
}

pub async fn update_role(
    State(state): State<AppState>,
    Json(payload): Json<RoleUpdateResp>,
) -> Result<Json<RoleUpdateResp>> {
    todo!()
}

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::result::Result;
// req
#[derive(Serialize, Deserialize)]
pub struct RoleUpdateReq {
    role: String,
    group: String,
}

// resp
#[derive(Serialize, Deserialize)]
pub struct RoleUpdateResp {
    role: String,
    group: String,
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route("/role", post(update_role))
}

pub async fn update_role(
    State(_state): State<AppState>,
    Json(_payload): Json<RoleUpdateResp>,
) -> Result<Json<RoleUpdateResp>> {
    todo!()
}

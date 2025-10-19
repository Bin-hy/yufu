use anyhow::anyhow;
use crate::AppState;
use crate::result::Result;

use auth::claims::Claims;
use axum::{
    Router,
    extract::{Json, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::entity::user::Model as UserModel;
use crate::result::ApiResponse;
use crate::service::user::UserService;
// request body

// response body

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize)]
pub struct UserVO {
    pub username: String,
    pub nickname: Option<String>,
    pub role: String,
}

impl From<UserModel> for UserVO {
    fn from(value: UserModel) -> Self {
        UserVO {
            username: value.username.unwrap().to_string(),
            nickname: value.nickname,
            role: "User".into(),
        }
    }
}
// Build routes
pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route("/user", get(get_user_by_id))
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/user",
    responses(
        (status = 200, description = "获取用户信息成功", body = UserVO),
    ),
))]
pub async fn get_user_by_id(
    claim: Claims,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<UserVO>>> {
    let db = &state.database.connection;
    let db_user = UserService::get_user_by_id(&db, claim.id.to_string()).await?;
    let user_vo: UserVO = UserVO::from(db_user);
    Ok(Json(ApiResponse::success(user_vo)))
}

#[cfg(feature = "openapi")]
use utoipa::OpenApi;

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        get_user_by_id,
    ),
    tags ((name = "User 用户", description = "User相关CRUD"),)
)]
pub struct UserApi;

use crate::AppState;
use crate::result::ApiResponse;
use crate::result::Result;
use crate::service::user::UserService;

use axum::routing::post;
use axum::{Json, Router, extract::State};
use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::OpenApi;

// request body

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginData {
    username: String,
    password: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryData {
    username: String,
    nickname: String,
    password: String,
}

// response body

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct LoginRespData {
    token: String,
    username: String,
    nickname: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct RegistryRespData {
    token: String,
    username: String,
    nickname: String,
}

// Build routes
pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(auth_register))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/register",
    request_body(
        content = RegistryData,
        description = "用户注册信息，包含用户名和密码",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "用户注册成功并返回 token", body = RegistryRespData),
        (status = 409, description = "请求数据无效或用户名已存在")
    ),
    // tag = "Auth身份校验"
))]
pub async fn auth_register(
    State(state): State<AppState>,
    Json(payload): Json<RegistryData>,
) -> Result<Json<ApiResponse<RegistryRespData>>> {
    let username = payload.username;
    let password = payload.password;
    let nickname = payload.nickname;
    // check username and password
    let token = UserService::register(
        &state.database.connection,
        &state.auth_state.token_manager,
        username.as_str(),
        password.as_str(),
        nickname.as_str(),
    )
    .await?;
    let resp = RegistryRespData {
        token,
        username,
        nickname,
    };
    Ok(Json(ApiResponse::success(resp)))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/login",
    request_body(
        content = LoginData,
        description = "用户登录信息，包含用户名和密码",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "登录成功并返回 token", body = LoginRespData),
        (status = 401, description = "用户名或密码错误"),
    ),
    // tag = "Auth身份校验"
))]
pub async fn login(
    State(mut state): State<AppState>,
    Json(login_data): Json<LoginData>,
) -> Result<Json<ApiResponse<LoginRespData>>> {
    let db = &state.database.connection;

    // 1. 校验请求数据
    let username = login_data.username.as_str();
    let password = login_data.password.as_str();

    // 2. 校验数据库账号密码
    let (token, user_vo) =
        UserService::auth(&db, &state.auth_state.token_manager, username, password).await?;

    let resp = LoginRespData {
        token: token.to_string(),
        username: username.to_string(),
        nickname: user_vo.nickname.unwrap_or(username.to_string()), //
    };
    Ok(Json(ApiResponse::success(resp)))
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        login,
        auth_register,
    ),
    tags ((name = "Auth身份校验", description = "权限身份校验接口"),)
)]
pub struct AuthApi;

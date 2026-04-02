use crate::result::Result;
use crate::service::user::UserService;
use crate::{AppState, IdType};

use axum::routing::post;
use axum::{Json, Router, extract::State};
use axum_extra::TypedHeader;
use axum_extra::headers::Cookie;
use http::{HeaderMap, header::SET_COOKIE};
use serde::{Deserialize, Serialize};

use crate::entity::refresh_token;
use crate::error::AppError;
#[cfg(feature = "openapi")]
use utoipa::OpenApi;
// request body

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginData {
    pub username: String,
    pub password: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryData {
    pub username: String,
    pub nickname: String,
    pub password: String,
}

// response body

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct LoginRespData {
    access_token: String,
    username: String,
    nickname: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct RegistryRespData {
    access_token: String,
    username: String,
    nickname: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct RefreshRespData {
    access_token: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct KickReqData {
    sub: IdType,
}

// Build routes
pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route("/kickout", post(kick_out))
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(auth_register))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
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
))]
pub async fn auth_register(
    State(state): State<AppState>,
    Json(payload): Json<RegistryData>,
) -> Result<(HeaderMap, Json<RegistryRespData>)> {
    let username = payload.username;
    let password = payload.password;
    let nickname = payload.nickname;

    // check username and password
    let (access_token, refresh_token) = UserService::register(
        &state.database.connection,
        &state.auth_state.token_manager,
        &state.auth_state.refresh_token_manager,
        username.as_str(),
        password.as_str(),
        nickname.as_str(),
    )
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        http::HeaderValue::from_str(&format!(
            "refresh_token={}; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age={}",
            refresh_token, state.config.auth.refresh_exp_seconds,
        ))?,
    );

    let resp = RegistryRespData {
        access_token,
        username,
        nickname,
    };
    Ok((headers, Json(resp)))
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
))]
pub async fn login(
    State(state): State<AppState>,
    Json(login_data): Json<LoginData>,
) -> Result<(HeaderMap, Json<LoginRespData>)> {
    let db = &state.database.connection;

    // 1. 校验请求数据
    let username = login_data.username.as_str();
    let password = login_data.password.as_str();

    // 2. 校验数据库账号密码
    let (access_token, refresh_token, user_vo) = UserService::auth(
        db,
        &state.auth_state.token_manager,
        &state.auth_state.refresh_token_manager,
        username,
        password,
    )
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        http::HeaderValue::from_str(&format!(
            "refresh_token={}; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age={}",
            refresh_token, state.config.auth.refresh_exp_seconds,
        ))?,
    );
    let resp = LoginRespData {
        access_token,
        username: username.to_string(),
        nickname: user_vo.nickname.unwrap_or(username.to_string()),
    };
    Ok((headers, Json(resp)))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/refresh",
    request_body(
        description = "使用刷新令牌获取新的访问令牌与刷新令牌",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "刷新成功，返回新的 access_token 与 refresh_token", body = RefreshRespData),
        (status = 401, description = "刷新令牌无效或已失效"),
    ),
))]
pub async fn refresh(
    State(state): State<AppState>,
    TypedHeader(cookie): TypedHeader<Cookie>,
) -> Result<(HeaderMap, Json<RefreshRespData>)> {
    // 从 Cookie 中读取 refresh_token
    let refresh_token = cookie
        .get("refresh_token")
        .ok_or_else(|| AppError::BadRequest("missing refresh token".to_string()))?;

    // 调用业务逻辑
    let (access_token, new_refresh_token) = UserService::refresh_tokens(
        &state.database.connection,
        &state.auth_state.refresh_token_manager,
        &state.auth_state.token_manager,
        refresh_token,
    )
    .await?;

    // 设置新的 refresh_token Cookie
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        format!(
            "refresh_token={}; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age={}",
            new_refresh_token, state.config.auth.refresh_exp_seconds
        )
        .parse()?,
    );

    Ok((headers, Json(RefreshRespData { access_token })))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/logout",
    request_body(
        description = "登出当前会话：提升刷新令牌版本，使现有刷新令牌失效",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "登出成功，返回空的令牌占位", body = RefreshRespData),
        (status = 401, description = "刷新令牌无效或已失效"),
    ),
))]
pub async fn logout(
    State(state): State<AppState>,
    TypedHeader(cookie): TypedHeader<Cookie>,
) -> Result<(HeaderMap, Json<RefreshRespData>)> {
    let refresh_token = cookie
        .get("refresh_token")
        .ok_or_else(|| AppError::BadRequest("missing refresh token".to_string()))?;

    UserService::logout_by_refresh(
        &state.database.connection,
        &state.auth_state.refresh_token_manager,
        refresh_token,
    )
    .await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        http::HeaderValue::from_str(
            "refresh_token=; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age=0",
        )?,
    );
    Ok((
        headers,
        Json(RefreshRespData {
            access_token: String::new(),
        }),
    ))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/kickout",
    request_body(
        content = KickReqData,
        description = "管理员强制下线指定用户，会话刷新族失效",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "强制下线成功"),
        (status = 404, description = "用户不存在或会话不存在"),
    ),
))]
pub async fn kick_out(
    State(state): State<AppState>,
    Json(payload): Json<KickReqData>,
) -> Result<()> {
    let db = &state.database.connection;
    refresh_token::Entity::delete_by_sub(db, payload.sub).await?;
    Ok(())
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        login,
        auth_register,
        refresh,
        logout,
        kick_out,
    ),
    tags ((name = "Auth身份校验", description = "权限身份校验接口"),)
)]
pub struct AuthApi;

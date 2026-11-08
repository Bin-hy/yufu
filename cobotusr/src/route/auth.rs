use crate::result::Result;
use crate::service::user::UserService;
use crate::{AppState, IdType};

use axum::routing::{get, post};
use axum::{Json, Router, extract::State};
use http::{HeaderMap, header::SET_COOKIE};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::service::security::{browser, revocation::RevocationReceipt};
use auth::security::budget::SecurityBudget;
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
    csrf_token: String,
    username: String,
    nickname: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct RegistryRespData {
    access_token: String,
    csrf_token: String,
    username: String,
    nickname: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct RefreshRespData {
    access_token: String,
    csrf_token: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct KickReqData {
    sub: IdType,
}

// Build routes
pub fn get_revocation_routes() -> Router<AppState> {
    Router::new().route("/kickout", post(kick_out))
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/csrf", get(csrf_challenge))
        .route("/health/ready", get(authentication_readiness))
        .route("/register", post(auth_register))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
}

async fn authentication_readiness(
    State(state): State<AppState>,
) -> (http::StatusCode, Json<serde_json::Value>) {
    let ready = state.security_clock().and_then(SecurityBudget::new).is_ok();
    (
        if ready {
            http::StatusCode::OK
        } else {
            http::StatusCode::SERVICE_UNAVAILABLE
        },
        Json(serde_json::json!({"ready": ready, "security_clock_synchronized": ready})),
    )
}

async fn csrf_challenge(
    State(state): State<AppState>,
    request_headers: HeaderMap,
) -> Result<(HeaderMap, Json<serde_json::Value>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let cookies = browser::cookies(&request_headers)?;
    let mut headers = HeaderMap::new();
    let proof = browser::issue_csrf(
        &state,
        cookies.get("refresh_token").map(String::as_str),
        &mut headers,
    )?;
    budget.remaining()?;
    Ok((
        headers,
        Json(serde_json::json!({"csrf_token": proof.expose_secret()})),
    ))
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
    request_headers: HeaderMap,
    Json(payload): Json<RegistryData>,
) -> Result<(HeaderMap, Json<RegistryRespData>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    browser::validate_csrf(&state, &request_headers, None, &budget)?;
    let username = payload.username;
    let password = payload.password;
    let nickname = payload.nickname;

    // check username and password
    let (access_token, refresh_token) = UserService::register(
        &state,
        username.as_str(),
        password.as_str(),
        nickname.as_str(),
        &budget,
    )
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, browser::refresh_cookie(&state, &refresh_token)?);

    let csrf = browser::issue_csrf(&state, Some(&refresh_token), &mut headers)?;
    let resp = RegistryRespData {
        access_token,
        csrf_token: csrf.expose_secret().into(),
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
    request_headers: HeaderMap,
    Json(login_data): Json<LoginData>,
) -> Result<(HeaderMap, Json<LoginRespData>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    browser::validate_csrf(&state, &request_headers, None, &budget)?;
    // 1. 校验请求数据
    let username = login_data.username.as_str();
    let password = login_data.password.as_str();

    // 2. 校验数据库账号密码
    let (access_token, refresh_token, user_vo) =
        UserService::auth(&state, username, password, &budget).await?;

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, browser::refresh_cookie(&state, &refresh_token)?);
    let csrf = browser::issue_csrf(&state, Some(&refresh_token), &mut headers)?;
    let resp = LoginRespData {
        access_token,
        csrf_token: csrf.expose_secret().into(),
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
    request_headers: HeaderMap,
) -> Result<(HeaderMap, Json<RefreshRespData>)> {
    // 从 Cookie 中读取 refresh_token
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let refresh_token = browser::validate_csrf(&state, &request_headers, None, &budget)?
        .ok_or_else(|| AppError::BadRequest("missing refresh token".to_string()))?;

    // 调用业务逻辑
    let (access_token, new_refresh_token) =
        UserService::refresh_tokens(&state, refresh_token.expose_secret(), &budget).await?;

    // 设置新的 refresh_token Cookie
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        browser::refresh_cookie(&state, &new_refresh_token)?,
    );

    let csrf = browser::issue_csrf(&state, Some(&new_refresh_token), &mut headers)?;
    Ok((
        headers,
        Json(RefreshRespData {
            access_token,
            csrf_token: csrf.expose_secret().into(),
        }),
    ))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/logout",
    request_body(
        description = "撤销当前登记会话及刷新族，提交后清除Cookie",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "撤销提交成功，返回回执", body = RevocationReceipt),
        (status = 401, description = "刷新令牌无效或已失效"),
    ),
))]
pub async fn logout(
    State(state): State<AppState>,
    request_headers: HeaderMap,
) -> Result<(HeaderMap, Json<RevocationReceipt>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let refresh_token = browser::validate_csrf(&state, &request_headers, None, &budget)?
        .ok_or_else(|| AppError::BadRequest("missing refresh token".to_string()))?;

    let receipt = UserService::logout_by_refresh(&state, &refresh_token, None, &budget).await?;
    Ok((browser::clear_session_cookies(&state)?, Json(receipt)))
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
    request_headers: HeaderMap,
    Json(payload): Json<KickReqData>,
) -> Result<Json<RevocationReceipt>> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &request_headers, &budget).await?;
    Ok(Json(
        browser::lifecycle(&state)?
            .kickout_sessions(&manager, &payload.sub.to_string(), &budget)
            .await?,
    ))
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

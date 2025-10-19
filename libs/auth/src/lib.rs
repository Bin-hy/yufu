pub mod claims;
mod claims_extractor;
pub mod token_manager;

use crate::token_manager::{TokenError, TokenManager};
use axum::extract::State;
/**
@ Method: 鉴权中间件函数
 */
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

#[derive(Clone)]
pub struct AuthState {
    pub token_manager: TokenManager,
}

/// 中间件：验证 JWT Token 并将 Claims 注入请求扩展中
pub async fn access_middleware(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    // 1. 从 Header 提取 token
    let auth_header = request.headers().get("Authorization");

    let token = match auth_header.and_then(|v| v.to_str().ok()) {
        Some(header_value) if header_value.starts_with("Bearer ") => {
            header_value.trim_start_matches("Bearer ").trim()
        }
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };

    // 2.初始化 TokenManager
    let token_manager = &auth_state.token_manager;

    // 3.校验 token
    match token_manager.verify_token(token) {
        Ok(claims) => {
            // 4.注入 Claims 到 Request 扩展，供后续 handler 使用
            request.extensions_mut().insert(claims);

            next.run(request).await
        }
        Err(TokenError::Expired) => {
            (StatusCode::UNAUTHORIZED, "Token expired, please re-login").into_response()
        }
        Err(TokenError::InvalidSignature) => {
            (StatusCode::UNAUTHORIZED, "Invalid token signature").into_response()
        }
        Err(TokenError::InvalidToken) => {
            (StatusCode::UNAUTHORIZED, "Malformed token").into_response()
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            format!("Token validation failed: {:?}", e),
        )
            .into_response(),
    }
}

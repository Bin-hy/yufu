//! Authentication middleware module.
//!
//! This module provides **JWT-based authentication** for Axum applications.
//!
//! It includes:
//! - [`access_middleware`]: A middleware function that verifies JWT tokens.
//! - [`AuthState`]: Shared state containing the [`TokenManager`] instance used for validation.
//!
//! The middleware checks the `Authorization` header for a bearer token,
//! validates it using the [`TokenManager`], and injects the decoded [`Claims`]
//! into the request's extensions for later extraction by handlers or extractors.
//!
//! # Example
//!
//! The following demonstrates how to integrate the authentication middleware
//! within an Axum application—just like in the `serve()` function of your project:
//!
//! ```no_run
//! use axum::{Router, middleware, routing::get};
//! use auth::auth::{access_middleware, AuthState};
//! use auth::token_manager::TokenManager;
//! use auth::claims::Claims;
//!
//! // Example protected route that requires authentication
//! async fn profile(claims: Claims) -> String {
//!     format!("Hello, user {}!", claims.id)
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // 1. Initialize the TokenManager (JWT secret + expiration time)
//!     let token_manager = TokenManager::new("my_secret_key", 3600);
//!
//!     // 2.1 Create the shared authentication state,in your middleware to verify user
//!     let auth_state = AuthState { token_manager };
//!     // 2.2 Create global state, in your auth service to generate user
//!     let app_state = AppState {auth_state:auth_state.clone()}
//!     // 3. Define protected routes that require valid JWT tokens
//!     let auth_routes = Router::new()
//!         .route("/profile", get(profile))
//!         // Apply the authentication middleware to this router
//!         .layer(middleware::from_fn_with_state(auth_state.clone(), access_middleware));
//!
//!     // 4. Optionally, define public routes that do not require authentication
//!     let public_routes = Router::new()
//!         .route("/health", get(|| async { "OK" }));
//!
//!     // 5. Combine routers into a single application
//!     let app = Router::new()
//!         .nest("/api/v1", auth_routes)
//!         .nest("/api/v1", public_routes)
//!         .with_state();
//!
//!     // 6. Start the Axum server
//!     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//!         .serve(app.into_make_service())
//!         .await
//!         .unwrap();
//! }
//!
//!
//! #[derive(Clone)]
//! struct AppState {
//!     auth_state: Arc<AuthState>,
//! }
//!
//! ```
pub mod claims;
mod claims_extractor;
pub mod token_manager;

use crate::token_manager::{TokenError, TokenManager};
use axum::extract::State;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Shared authentication state that holds the [`TokenManager`].
///
/// This state is passed into the [`access_middleware`] so that
/// each request can use the same token management configuration.
#[derive(Clone)]
pub struct AuthState {
    pub token_manager: TokenManager,
}

/// Middleware function for JWT authentication.
///
/// This middleware performs the following steps:
///
/// 1. Extracts the `Authorization` header from the incoming request.
/// 2. Validates the bearer token using the [`TokenManager`].
/// 3. On success, inserts the decoded [`Claims`] into the request's extensions.
/// 4. On failure, returns an appropriate `401 Unauthorized` response.
///
/// # Arguments
///
/// * `State(auth_state)` — Shared [`AuthState`] containing the [`TokenManager`].
/// * `request` — The incoming HTTP request.
/// * `next` — The next middleware or handler in the Axum stack.
///
/// # Returns
///
/// A [`Response`] that either:
/// - Proceeds to the next handler if authentication succeeds.
/// - Returns a `401 Unauthorized` error if authentication fails.
///
/// # Errors
///
/// The middleware may return the following responses:
///
/// * `401 Unauthorized` — If the `Authorization` header is missing or invalid.
/// * `401 Unauthorized` — If the token is expired, malformed, or has an invalid signature.
///
/// # Example
///
/// ```no_run
/// use axum::{middleware, Router, routing::get};
/// use auth::{access_middleware, AuthState};
/// use auth::token_manager::TokenManager;
///
/// async fn profile(claims: my_crate::claims::Claims) -> String {
///     format!("User ID: {}", claims.id)
/// }
///
/// let token_manager = TokenManager::new("secret", 3600);
/// let auth_state = AuthState { token_manager };
///
/// let app = Router::new()
///     .route("/profile", get(profile))
///     .layer(middleware::from_fn_with_state(auth_state, access_middleware));
/// ```
pub async fn access_middleware(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    // 1. Extract token from the Authorization header.
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

    // 2. Retrieve the TokenManager from state.
    let token_manager = &auth_state.token_manager;

    // 3. Validate the token and inject Claims into request extensions.
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

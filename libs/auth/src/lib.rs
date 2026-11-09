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
//! # Middleware integration
//!
//! New integrations use [`strict_access_middleware`] with a configured
//! [`security::credential::StrictAccessState`] to verify current credential status.
//! The legacy JWT middleware remains available for compatibility:
//!
//! ```no_run
//! use axum::{Router, middleware, routing::get};
//! use auth::{AuthState, access_middleware};
//! use auth::claims::Claims;
//! use auth::token_manager::TokenManager;
//!
//! async fn profile(claims: Claims<i32>) -> String {
//!     format!("Hello, user {}!", claims.id)
//! }
//! let state = AuthState {
//!     token_manager: TokenManager::new("example-access-key", 3600),
//!     refresh_token_manager: TokenManager::new("example-refresh-key", 86400),
//! };
//! let app: Router = Router::new().route("/profile", get(profile))
//!     .layer(middleware::from_fn_with_state(state, access_middleware::<i32>));
//! ```
pub mod claims;
mod claims_extractor;
pub mod jwks;
pub mod security;
pub mod token_manager;

/// Strict access profile, including current registered credential status.
pub async fn strict_access_middleware(
    State(auth): State<Arc<security::credential::StrictAccessState>>,
    mut request: Request,
    next: Next,
) -> Response {
    use security::{credential::AuthenticatedContext, error::SecurityError};
    request.extensions_mut().remove::<AuthenticatedContext>();
    match auth.authenticate(request.headers()).await {
        Ok(context) => {
            request.extensions_mut().insert(context);
            next.run(request).await
        }
        Err(error) => {
            let status = match error {
                SecurityError::InvalidCredential | SecurityError::CredentialRevoked => {
                    StatusCode::UNAUTHORIZED
                }
                SecurityError::Forbidden => StatusCode::FORBIDDEN,
                SecurityError::InvalidResource => StatusCode::BAD_REQUEST,
                SecurityError::Conflict => StatusCode::CONFLICT,
                SecurityError::DependencyUnavailable
                | SecurityError::DeadlineExceeded
                | SecurityError::CapacityExceeded
                | SecurityError::CommitUncertain => StatusCode::SERVICE_UNAVAILABLE,
            };
            (status, error.to_string()).into_response()
        }
    }
}

use crate::jwks::JwksClient;
use crate::token_manager::{TokenError, TokenManager};
use axum::extract::State;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::claims::Claims;
use axum::body::Body;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

/// Shared authentication state that holds the [`TokenManager`].
///
/// This state is passed into the [`access_middleware`] so that
/// each request can use the same token management configuration.
#[derive(Clone)]
pub struct AuthState {
    pub token_manager: TokenManager,
    pub refresh_token_manager: TokenManager,
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
/// use axum::{Router, middleware, routing::get};
/// use auth::{AuthState, access_middleware};
/// use auth::claims::Claims;
/// use auth::token_manager::TokenManager;
/// async fn profile(claims: Claims<i32>) -> String { claims.id.to_string() }
/// let state = AuthState {
///     token_manager: TokenManager::new("example-key", 3600),
///     refresh_token_manager: TokenManager::new("example-refresh-key", 86400),
/// };
/// let app: Router = Router::new().route("/profile", get(profile))
///     .layer(middleware::from_fn_with_state(state, access_middleware::<i32>));
/// ```
pub async fn access_middleware<IDTYPE>(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response
where
    for<'de> IDTYPE: Clone + Send + Sync + 'static + Serialize + Deserialize<'de>,
{
    // 1. Extract token from the Authorization header, or query param (for WebSocket).
    let token = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .or_else(|| {
            request.uri().query().and_then(|q| {
                q.split('&')
                    .filter_map(|p| p.split_once('='))
                    .find(|(k, _)| *k == "access_token")
                    .map(|(_, v)| v.trim().to_string())
            })
        });

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };
    let token = token.as_str();

    // 2. Retrieve the TokenManager from state.
    let token_manager = &auth_state.token_manager;

    // 3. Validate the token and inject Claims into request extensions.
    match token_manager.verify_token::<IDTYPE>(token) {
        Ok(claims) => {
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

/// Middleware for validating JWT tokens with optional custom callback logic.
///
/// This middleware performs the following tasks:
///
/// 1. Extracts and parses the Bearer token from the `Authorization` header.
/// 2. Uses the provided [`TokenManager`] to validate the token.
/// 3. Injects the verified [`Claims`] into the request’s `Extensions`.
/// 4. Executes a user-defined asynchronous callback after successful validation
///    (e.g., to load user data from a database).
/// 5. Returns appropriate HTTP error responses if validation or callback fails.
///
/// # Type Parameters
///
/// - `F`: An asynchronous function or closure that takes owned `Claims` and `Request`
///   and returns the updated request.
/// - `Fut`: The future returned by the callback function.
///
/// # Arguments
///
/// * `auth_state` - Application [`AuthState`] containing the [`TokenManager`].
/// * `request` - The current HTTP request, where extensions can be modified.
/// * `next` - The next middleware or handler in the Axum chain.
/// * `callback` - A user-defined async function that runs after token verification,
///   allowing for additional logic such as database lookups.
///
/// # Returns
///
/// * On success, forwards the request to the next handler.
/// * On failure, returns an appropriate [`StatusCode`] and error message.
///
/// # Example
///
/// ```no_run
/// use axum::{Router, body::Body, extract::{Request, State}, middleware::{self, Next}, response::Response, routing::get};
/// use auth::{AuthState, access_middleware_with_callback};
/// use auth::token_manager::TokenManager;
/// async fn load_identity(state: State<AuthState>, request: Request<Body>, next: Next) -> Response {
///     access_middleware_with_callback::<i32, _, _>(state, request, next, |claims, mut request| async move {
///         request.extensions_mut().insert(claims.id);
///         Ok(request)
///     }).await
/// }
/// let state = AuthState {
///     token_manager: TokenManager::new("example-key", 3600),
///     refresh_token_manager: TokenManager::new("example-refresh-key", 86400),
/// };
/// let app: Router = Router::new().route("/profile", get(|| async { "OK" }))
///     .layer(middleware::from_fn_with_state(state, load_identity));
/// ```
pub async fn access_middleware_with_callback<IDTYPE, F, Fut>(
    State(auth_state): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
    callback: F,
) -> Response
where
    for<'de> IDTYPE: Clone + Send + Sync + 'static + Serialize + Deserialize<'de>,
    for<'a> F: Fn(Claims<IDTYPE>, Request<Body>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Request<Body>, (StatusCode, String)>> + Send + 'static,
{
    // 1. extract Token from Header, or query param (for WebSocket)
    let token = match request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .or_else(|| {
            request.uri().query().and_then(|q| {
                q.split('&')
                    .filter_map(|p| p.split_once('='))
                    .find(|(k, _)| *k == "access_token")
                    .map(|(_, v)| v.trim().to_string())
            })
        }) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };
    let token = token.as_str();

    // 2. validate Token
    let token_manager = &auth_state.token_manager;
    match token_manager.verify_token::<IDTYPE>(token) {
        Ok(claims) => {
            // 3. add Claims
            request.extensions_mut().insert(claims.clone());

            // 4. execute the callback (获取所有权)
            let request = match callback(claims, request).await {
                Ok(req) => req,
                Err((status, msg)) => return (status, msg).into_response(),
            };

            // 5. execute the next middleware/handler
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

/// JWT verification middleware using a [`JwksClient`] (RS256, public-key only).
///
/// Use this in services that consume tokens issued by another service (e.g.
/// cobotrix consuming resource-app-issued tokens). Validated [`crate::claims::AccessTokenClaims`]
/// is inserted into request extensions so handlers can extract it.
pub async fn access_middleware_rs256(
    State(jwks): State<Arc<JwksClient>>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match security::credential::extract_bearer(request.headers()) {
        Ok(token) => token,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };
    let token = token.expose_secret();

    match jwks.verify(token) {
        Ok(claims) => {
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

/// JWT verification middleware using the **local** [`crate::token_manager::IdTokenSigner`]
/// (RS256, signer holds the private key, no JWKS roundtrip).
///
/// Use this in the IdP itself (e.g. resource-app) to verify tokens it just issued.
/// External services should use [`access_middleware_rs256`] with a [`JwksClient`].
///
/// Validated [`crate::claims::AccessTokenClaims`] is inserted into request extensions.
/// `aud` is NOT enforced here — apply per-route audience checks if needed.
pub async fn access_middleware_local_rs256(
    State(signer): State<Arc<crate::token_manager::IdTokenSigner>>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match security::credential::extract_bearer(request.headers()) {
        Ok(token) => token,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };
    let token = token.expose_secret();

    match signer.verify_access_token(token, None) {
        Ok(claims) => {
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

/// Trait for async permission checking, used by the `require_perm!` macro
/// to support PDP-based enforcement (calling resource-app's `/enforce` endpoint)
/// instead of relying on permissions embedded in the JWT claims.
///
/// If a `PermissionChecker` is present in request extensions, `require_perm!`
/// will use it for an async PDP check. Otherwise it falls back to
/// `AccessTokenClaims.permissions` (backward compatible with resource-app).
pub trait PermissionChecker: Send + Sync {
    fn check_permission<'a>(
        &'a self,
        subject: &str,
        permission: &str,
        auth_header: &str,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

/// Macro: create an axum middleware layer that requires a specific permission.
///
/// The token MUST already be verified by `access_middleware_rs256` or
/// `access_middleware_local_rs256` so `AccessTokenClaims` is in request extensions.
///
/// **Two modes** (checked in order):
/// 1. If `Arc<dyn PermissionChecker>` is in request extensions, calls it for an
///    async PDP check (e.g. cobotrix calling resource-app's `/api/v1/enforce`).
/// 2. Otherwise falls back to checking `AccessTokenClaims.permissions` embedded
///    in the JWT (backward compatible with resource-app).
///
/// # Usage
///
/// ```ignore
/// use auth::require_perm;
///
/// Router::new()
///     .route("/devices", get(list).layer(require_perm!("device:read")))
/// ```
#[macro_export]
macro_rules! require_perm {
    ($perm:expr) => {
        axum::middleware::from_fn(
            |mut request: axum::extract::Request, next: axum::middleware::Next| async move {
                // Mode 1: PDP-based check via PermissionChecker in extensions
                if let Some(checker) = request
                    .extensions()
                    .get::<std::sync::Arc<dyn $crate::PermissionChecker>>()
                {
                    let sub = request
                        .extensions()
                        .get::<$crate::claims::AccessTokenClaims>()
                        .map(|c| c.sub.clone())
                        .unwrap_or_default();
                    let auth_header = request
                        .headers()
                        .get("Authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default();
                    let allowed = checker.check_permission(&sub, $perm, auth_header).await;
                    if allowed {
                        return next.run(request).await;
                    }
                    return axum::response::IntoResponse::into_response((
                        axum::http::StatusCode::FORBIDDEN,
                        concat!("Missing permission: ", $perm),
                    ));
                }

                // Mode 2: fallback to JWT-embedded permissions
                let allowed = request
                    .extensions()
                    .get::<$crate::claims::AccessTokenClaims>()
                    .map(|c| c.permissions.iter().any(|p| p == $perm))
                    .unwrap_or(false);
                if allowed {
                    next.run(request).await
                } else {
                    axum::response::IntoResponse::into_response((
                        axum::http::StatusCode::FORBIDDEN,
                        concat!("Missing permission: ", $perm),
                    ))
                }
            },
        )
    };
}

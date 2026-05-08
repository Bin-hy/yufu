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
pub mod jwks;
pub mod token_manager;

use crate::jwks::JwksClient;
use crate::token_manager::{TokenError, TokenManager};
use axum::extract::State;
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
pub async fn access_middleware<IDTYPE>(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response
where
    for<'de> IDTYPE: Clone + Send + Sync + 'static + Serialize + Deserialize<'de>,
{
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
/// - `F`: An asynchronous function or closure that takes `&Claims` and `&mut Request`
///   and performs custom logic.
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
/// ```rust,no_run
/// use axum::{
///     Router,
///     routing::get,
///     middleware,
///     http::{Request, StatusCode},
///     extract::State,
///     response::IntoResponse,
/// };
/// use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait};
/// use uuid::Uuid;
/// use std::sync::Arc;
///
/// #[derive(Clone)]
/// struct AuthState {
///     token_manager: Arc<TokenManager>,
/// }
///
/// #[derive(Clone, Debug)]
/// struct Claims {
///     user_id: Uuid,
///     username: String,
/// }
///
/// struct TokenManager;
/// impl TokenManager {
///     fn verify_token(&self, token: &str) -> Result<Claims, TokenError> {
///         // Example: Simulate token verification
///         Ok(Claims {
///             user_id: Uuid::new_v4(),
///             username: "demo_user".to_string(),
///         })
///     }
/// }
///
/// #[derive(Debug)]
/// enum TokenError { Expired, InvalidSignature, InvalidToken }
///
/// async fn access_middleware_with_callback<F, Fut>(
///     State(auth_state): State<AuthState>,
///     mut request: Request,
///     next: axum::middleware::Next,
///     callback: F,
/// ) -> axum::response::Response
/// where
///     F: FnOnce(&Claims, &mut Request) -> Fut + Send + 'static,
///     Fut: std::future::Future<Output = Result<(), (StatusCode, String)>> + Send,
/// {
///     // Extract token from Authorization header
///     let token = match request
///         .headers()
///         .get("Authorization")
///         .and_then(|v| v.to_str().ok())
///         .and_then(|h| h.strip_prefix("Bearer "))
///     {
///         Some(t) => t.trim(),
///         None => return (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response(),
///     };
///
///     // Validate token
///     let token_manager = &auth_state.token_manager;
///     match token_manager.verify_token(token) {
///         Ok(claims) => {
///             // Inject claims into request extensions
///             request.extensions_mut().insert(claims.clone());
///
///             // Run custom callback
///             if let Err((code, msg)) = callback(&claims, &mut request).await {
///                 return (code, msg).into_response();
///             }
///
///             // Continue with the next handler
///             next.run(request).await
///         }
///         Err(TokenError::Expired) => (StatusCode::UNAUTHORIZED, "Token expired").into_response(),
///         Err(TokenError::InvalidSignature) => (StatusCode::UNAUTHORIZED, "Invalid signature").into_response(),
///         Err(TokenError::InvalidToken) => (StatusCode::UNAUTHORIZED, "Malformed token").into_response(),
///     }
/// }
///
/// // Example callback: load user info from database based on the token’s user_id
/// async fn load_user_info(
///     claims: &Claims,
///     req: &mut Request,
/// ) -> Result<(), (StatusCode, String)> {
///     // Simulate database lookup
///     let user_info = format!("User<{}>", claims.username);
///
///     // Inject user info into request extensions
///     req.extensions_mut().insert(user_info);
///     Ok(())
/// }
///
/// async fn handler(req: Request) -> impl IntoResponse {
///     let user_info = req.extensions().get::<String>().unwrap();
///     format!("Hello, {user_info}")
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let auth_state = AuthState {
///         token_manager: Arc::new(TokenManager),
///     };
///
///     let app = Router::new()
///         .route("/secure", get(handler))
///         .route_layer(middleware::from_fn_with_state(
///             auth_state.clone(),
///             |state, req, next| access_middleware_with_callback(state, req, next, load_user_info),
///         ));
///
///     println!("✅ Middleware with callback ready!");
/// }
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
    // 1. extract Token from Header
    let token = match request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(t) => t.trim(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };

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
/// cobotrix consuming cobotsrv-issued tokens). Validated [`crate::claims::AccessTokenClaims`]
/// is inserted into request extensions so handlers can extract it.
pub async fn access_middleware_rs256(
    State(jwks): State<Arc<JwksClient>>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(t) => t.trim(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };

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
/// Use this in the IdP itself (e.g. cobotsrv) to verify tokens it just issued.
/// External services should use [`access_middleware_rs256`] with a [`JwksClient`].
///
/// Validated [`crate::claims::AccessTokenClaims`] is inserted into request extensions.
/// `aud` is NOT enforced here — apply per-route audience checks if needed.
pub async fn access_middleware_local_rs256(
    State(signer): State<Arc<crate::token_manager::IdTokenSigner>>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(t) => t.trim(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            )
                .into_response();
        }
    };

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

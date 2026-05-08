use crate::claims::{AccessTokenClaims, Claims};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use http::StatusCode;
use serde::Serialize;

/// Allows extracting [`Claims`] from request parts in Axum handlers.
///
/// This implementation makes it possible to use `Claims` directly as a parameter
/// in Axum route handlers, assuming the authentication middleware has already
/// validated the JWT and inserted the claims into the request's extensions.
///
/// If the claims are not found (e.g., the request is unauthenticated or the token
/// is invalid), a `401 Unauthorized` response will be returned automatically.
///
/// # Type Parameters
///
/// * `S` - The Axum state type. It must implement [`Send`] and [`Sync`].
///
/// # Errors
///
/// Returns a tuple `(StatusCode::UNAUTHORIZED, "Missing or invalid token claims")`
/// if the claims are not present in the request extensions.
///
/// # Example
///
/// ```
/// use axum::{Router, routing::get, extract::FromRequestParts, http::StatusCode};
/// use auth::Claims;
///
/// async fn protected_endpoint(claims: Claims) -> String {
///     format!("Hello, user {}!", claims.id)
/// }
///
/// let app = Router::new().route("/me", get(protected_endpoint));
/// ```
///
/// In this example, `Claims` is automatically extracted from the request
/// if it was previously inserted by an authentication middleware.
///
/// # See Also
///
/// - [`axum::extract::FromRequestParts`]
/// - [`axum::Extension`]
impl<S, IDTYPE> FromRequestParts<S> for Claims<IDTYPE>
where
    S: Send + Sync,
    IDTYPE: Clone + Send + Sync + 'static + Serialize,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims<IDTYPE>>()
            .cloned()
            .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid token claims"))
    }
}

impl<S> FromRequestParts<S> for AccessTokenClaims
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AccessTokenClaims>()
            .cloned()
            .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid token claims"))
    }
}

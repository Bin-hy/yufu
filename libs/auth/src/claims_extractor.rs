use crate::claims::Claims;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use http::StatusCode;

impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid token claims"))
    }
}

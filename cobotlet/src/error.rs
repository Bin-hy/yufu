use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    AlreadyExists(String),
    BadRequest(String),
    Throw(String),
    InternalServerError(anyhow::Error),
}

impl AppError {
    pub fn not_found<T: ToString>(t: T) -> Self {
        AppError::NotFound(t.to_string())
    }

    pub fn already_exists<T: ToString>(t: T) -> Self {
        AppError::AlreadyExists(t.to_string())
    }

    pub fn bad_request<T: ToString>(t: T) -> Self {
        AppError::BadRequest(t.to_string())
    }

    pub fn throw<T: ToString>(t: T) -> Self {
        AppError::Throw(t.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::NotFound(e) => (StatusCode::NOT_FOUND, e),
            AppError::AlreadyExists(e) => (StatusCode::CONFLICT, e),
            AppError::BadRequest(e) => (StatusCode::BAD_REQUEST, e),
            AppError::Throw(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            AppError::InternalServerError(e) => {
                tracing::error!(?e, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({"error": msg}))).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        AppError::InternalServerError(err.into())
    }
}

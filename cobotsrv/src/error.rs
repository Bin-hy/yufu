use crate::result::{ApiResponse, BizCode};
use axum::response::{IntoResponse, Json, Response};
use http::StatusCode;

#[derive(Debug)]
pub enum AppError {
    NoAvailableDevice,
    RequestProxyError,
    NotFound(String),
    AlreadyExists(String),
    Throw(String),
    InternalServerError(anyhow::Error),

    Unauthorized(String),
}

impl AppError {
    pub fn not_found<T>(t: T) -> Self
    where
        T: ToString,
    {
        AppError::NotFound(t.to_string())
    }

    pub fn already_exists<T>(t: T) -> Self
    where
        T: ToString,
    {
        AppError::AlreadyExists(t.to_string())
    }

    pub fn throw<T>(t: T) -> Self
    where
        T: ToString,
    {
        AppError::Throw(t.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, biz_code, message) = match self {
            AppError::NoAvailableDevice => (
                StatusCode::SERVICE_UNAVAILABLE,
                BizCode::NoAvailableDevice,
                BizCode::NoAvailableDevice.message().to_string(),
            ),
            AppError::RequestProxyError => (
                StatusCode::BAD_REQUEST,
                BizCode::RequestProxyError,
                BizCode::RequestProxyError.message().to_string(),
            ),
            AppError::NotFound(err) => (StatusCode::NOT_FOUND, BizCode::NotFound, err),
            AppError::AlreadyExists(err) => (StatusCode::CONFLICT, BizCode::AlreadyExists, err),
            AppError::InternalServerError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                BizCode::InternalServerError,
                err.to_string(),
            ),
            AppError::Throw(err) => (StatusCode::BAD_REQUEST, BizCode::BizError, err),
            AppError::Unauthorized(err) => (StatusCode::UNAUTHORIZED, BizCode::Unauthorized, err),
        };

        let message = message.as_str();
        let body = ApiResponse::<()>::error_with_code(biz_code.code(), message);
        (status, Json(body)).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        AppError::InternalServerError(err.into())
    }
}

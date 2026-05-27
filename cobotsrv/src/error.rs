use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use sea_orm::{DbErr, SqlErr};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NoAvailableDevice,
    RequestProxyError,
    NotFound(String),
    AlreadyExists(String),
    Throw(String),
    InternalServerError(anyhow::Error),
    Unauthorized(String),
    BadRequest(String),
    Forbidden(String),
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
        let (status, msg) = match self {
            AppError::NoAvailableDevice => {
                (StatusCode::SERVICE_UNAVAILABLE, "no available device".to_string())
            }
            AppError::RequestProxyError => (StatusCode::BAD_REQUEST, "request error".to_string()),
            AppError::NotFound(e) => (StatusCode::NOT_FOUND, e),
            AppError::AlreadyExists(e) => (StatusCode::CONFLICT, e),
            AppError::InternalServerError(e) => {
                tracing::error!(?e, "internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".to_string())
            }
            AppError::Throw(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            AppError::Unauthorized(e) => (StatusCode::UNAUTHORIZED, e),
            AppError::BadRequest(e) => (StatusCode::BAD_REQUEST, e),
            AppError::Forbidden(e) => (StatusCode::FORBIDDEN, e),
        };
        (status, Json(json!({"error": msg}))).into_response()
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

/// Extension trait for SeaORM `Result<T, DbErr>`.
pub trait SeaResultExt<T> {
    fn into_app(self) -> Result<T, AppError>;
    fn into_app_with_conflict_msg<M: ToString>(self, msg: M) -> Result<T, AppError>;
}

impl<T> SeaResultExt<T> for Result<T, DbErr> {
    fn into_app(self) -> Result<T, AppError> {
        self.into_app_with_conflict_msg("AlreadyExists")
    }

    fn into_app_with_conflict_msg<M: ToString>(self, msg: M) -> Result<T, AppError> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                if let Some(sql_err) = e.sql_err() {
                    return match sql_err {
                        SqlErr::UniqueConstraintViolation(_) => {
                            Err(AppError::AlreadyExists(msg.to_string()))
                        }
                        SqlErr::ForeignKeyConstraintViolation(_) => {
                            Err(AppError::BadRequest("外键约束失败".to_string()))
                        }
                        _ => Err(AppError::InternalServerError(e.into())),
                    };
                }
                match &e {
                    DbErr::RecordNotFound(_) => Err(AppError::NotFound("记录未找到".to_string())),
                    _ => Err(AppError::InternalServerError(e.into())),
                }
            }
        }
    }
}

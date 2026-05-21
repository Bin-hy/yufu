use axum::response::{IntoResponse, Response};
use http::StatusCode;
use sea_orm::{DbErr, SqlErr};
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
        match self {
            AppError::NoAvailableDevice => (
                StatusCode::SERVICE_UNAVAILABLE,
                "no available device".to_string(),
            )
                .into_response(),
            AppError::RequestProxyError => {
                (StatusCode::BAD_REQUEST, "request error".to_string()).into_response()
            }
            AppError::NotFound(err) => (StatusCode::NOT_FOUND, err).into_response(),
            AppError::AlreadyExists(err) => (StatusCode::CONFLICT, err).into_response(),
            AppError::InternalServerError(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
            }
            AppError::Throw(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
            AppError::Unauthorized(err) => (StatusCode::UNAUTHORIZED, err).into_response(),
            AppError::BadRequest(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
            AppError::Forbidden(err) => (StatusCode::FORBIDDEN, err.to_string()).into_response(),
        }
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

/// 为 SeaORM 的 `Result<T, DbErr>` 提供扩展方法，
/// `.into_app()?` 将数据库错误自动转为 `AppError`。
pub trait SeaResultExt<T> {
    /// 转换为 `Result<T, AppError>`，使用默认错误提示。
    fn into_app(self) -> Result<T, AppError>;

    /// 转换为 `Result<T, AppError>`，
    /// 当遇到唯一约束冲突（UniqueConstraintViolation）时，
    /// 使用你提供的 `msg` 作为提示。
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

                // 进一步匹配 SeaORM 的其他错误类型
                match &e {
                    DbErr::RecordNotFound(_) => Err(AppError::NotFound("记录未找到".to_string())),
                    _ => Err(AppError::InternalServerError(e.into())),
                }
            }
        }
    }
}

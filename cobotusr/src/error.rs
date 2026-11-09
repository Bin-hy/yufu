use axum::response::{IntoResponse, Response};
use http::StatusCode;
use sea_orm::{DbErr, SqlErr};
#[derive(Debug)]
pub enum AppError {
    Identity {
        reason: crate::domain::identity::error::IdentityError,
        request_id: uuid::Uuid,
    },
    Security(auth::security::error::SecurityError),
    BrowserRejected(BrowserRejection),
    NotFound(String),
    AlreadyExists(String),
    Throw(String),
    InternalServerError(anyhow::Error),
    Unauthorized(String),
    BadRequest(String),
    Forbidden(String),
}

/// Public transport failures only; never include proofs, cookies, or signing details.
#[derive(Debug, Clone, Copy)]
pub enum BrowserRejection {
    OriginMissing,
    OriginInvalid,
    OriginNotAllowed,
    CookieInvalid,
    CookieMissing,
    CookieRequiresHttps,
    ProofMissing,
    ProofMismatch,
    ProofInvalid,
}

impl BrowserRejection {
    pub fn code(self) -> &'static str {
        match self {
            Self::OriginMissing => "csrf_origin_missing",
            Self::OriginInvalid => "csrf_origin_invalid",
            Self::OriginNotAllowed => "csrf_origin_not_allowed",
            Self::CookieInvalid => "csrf_cookie_invalid",
            Self::CookieMissing => "csrf_cookie_missing",
            Self::CookieRequiresHttps => "csrf_cookie_requires_https",
            Self::ProofMissing => "csrf_proof_missing",
            Self::ProofMismatch => "csrf_proof_mismatch",
            Self::ProofInvalid => "csrf_proof_invalid",
        }
    }
}

impl AppError {
    pub fn identity(
        reason: crate::domain::identity::error::IdentityError,
        request_id: uuid::Uuid,
    ) -> Self {
        Self::Identity { reason, request_id }
    }
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
            AppError::Identity { reason, request_id } => {
                use crate::domain::identity::error::IdentityError;
                let status = match reason {
                    IdentityError::Unauthenticated | IdentityError::LoginRequired => StatusCode::UNAUTHORIZED,
                    IdentityError::Forbidden | IdentityError::CookieRequiresHttps | IdentityError::SelfApproval | IdentityError::AccessDenied | IdentityError::ConsentRequired => StatusCode::FORBIDDEN,
                    IdentityError::NotFound => StatusCode::NOT_FOUND,
                    IdentityError::Conflict | IdentityError::AlreadyConsumed | IdentityError::OwnerRequired | IdentityError::LastAdministrator | IdentityError::ApprovalObsolete => StatusCode::CONFLICT,
                    IdentityError::Expired => StatusCode::GONE,
                    IdentityError::InvalidInput | IdentityError::InvalidScope => StatusCode::BAD_REQUEST,
                    IdentityError::DependencyUnavailable | IdentityError::DeadlineExceeded | IdentityError::CapacityExceeded | IdentityError::CommitUncertain => StatusCode::SERVICE_UNAVAILABLE,
                };
                tracing::warn!(%request_id, reason = reason.code(), "identity request rejected");
                (status, [(http::header::CACHE_CONTROL, "no-store".to_owned()), (http::HeaderName::from_static("x-request-id"), request_id.to_string())], axum::Json(serde_json::json!({"code":reason.code(),"request_id":request_id}))).into_response()
            }
            AppError::BrowserRejected(reason) => (
                StatusCode::FORBIDDEN,
                [(http::header::CACHE_CONTROL, "no-store")],
                axum::Json(serde_json::json!({"code": reason.code(), "message": "browser authentication rejected"})),
            )
                .into_response(),
            AppError::Security(err) => {
                use auth::security::error::SecurityError;
                let status = match err {
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
                (status, err.to_string()).into_response()
            }
            AppError::NotFound(err) => (StatusCode::NOT_FOUND, err).into_response(),
            AppError::AlreadyExists(err) => (StatusCode::CONFLICT, err).into_response(),
            AppError::InternalServerError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_server_error").into_response()
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
        let err = err.into();
        if let Some(identity) = err.downcast_ref::<crate::domain::identity::error::IdentityError>()
        {
            AppError::identity(*identity, uuid::Uuid::new_v4())
        } else if let Some(security) = err.downcast_ref::<auth::security::error::SecurityError>() {
            AppError::Security(*security)
        } else {
            AppError::InternalServerError(err)
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use auth::security::error::SecurityError;

    #[test]
    fn security_errors_keep_their_sanitized_http_classification() {
        for (error, expected) in [
            (SecurityError::InvalidCredential, StatusCode::UNAUTHORIZED),
            (SecurityError::CredentialRevoked, StatusCode::UNAUTHORIZED),
            (SecurityError::Forbidden, StatusCode::FORBIDDEN),
            (SecurityError::InvalidResource, StatusCode::BAD_REQUEST),
            (SecurityError::Conflict, StatusCode::CONFLICT),
            (
                SecurityError::DependencyUnavailable,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                SecurityError::DeadlineExceeded,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                SecurityError::CapacityExceeded,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
        ] {
            assert_eq!(AppError::from(error).into_response().status(), expected);
        }
    }
}

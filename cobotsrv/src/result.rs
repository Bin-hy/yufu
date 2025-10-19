use serde::{Deserialize, Serialize};
use std::result;

use crate::error::AppError;

pub type Result<T> = result::Result<T, AppError>;

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32, // service status code
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(data),
        }
    }

    pub fn error_with_code(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }
}

/// 业务状态码定义（0 表示成功）
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BizCode {
    Success = 0,
    Unauthorized = 401,
    NotFound = 1001,
    AlreadyExists = 1002,
    NoAvailableDevice = 2001,
    RequestProxyError = 2002,
    InternalServerError = 5000,
    BizError = 4000,
}

impl BizCode {
    pub fn message(&self) -> &'static str {
        match self {
            BizCode::Success => "success",
            BizCode::Unauthorized => "unauthorized",
            BizCode::NotFound => "resource not found",
            BizCode::AlreadyExists => "already exists",
            BizCode::NoAvailableDevice => "no available device",
            BizCode::RequestProxyError => "request proxy error",
            BizCode::InternalServerError => "internal server error",
            BizCode::BizError => "business error",
        }
    }

    pub fn code(&self) -> i32 {
        *self as i32
    }
}

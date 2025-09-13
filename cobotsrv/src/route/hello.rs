use anyhow::anyhow;
use axum::{
    extract::{Query, State},
    response::Response,
    routing::get,
    Json, Router,
};
use http::StatusCode;

use crate::AppState;
use crate::error::AppError;
use crate::result::Result;

use serde::{Deserialize, Serialize};
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Hello {
    #[serde(default)]
    pub alias: String,
}

#[derive(Deserialize)]
struct Code {
    #[serde(default)]
    code: u16,
    #[serde(default)]
    msg: String,
}

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/hello", get(hello))
        .route("/error", get(error).post(error).delete(error).patch(error))
}

async fn hello(
    State(state): State<AppState>,
) -> Result<Json<Hello>> {
    Ok(Json(
        Hello {
            #[cfg(feature = "net4mqtt")]
            alias: state.config.net4mqtt.unwrap().alias,
            #[cfg(not(feature = "net4mqtt"))]
            alias: state.config.http.listen.to_string(),
        }
    ))
}

async fn error(
    Query(req): Query<Code>,
) -> Result<Response<String>> {
    if let Ok(code) = http::StatusCode::from_u16(req.code) {
        match code {
            StatusCode::NOT_FOUND => Err(AppError::not_found(req.msg)),
            StatusCode::CONFLICT => Err(AppError::already_exists(req.msg)),
            StatusCode::INTERNAL_SERVER_ERROR => Err(AppError::InternalServerError(anyhow!(req.msg))),
            _ => Ok(if req.msg.is_empty() {
                Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body("".to_string())?
            } else {
                    Response::builder()
                        .status(StatusCode::OK)
                        .body(req.msg)?
                })
        }
    } else {
        Err(AppError::throw("query code is error"))
    }
}

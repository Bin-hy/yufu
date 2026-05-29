use axum::{Router, extract::State, response::IntoResponse, routing::get};

use crate::AppState;
use crate::result::Result;

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/hello", get(hello))
        .route("/error", get(error))
}

async fn hello(State(_state): State<AppState>) -> impl IntoResponse {
    "Hello, I am cobotrix".to_string()
}

async fn error() -> Result<()> {
    Err(crate::error::AppError::Throw(
        "This is a simulated error".to_string(),
    ))
}

use axum::{
    Router,
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
    routing::get,
};
use http::{HeaderValue, Uri, header};
use serde::{Deserialize, Serialize};
use tracing::Span;

use crate::store::Node;
use crate::{AppState, error::AppError, result::Result};

#[derive(Serialize, Deserialize, Clone)]
pub struct QueryExtract {
    #[serde(default)]
    pub nodes: Vec<String>,
}

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/remote/{device}/{module}", 
            get(remote).
                post(remote).
                patch(remote).
                delete(remote)
        )
}

async fn remote(
    State(state): State<AppState>,
    Path((device, module)): Path<(String, String)>,
    mut req: Request,
) -> Result<Response> {
    let uri = format!("/{module}");
    *req.uri_mut() = Uri::try_from(uri).unwrap();

    match state.storage.get_map_nodes().get(&device) {
        Some(server) => request_proxy(state, req, server).await,
        None => Err(AppError::NoAvailableDevice),
    }
}

async fn request_proxy(state: AppState, mut req: Request, target: &Node) -> Result<Response> {
    Span::current().record("target", target.url.clone());
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);
    let uri = format!("{}{}", target.url, path_query);
    *req.uri_mut() = Uri::try_from(uri).unwrap();
    req.headers_mut().remove("Authorization");
    if !target.token.is_empty() {
        req.headers_mut().insert(
            &header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", target.token))?,
        );
    };

    let (headers, body) = req.into_parts();
    use http_body_util::BodyExt;
    let body = body.collect().await.unwrap().to_bytes();
    let req: Request<axum::body::Bytes> = Request::from_parts(headers, body);
    let req = reqwest::Request::try_from(req).unwrap();

    let res = state
        .client
        .execute(req)
        .await
        .map_err(|_| AppError::RequestProxyError)?;
    let res = http::Response::from(res);
    Ok(res.into_response())
}

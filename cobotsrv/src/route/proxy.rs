use axum::body::Body;
use axum::{
    Router,
    extract::{Request, State},
    response::Response,
    routing::any,
};
use http::{HeaderName, HeaderValue, StatusCode};
use tracing::warn;

use crate::AppState;
use crate::entity::user;

/// Proxy management requests to cobotrix after auth.
///
/// Forwards the original `Authorization` header so cobotrix can independently
/// verify the JWT (defense in depth). Also injects `X-User-Id`, `X-Username`,
/// `X-Role-Id` as enrichment for downstream code that doesn't want to re-decode
/// the token; cobotrix should treat these as advisory and cross-check against
/// the verified token's `sub` claim.
pub fn route() -> Router<AppState> {
    Router::new().route("/mgmt/{*path}", any(proxy))
}

async fn proxy(
    State(state): State<AppState>,
    mut req: Request,
) -> Result<Response<Body>, (StatusCode, String)> {
    let user = req
        .extensions()
        .get::<user::Model>()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "user not found in request".to_string(),
            )
        })?
        .clone();

    // Build target URL
    let original_uri = req.uri().clone();
    let path_query = original_uri
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or("");
    let target_path = path_query.strip_prefix("/mgmt").unwrap_or(path_query);
    let target_url = format!("{}{}", state.config.http.cobotrix_url, target_path);

    // Inject trusted enrichment headers (Authorization is preserved for cobotrix to re-verify).
    let headers = req.headers_mut();
    headers.insert(
        HeaderName::from_static("x-user-id"),
        HeaderValue::from_str(&user.id.to_string())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    );
    headers.insert(
        HeaderName::from_static("x-username"),
        HeaderValue::from_str(&user.username)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    );
    headers.insert(
        HeaderName::from_static("x-role-id"),
        HeaderValue::from_str(&user.role_id.to_string())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    );

    // Forward to cobotrix — extract body as bytes, rebuild request
    let (parts, axum_body) = req.into_parts();
    use http_body_util::BodyExt;
    let body_bytes = axum_body
        .collect()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .to_bytes();
    let forward_req: http::Request<axum::body::Bytes> =
        http::Request::from_parts(parts, body_bytes);
    let mut forward_req = reqwest::Request::try_from(forward_req).map_err(|e| {
        warn!(error = %e, "failed to convert request for proxy");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    *forward_req.url_mut() = target_url
        .parse()
        .map_err(|e: url::ParseError| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let client = reqwest::Client::new();
    let res = client.execute(forward_req).await.map_err(|e| {
        warn!(error = %e, "cobotrix proxy failed");
        (
            StatusCode::BAD_GATEWAY,
            format!("cobotrix unreachable: {e}"),
        )
    })?;

    let status = res.status();
    let headers = res.headers().clone();
    let body_bytes = res
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let mut response = Response::new(Body::from(body_bytes));
    *response.status_mut() = status;
    response.headers_mut().extend(headers);
    Ok(response)
}

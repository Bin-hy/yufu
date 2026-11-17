//! Public branding responses contain presentation data only.
use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{error::IdentityError, site_branding::StoredSiteAsset},
    error::AppError,
    service::identity::{repository::PgIdentityRepository, site_settings::SiteSettingsService},
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::get,
};
use http::{HeaderMap, HeaderValue, StatusCode};
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn service(state: &AppState) -> Result<SiteSettingsService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(SiteSettingsService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    ))
}
pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/site/branding", get(branding))
        .route("/site/assets/{id}", get(asset))
}
async fn branding(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
) -> Result<Response, AppError> {
    let result = async { service(&state)?.public_branding(&ctx.budget).await }.await;
    result
        .map(|value| ([(http::header::CACHE_CONTROL, "no-store")], Json(value)).into_response())
        .map_err(|e| AppError::identity(e, ctx.id))
}
async fn asset(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let result = async { service(&state)?.asset(None, id, &ctx.budget).await }.await;
    result
        .map(|asset| asset_response(asset, &headers, true))
        .map_err(|e| AppError::identity(e, ctx.id))
}
pub(super) fn asset_response(
    asset: StoredSiteAsset,
    request: &HeaderMap,
    public: bool,
) -> Response {
    let etag = format!("\"{}\"", asset.sha256);
    let matched = public
        && request
            .get(http::header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(',').any(|v| {
                    v.trim() == etag
                        || v.trim() == "*"
                        || v.trim().strip_prefix("W/") == Some(etag.as_str())
                })
            });
    let mut response = if matched {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        asset.bytes.into_response()
    };
    let h = response.headers_mut();
    h.insert(
        http::header::CACHE_CONTROL,
        HeaderValue::from_static(if public {
            "no-cache, must-revalidate"
        } else {
            "no-store"
        }),
    );
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if let Ok(value) = HeaderValue::from_str(&asset.mime) {
        h.insert(http::header::CONTENT_TYPE, value);
    }
    if public && let Ok(value) = HeaderValue::from_str(&etag) {
        h.insert(http::header::ETAG, value);
    }
    response
}

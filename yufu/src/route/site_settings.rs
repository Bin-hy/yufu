use super::{
    identity_browser::IdentityRequest,
    site_branding::{asset_response, service},
};
use crate::{
    AppState,
    domain::identity::{operation::IdentityOperation, site_branding::*},
    error::AppError,
    service::identity::{browser, site_settings::admin_response},
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State, rejection::JsonRejection},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use http::{HeaderMap, StatusCode};
use serde::Deserialize;
use std::time::Duration;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/site-settings", get(read).put(save))
        .route("/admin/site-settings/reset", post(reset))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .route("/admin/site-assets", post(upload))
        .route("/admin/site-assets/{id}", get(preview))
}
fn json_no_store(value: serde_json::Value) -> Response {
    ([(http::header::CACHE_CONTROL, "no-store")], Json(value)).into_response()
}
async fn read(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let result = async {
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        service(&state)?.admin_settings(&secret, &ctx.budget).await
    }
    .await;
    result
        .map(|s| json_no_store(admin_response(s)))
        .map_err(|e| AppError::identity(e, ctx.id))
}
async fn save(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<SaveSiteSettings>, JsonRejection>,
) -> Result<Response, AppError> {
    let result = async {
        let clock = state.security_clock()?;
        let evidence =
            browser::verify_write(&state.config, clock.as_ref(), &headers, None, &ctx.budget)?;
        let Json(input) =
            body.map_err(|_| crate::domain::identity::error::IdentityError::InvalidInput)?;
        service(&state)?
            .save(&evidence, input, ctx.id, &ctx.budget)
            .await
    }
    .await;
    result
        .map(|s| json_no_store(admin_response(s)))
        .map_err(|e| AppError::identity(e, ctx.id))
}
async fn reset(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<ResetSiteSettings>, JsonRejection>,
) -> Result<Response, AppError> {
    let result = async {
        let clock = state.security_clock()?;
        let evidence =
            browser::verify_write(&state.config, clock.as_ref(), &headers, None, &ctx.budget)?;
        let Json(input) =
            body.map_err(|_| crate::domain::identity::error::IdentityError::InvalidInput)?;
        service(&state)?
            .reset(&evidence, input.expected_revision, ctx.id, &ctx.budget)
            .await
    }
    .await;
    result
        .map(|s| json_no_store(admin_response(s)))
        .map_err(|e| AppError::identity(e, ctx.id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadQuery {
    kind: SiteAssetKind,
}
fn upload_error(status: StatusCode, code: &str, id: Uuid) -> Response {
    (
        status,
        [(http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({"code":code,"request_id":id})),
    )
        .into_response()
}
async fn upload(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
    Query(query): Query<UploadQuery>,
    request: Request,
) -> Result<Response, AppError> {
    let headers = request.headers();
    let clock = state
        .security_clock()
        .map_err(|e| AppError::identity(e.into(), ctx.id))?;
    let evidence = browser::verify_write(&state.config, clock.as_ref(), headers, None, &ctx.budget)
        .map_err(|e| AppError::identity(e, ctx.id))?;
    let secret = browser::read_sso_cookie(&state.config, headers)
        .map_err(|e| AppError::identity(e, ctx.id))?;
    let service = service(&state).map_err(|e| AppError::identity(e, ctx.id))?;
    service
        .preflight(&secret, IdentityOperation::SiteAssetUpload, &ctx.budget)
        .await
        .map_err(|e| AppError::identity(e, ctx.id))?;
    let bytes = match tokio::time::timeout(
        Duration::from_secs(30),
        axum::body::to_bytes(request.into_body(), query.kind.max_bytes()),
    )
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            use std::error::Error;
            let limited = error
                .source()
                .is_some_and(|e| e.is::<http_body_util::LengthLimitError>());
            return Ok(upload_error(
                if limited {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                if limited {
                    "image_too_large"
                } else {
                    "invalid_image_body"
                },
                ctx.id,
            ));
        }
        Err(_) => {
            return Ok(upload_error(
                StatusCode::REQUEST_TIMEOUT,
                "upload_timeout",
                ctx.id,
            ));
        }
    };
    let asset = service
        .upload(&evidence, query.kind, bytes.to_vec(), &ctx.budget)
        .await
        .map_err(|e| AppError::identity(e, ctx.id))?;
    Ok((
        StatusCode::CREATED,
        [(http::header::CACHE_CONTROL, "no-store")],
        Json(asset),
    )
        .into_response())
}
async fn preview(
    State(state): State<AppState>,
    Extension(ctx): Extension<IdentityRequest>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let result = async {
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        service(&state)?.asset(Some(&secret), id, &ctx.budget).await
    }
    .await;
    result
        .map(|asset| asset_response(asset, &headers, false))
        .map_err(|e| AppError::identity(e, ctx.id))
}

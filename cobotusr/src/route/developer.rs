//! Authenticated control-panel APIs for applications and independent API readers.
use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{error::IdentityError, registration::CreateApplication},
    error::AppError,
    service::identity::{
        application::ApplicationService,
        browser,
        reader::{ReaderLocation, ReaderManagement, ReaderService},
        repository::PgIdentityRepository,
    },
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    routing::get,
};
use http::HeaderMap;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .merge(super::application_governance::get_routes())
        .merge(super::application_admin::get_routes())
        .route("/developer/apps", get(list_apps).post(register))
        .route("/developer/apps/{id}", get(detail))
        .route("/developer/operations/{id}", get(operation))
        .merge(reader_routes(
            "/developer/apps/{id}/environments/{env}/integration/readers",
            "/developer/apps/{id}/environments/{env}/integration/readers/{reader_id}",
            ReaderManagement::Developer,
        ))
        .merge(reader_routes(
            "/admin/apps/{id}/environments/{env}/integration/readers",
            "/admin/apps/{id}/environments/{env}/integration/readers/{reader_id}",
            ReaderManagement::Administrator,
        ))
}
pub(super) fn applications(state: &AppState) -> Result<ApplicationService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(ApplicationService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    ))
}
fn reader_routes(list: &str, item: &str, management: ReaderManagement) -> Router<AppState> {
    Router::new()
        .route(list, get(list_readers).post(create_reader))
        .route(item, axum::routing::patch(change_reader))
        .layer(Extension(management))
}
fn readers(state: &AppState, management: ReaderManagement) -> Result<ReaderService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(ReaderService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    )
    .with_management(management))
}
pub(super) fn single_key(headers: &HeaderMap) -> Result<&str, IdentityError> {
    let values: Vec<_> = headers.get_all("idempotency-key").iter().collect();
    if values.len() != 1 {
        return Err(IdentityError::InvalidInput);
    }
    values[0].to_str().map_err(|_| IdentityError::InvalidInput)
}
pub(super) type ManagementResponse = Result<(HeaderMap, Json<serde_json::Value>), AppError>;
pub(super) fn response<T: serde::Serialize>(
    result: Result<T, IdentityError>,
    id: Uuid,
) -> ManagementResponse {
    let value = result
        .and_then(|value| {
            serde_json::to_value(value).map_err(|_| IdentityError::DependencyUnavailable)
        })
        .map_err(|reason| AppError::identity(reason, id))?;
    Ok((browser::protected_headers(), Json(value)))
}
async fn register(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<CreateApplication>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Json(request) = body.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(&state.config, state.security_clock()?.as_ref(), &headers, None, &context.budget)?;
        let receipt = applications(&state)?.register(&evidence, request, single_key(&headers)?, context.id, &context.budget).await?;
        let secrets: Vec<_> = receipt.newly_issued_secrets.into_iter().map(|(environment_id, secret)|
            serde_json::json!({"environment_id":environment_id,"client_secret":secret.into_response()})).collect();
        Ok::<_, IdentityError>(serde_json::json!({"operation_id":receipt.operation_id,"application_id":receipt.application_id,
            "management_mode":receipt.management_mode,"environments":receipt.environments,"secrets":secrets}))
    }.await;
    response(result, context.id)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppPageQuery {
    #[serde(default)]
    after: i32,
    #[serde(default = "page_size")]
    limit: u32,
}
fn page_size() -> u32 {
    25
}
async fn list_apps(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    query: Result<Query<AppPageQuery>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Query(page) = query.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .list(&secret, page.after, page.limit, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}
async fn detail(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .detail(&secret, id, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}
async fn operation(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .operation(&secret, id, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReaderCreateBody {
    audience: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReaderChangeBody {
    expected_revision: u64,
    enabled: bool,
    reset_secret: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReaderPageQuery {
    after: Option<Uuid>,
    #[serde(default = "page_size")]
    limit: u32,
}
async fn list_readers(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Extension(management): Extension<ReaderManagement>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    query: Result<Query<ReaderPageQuery>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((application_id, environment_id)) =
            path.map_err(|_| IdentityError::InvalidInput)?;
        let Query(page) = query.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        readers(&state, management)?
            .list(
                &secret,
                ReaderLocation {
                    application_id,
                    environment_id,
                },
                page.after,
                page.limit,
                &context.budget,
            )
            .await
    }
    .await;
    response(result, context.id)
}
async fn create_reader(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Extension(management): Extension<ReaderManagement>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    body: Result<Json<ReaderCreateBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((application_id, environment_id)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(&state.config, state.security_clock()?.as_ref(), &headers, None, &context.budget)?;
        let reader = readers(&state, management)?.create(&evidence, ReaderLocation { application_id, environment_id }, &body.audience, single_key(&headers)?, context.id, &context.budget).await?;
        let mut value=serde_json::json!({"id":reader.id,"reader_id":reader.reader_id,"revision":reader.revision,"operation_id":reader.operation_id});
        if let Some(secret)=reader.secret { value["secret"]=serde_json::json!(secret.into_response()); }
        Ok::<_, IdentityError>(value)
    }.await;
    response(result, context.id)
}
async fn change_reader(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Extension(management): Extension<ReaderManagement>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid, String)>, PathRejection>,
    body: Result<Json<ReaderChangeBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((application_id, environment_id, reader_id)) =
            path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let changed = readers(&state, management)?
            .change(
                &evidence,
                &reader_id,
                ReaderLocation {
                    application_id,
                    environment_id,
                },
                body.expected_revision,
                body.enabled,
                body.reset_secret,
                single_key(&headers)?,
                context.id,
                &context.budget,
            )
            .await?;
        let mut value =
            serde_json::json!({"revision":changed.revision,"operation_id":changed.operation_id});
        if let Some(secret) = changed.secret {
            value["secret"] = serde_json::json!(secret.into_response());
        }
        Ok::<_, IdentityError>(value)
    }
    .await;
    response(result, context.id)
}

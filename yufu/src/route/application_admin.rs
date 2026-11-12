use super::{
    application_governance::change,
    developer::{ManagementResponse, applications, response, single_key},
    identity_browser::IdentityRequest,
};
use crate::{
    AppState,
    domain::identity::{
        application::EnvironmentConfigInput, error::IdentityError,
        registration::CreateAdminApplication,
    },
    service::identity::{application::ApplicationMutation, browser},
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    routing::{patch, post, put},
};
use http::HeaderMap;
use serde::Deserialize;
use uuid::Uuid;
pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/apps", post(register))
        .route("/admin/apps/{id}", patch(metadata_admin))
        .route("/developer/apps/{id}", patch(metadata_developer))
        .route("/admin/apps/{id}/environments/{env}/config", put(config))
        .route(
            "/admin/apps/{id}/environments/{env}/activate",
            post(activate),
        )
        .route(
            "/admin/apps/{id}/environments/{env}/secret/reset",
            post(reset),
        )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigBody {
    expected_revision: u64,
    config: EnvironmentConfigInput,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataBody {
    name: String,
    description: Option<String>,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivationBody {
    version_id: Uuid,
    expected_revision: u64,
    accept_logout_limitation: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionBody {
    expected_revision: u64,
}
async fn register(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<CreateAdminApplication>, JsonRejection>,
) -> ManagementResponse {
    let result=async {
        let Json(body)=body.map_err(|_|IdentityError::InvalidInput)?;
        let evidence=browser::verify_write(&state.config,state.security_clock()?.as_ref(),&headers,None,&context.budget)?;
        let receipt=applications(&state)?.register_administrator(&evidence,body,single_key(&headers)?,context.id,&context.budget).await?;
        let secrets:Vec<_>=receipt.newly_issued_secrets.into_iter().map(|(environment_id,secret)|serde_json::json!({"environment_id":environment_id,"client_secret":secret.into_response()})).collect();
        Ok::<_,IdentityError>(serde_json::json!({"operation_id":receipt.operation_id,"application_id":receipt.application_id,"management_mode":receipt.management_mode,"environments":receipt.environments,"newly_issued_secrets":secrets}))
    }.await;
    response(result, context.id)
}
async fn metadata_developer(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<MetadataBody>, JsonRejection>,
) -> ManagementResponse {
    metadata(&state, &context, &headers, path, body, false).await
}
async fn metadata_admin(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<MetadataBody>, JsonRejection>,
) -> ManagementResponse {
    metadata(&state, &context, &headers, path, body, true).await
}
async fn metadata(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<MetadataBody>, JsonRejection>,
    admin: bool,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            state,
            context,
            headers,
            id,
            ApplicationMutation::UpdateMetadata {
                name: body.name,
                description: body.description,
                expected_revision: body.expected_revision,
            },
            admin,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn config(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    body: Result<Json<ConfigBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, env)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::UpdateConfig {
                environment_id: env,
                config: body.config,
                expected_revision: body.expected_revision,
            },
            true,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn activate(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    body: Result<Json<ActivationBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, env)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::ActivateProduction {
                environment_id: env,
                version_id: body.version_id,
                expected_revision: body.expected_revision,
                accept_logout_limitation: body.accept_logout_limitation,
            },
            true,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn reset(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    body: Result<Json<RevisionBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, env)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::ResetSecret {
                environment_id: env,
                expected_revision: body.expected_revision,
            },
            true,
        )
        .await
    }
    .await;
    response(result, context.id)
}

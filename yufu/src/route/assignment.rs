use super::{
    developer::{ManagementResponse, response, single_key},
    identity_browser::IdentityRequest,
};
use crate::{
    AppState,
    domain::identity::error::IdentityError,
    service::identity::{
        assignment::{AssignmentMutation, AssignmentService},
        browser,
        repository::{AssignmentTarget, PgIdentityRepository},
    },
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    routing::{get, patch},
};
use http::HeaderMap;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/groups", get(groups).post(create))
        .route(
            "/admin/groups/{id}",
            get(group).patch(update).delete(delete),
        )
        .route("/admin/groups/{id}/members/{user_id}", patch(member))
        .route(
            "/admin/apps/{id}/assignments",
            get(assignments).patch(assign),
        )
        .route(
            "/admin/apps/{id}/environments/{env}/test-users/{user_id}",
            patch(test_user),
        )
}
fn service(state: &AppState) -> Result<AssignmentService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(AssignmentService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    after: Option<Uuid>,
    #[serde(default = "limit")]
    limit: u32,
    q: Option<String>,
}
fn limit() -> u32 {
    25
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    name: String,
    description: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    name: String,
    description: Option<String>,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberBody {
    present: bool,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionBody {
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignmentBody {
    target: AssignmentTarget,
    present: bool,
    expected_revision: u64,
}
async fn mutate(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    mutation: AssignmentMutation,
) -> Result<serde_json::Value, IdentityError> {
    let evidence = browser::verify_write(
        &state.config,
        state.security_clock()?.as_ref(),
        headers,
        None,
        &context.budget,
    )?;
    let receipt = service(state)?
        .mutate(
            &evidence,
            mutation,
            single_key(headers)?,
            context.id,
            &context.budget,
        )
        .await?;
    Ok(serde_json::json!({"operation_id":receipt.operation_id,"summary":receipt.summary}))
}
async fn groups(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    query: Result<Query<Page>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Query(page) = query.map_err(|_| IdentityError::InvalidInput)?;
        service(&state)?
            .groups(
                &browser::read_sso_cookie(&state.config, &headers)?,
                page.after,
                page.limit,
                page.q.as_deref(),
                &context.budget,
            )
            .await
    }
    .await;
    response(result, context.id)
}
async fn group(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        service(&state)?
            .group(
                &browser::read_sso_cookie(&state.config, &headers)?,
                id,
                &context.budget,
            )
            .await
    }
    .await;
    response(result, context.id)
}
async fn assignments(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        service(&state)?
            .assignments(
                &browser::read_sso_cookie(&state.config, &headers)?,
                id,
                &context.budget,
            )
            .await
    }
    .await;
    response(result, context.id)
}
async fn create(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<CreateBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        mutate(
            &state,
            &context,
            &headers,
            AssignmentMutation::CreateGroup {
                name: body.name,
                description: body.description,
            },
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn update(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<UpdateBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        mutate(
            &state,
            &context,
            &headers,
            AssignmentMutation::UpdateGroup {
                group_id: id,
                name: body.name,
                description: body.description,
                expected_revision: body.expected_revision,
            },
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn delete(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<RevisionBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        mutate(
            &state,
            &context,
            &headers,
            AssignmentMutation::DeleteGroup {
                group_id: id,
                expected_revision: body.expected_revision,
            },
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn member(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, i32)>, PathRejection>,
    body: Result<Json<MemberBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, user)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        mutate(
            &state,
            &context,
            &headers,
            AssignmentMutation::SetGroupMember {
                group_id: id,
                user_id: user,
                present: body.present,
                expected_revision: body.expected_revision,
            },
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn assign(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<AssignmentBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        mutate(
            &state,
            &context,
            &headers,
            AssignmentMutation::SetAssignment {
                application_id: id,
                target: body.target,
                present: body.present,
                expected_revision: body.expected_revision,
            },
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn test_user(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid, i32)>, PathRejection>,
    body: Result<Json<MemberBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, env, user)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        mutate(
            &state,
            &context,
            &headers,
            AssignmentMutation::SetTestUser {
                application_id: id,
                environment_id: env,
                user_id: user,
                present: body.present,
                expected_revision: body.expected_revision,
            },
        )
        .await
    }
    .await;
    response(result, context.id)
}

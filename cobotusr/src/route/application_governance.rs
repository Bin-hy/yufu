//! Each route selects an operation; control-panel writes require center CSRF evidence.
use super::{
    developer::{ManagementResponse, applications, response, single_key},
    identity_browser::IdentityRequest,
};
use crate::{
    AppState,
    domain::identity::{application::EnvironmentConfigInput, error::IdentityError},
    service::identity::{application::ApplicationMutation, browser},
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    routing::{get, patch, post, put},
};
use http::HeaderMap;
use serde::Deserialize;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/developer/collaboration-candidates", get(candidates))
        .route(
            "/developer/apps/{id}/environments/{env}/config",
            put(update_config),
        )
        .route(
            "/developer/apps/{id}/environments/{env}/submit",
            post(submit),
        )
        .route(
            "/developer/apps/{id}/environments/{env}/secret/reset",
            post(reset_secret),
        )
        .route("/developer/apps/{id}/members/{user_id}", patch(member))
        .route("/developer/apps/{id}/owner", patch(owner))
        .route(
            "/developer/apps/{id}/approvals/{approval_id}",
            get(developer_approval),
        )
        .route("/admin/apps", get(admin_list))
        .route("/admin/apps/{id}", get(admin_detail))
        .route("/admin/apps/{id}/lifecycle", patch(lifecycle))
        .route("/admin/apps/{id}/owner", patch(admin_owner))
        .route(
            "/admin/apps/{id}/approvals/{approval_id}/review",
            post(review),
        )
        .route(
            "/admin/apps/{id}/approvals/{approval_id}",
            get(admin_approval),
        )
        .route("/admin/collaboration-candidates", patch(candidate_change))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigBody {
    expected_revision: u64,
    config: EnvironmentConfigInput,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitBody {
    expected_revision: u64,
    version_id: Uuid,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionBody {
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberBody {
    expected_revision: u64,
    present: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerBody {
    expected_revision: u64,
    user_id: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleBody {
    expected_revision: u64,
    active: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewBody {
    expected_revision: u64,
    approve: bool,
    reason: Option<String>,
    accept_logout_limitation: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateBody {
    requester_user_id: i32,
    candidate_user_id: i32,
    allowed: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    after: i32,
    #[serde(default = "page_size")]
    limit: u32,
}
fn page_size() -> u32 {
    25
}
async fn developer_approval(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
) -> ManagementResponse {
    approval_detail(&state, &context, &headers, path, false).await
}
async fn admin_approval(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
) -> ManagementResponse {
    approval_detail(&state, &context, &headers, path, true).await
}
async fn approval_detail(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    admin: bool,
) -> ManagementResponse {
    let result = async {
        let Path((id, approval_id)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, headers)?;
        applications(state)?
            .approval(&secret, id, approval_id, admin, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}

pub(super) async fn change(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    id: i32,
    mutation: ApplicationMutation,
    admin: bool,
) -> Result<serde_json::Value, IdentityError> {
    let evidence = browser::verify_write(
        &state.config,
        state.security_clock()?.as_ref(),
        headers,
        None,
        &context.budget,
    )?;
    let service = applications(state)?;
    let changed = if admin {
        service
            .mutate_administrator(
                &evidence,
                id,
                mutation,
                single_key(headers)?,
                context.id,
                &context.budget,
            )
            .await?
    } else {
        service
            .mutate(
                &evidence,
                id,
                mutation,
                single_key(headers)?,
                context.id,
                &context.budget,
            )
            .await?
    };
    let mut value = serde_json::json!({"operation_id":changed.receipt.operation_id,"summary":changed.receipt.summary});
    if let Some(secret) = changed.newly_issued_secret {
        value["client_secret"] = serde_json::json!(secret.into_response());
    }
    Ok(value)
}
async fn update_config(
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
                expected_revision: body.expected_revision,
                config: body.config,
            },
            false,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn submit(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    body: Result<Json<SubmitBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, env)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::SubmitProduction {
                environment_id: env,
                expected_revision: body.expected_revision,
                version_id: body.version_id,
            },
            false,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn reset_secret(
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
            false,
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
    path: Result<Path<(i32, i32)>, PathRejection>,
    body: Result<Json<MemberBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, user_id)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::SetMember {
                user_id,
                expected_revision: body.expected_revision,
                present: body.present,
            },
            false,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn owner(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<OwnerBody>, JsonRejection>,
) -> ManagementResponse {
    owner_change(&state, &context, &headers, path, body, false).await
}
async fn admin_owner(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<OwnerBody>, JsonRejection>,
) -> ManagementResponse {
    owner_change(&state, &context, &headers, path, body, true).await
}
async fn owner_change(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<OwnerBody>, JsonRejection>,
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
            ApplicationMutation::TransferOwner {
                user_id: body.user_id,
                expected_revision: body.expected_revision,
            },
            admin,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn lifecycle(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<LifecycleBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::SetLifecycle {
                active: body.active,
                expected_revision: body.expected_revision,
            },
            true,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn review(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<(i32, Uuid)>, PathRejection>,
    body: Result<Json<ReviewBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Path((id, approval_id)) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        change(
            &state,
            &context,
            &headers,
            id,
            ApplicationMutation::ReviewProduction {
                approval_id,
                expected_revision: body.expected_revision,
                approve: body.approve,
                reason: body.reason,
                accept_logout_limitation: body.accept_logout_limitation,
            },
            true,
        )
        .await
    }
    .await;
    response(result, context.id)
}
async fn candidate_change(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<CandidateBody>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        applications(&state)?
            .set_candidate(
                &evidence,
                body.requester_user_id,
                body.candidate_user_id,
                body.allowed,
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>(serde_json::json!({"allowed":body.allowed}))
    }
    .await;
    response(result, context.id)
}
async fn candidates(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    query: Result<Query<Page>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Query(page) = query.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .candidates(&secret, page.after, page.limit, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}
async fn admin_list(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    query: Result<Query<Page>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Query(page) = query.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .list_administrator(&secret, page.after, page.limit, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}
async fn admin_detail(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .detail_administrator(&secret, id, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}

//! Enterprise operations use current center authentication and exact grants in the write transaction.
use super::identity_browser::{IdentityRequest, accounts};
use crate::{
    AppState,
    domain::identity::{
        account::{InvitationPurpose, PersonKind},
        error::IdentityError,
        session::ActionTarget,
    },
    error::AppError,
    service::identity::{account::InvitationRequest, browser},
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    routing::{patch, post},
};
use http::HeaderMap;
use serde::Deserialize;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/invitations", post(issue_invitation))
        .route("/admin/password-resets", post(issue_reset))
        .route("/admin/invitations/{id}/revoke", post(revoke_invitation))
        .route("/admin/password-resets/{id}/revoke", post(revoke_reset))
        .route("/admin/accounts/{id}/enabled", patch(set_enabled))
        .route("/admin/accounts/{id}/developer", patch(set_developer))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnabledBody {
    enabled: bool,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeveloperBody {
    developer_enabled: bool,
    expected_revision: u64,
}
async fn set_enabled(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<EnabledBody>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(user_id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let result = accounts(&state)?
            .set_enabled(
                &evidence,
                user_id,
                body.enabled,
                body.expected_revision,
                context.id,
                &context.budget,
            )
            .await?;
        let headers = if result.requires_login {
            browser::clear_center_cookies(&state.config)?
        } else {
            browser::protected_headers()
        };
        Ok::<_, IdentityError>((headers, Json(serde_json::json!(result))))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
async fn set_developer(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<DeveloperBody>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(user_id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(body) = body.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let result = accounts(&state)?
            .set_developer(
                &evidence,
                user_id,
                body.developer_enabled,
                body.expected_revision,
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>((
            browser::protected_headers(),
            Json(serde_json::json!(result)),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationBody {
    purpose: InvitationPurpose,
    target_user_id: Option<i32>,
    person_kind: PersonKind,
    developer_grant: bool,
}
async fn issue_invitation(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<InvitationBody>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Json(body) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(&state.config, clock.as_ref(), &headers, None, &context.budget)?;
        let action = accounts(&state)?.issue_invitation(&evidence, InvitationRequest {
            purpose: body.purpose, target_user_id: body.target_user_id,
            person_kind: body.person_kind, developer_grant: body.developer_grant,
        }, context.id, &context.budget).await?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(serde_json::json!({"id":action.id,"secret":action.secret.into_response(),"expires_at":action.expires_at/1000}))))
    }.await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetBody {
    user_id: i32,
}
async fn issue_reset(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<ResetBody>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Json(body) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(&state.config, clock.as_ref(), &headers, None, &context.budget)?;
        let action = accounts(&state)?.issue_reset(&evidence, body.user_id, context.id, &context.budget).await?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(serde_json::json!({"id":action.id,"secret":action.secret.into_response(),"expires_at":action.expires_at/1000}))))
    }.await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
async fn revoke_invitation(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    revoke(state, context, headers, path, false).await
}
async fn revoke_reset(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    revoke(state, context, headers, path, true).await
}
async fn revoke(
    state: AppState,
    context: IdentityRequest,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    reset: bool,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let target = if reset {
            ActionTarget::PasswordReset(id)
        } else {
            ActionTarget::Invitation(id)
        };
        accounts(&state)?
            .revoke_action(&evidence, target, context.id, &context.budget)
            .await?;
        Ok::<_, IdentityError>((
            browser::protected_headers(),
            Json(serde_json::json!({"revoked":true})),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

use super::identity_browser::{IdentityRequest, accounts, sessions, sso};
use crate::service::identity::{browser, sso::BrowserAuthentication};
use crate::{
    AppState,
    domain::identity::{error::IdentityError, session::IdentityRevocationScope},
    error::AppError,
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    routing::{get, post},
};
use http::HeaderMap;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/account/session", get(session))
        .route("/account/profile", get(session).patch(update_profile))
        .route("/account/password", post(change_password))
        .route("/account/sessions/{id}/revoke", post(revoke_session))
        .route("/account/logout-all", post(logout_all))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRequest {
    nickname: String,
    email: Option<String>,
}
async fn update_profile(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<ProfileRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let profile = accounts(&state)?
            .update_profile(
                &evidence,
                &payload.nickname,
                payload.email.as_deref(),
                context.id,
                &context.budget,
            )
            .await?;
        let mut value =
            serde_json::json!({"username":profile.preferred_username,"nickname":profile.name});
        if let Some(email) = profile.email {
            value["email"] = serde_json::json!(email);
        }
        if let Some(verified) = profile.email_verified {
            value["email_verified"] = serde_json::json!(verified);
        }
        Ok::<_, IdentityError>((browser::protected_headers(), Json(value)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordRequest {
    old_password: String,
    new_password: String,
}
async fn change_password(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<PasswordRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let old = crate::domain::identity::account::AccountPassword::new(payload.old_password)?;
        let new = crate::domain::identity::account::AccountPassword::new(payload.new_password)?;
        let receipt = accounts(&state)?
            .change_password(&evidence, &old, &new, context.id, &context.budget)
            .await?;
        Ok::<_, IdentityError>((
            browser::clear_center_cookies(&state.config)?,
            Json(serde_json::json!(receipt)),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

pub(crate) fn session_response(
    authentication: &BrowserAuthentication,
    csrf_token: &str,
) -> serde_json::Value {
    let mut result = serde_json::json!({
        "user_id": authentication.actor.account().user_id,
        "sub": authentication.actor.principal().subject(),
        "username":authentication.profile.preferred_username,
        "nickname":authentication.profile.name,
        "person_kind":authentication.actor.account().person_kind,
        "developer_enabled":authentication.actor.account().developer_enabled,
        "administrator":authentication.actor.account().administrator,
        "sso_session_id":authentication.root.id,
        "auth_time":authentication.root.authenticated_at / 1000,
        "expires_at":authentication.root.expires_at / 1000,
        "csrf_token":csrf_token,
    });
    if let Some(email) = &authentication.profile.email {
        result["email"] = serde_json::json!(email);
        if let Some(verified) = authentication.profile.email_verified {
            result["email_verified"] = serde_json::json!(verified);
        }
    }
    result
}

async fn session(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let authentication = sso(&state)?
            .authenticate_browser(&secret, &context.budget)
            .await?;
        let clock = state.security_clock()?;
        let challenge = browser::center_challenge(
            &state.config,
            clock.as_ref(),
            &headers,
            &authentication.root.secret_digest,
        )?;
        context.budget.remaining()?;
        Ok::<_, IdentityError>((
            challenge.headers,
            Json(session_response(
                &authentication,
                challenge.csrf_token.expose_secret(),
            )),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

async fn revoke_session(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let authentication = sso(&state)?
            .authenticate_browser(&secret, &context.budget)
            .await?;
        sessions(&state)?
            .revoke_personal(
                &secret,
                IdentityRevocationScope::CurrentCenter { sso_session_id: id },
                context.id,
                &context.budget,
            )
            .await?;
        let response_headers = if id == authentication.root.id {
            browser::clear_center_cookies(&state.config)?
        } else {
            browser::protected_headers()
        };
        Ok::<_, IdentityError>((response_headers, Json(serde_json::json!({"revoked":true}))))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

async fn logout_all(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let clock = state.security_clock()?;
        browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let authentication = sso(&state)?
            .authenticate_browser(&secret, &context.budget)
            .await?;
        sessions(&state)?
            .revoke_personal(
                &secret,
                IdentityRevocationScope::UserAll {
                    user_id: authentication.actor.account().user_id,
                },
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>((
            browser::clear_center_cookies(&state.config)?,
            Json(serde_json::json!({"revoked":true})),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

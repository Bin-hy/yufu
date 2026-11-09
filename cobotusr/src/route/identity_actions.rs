use super::identity_browser::{IdentityRequest, accounts, login_flow};
use crate::{
    AppState,
    domain::identity::{account::AccountPassword, error::IdentityError},
    error::AppError,
    service::identity::{
        account::{ActionProof, InvitationAcceptance},
        browser,
    },
};
use auth::security::identity::SecretToken;
use axum::{
    Extension, Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    routing::{get, post},
};
use http::HeaderMap;
use serde::Deserialize;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/identity/actions/exchange", post(exchange))
        .route("/identity/actions/{id}", get(inspect))
        .route("/identity/invitations/accept", post(accept))
        .route("/identity/password-resets/complete", post(reset))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExchangeRequest {
    kind: String,
    secret: String,
    flow_id: Option<Uuid>,
}
async fn exchange(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<ExchangeRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(&state.config, clock.as_ref(), &headers, None, &context.budget)?;
        let flow = login_flow(&state, &headers, payload.flow_id)?;
        let secret = SecretToken::new(payload.secret)?;
        let receipt = accounts(&state)?.exchange_action(&payload.kind, &secret, &evidence, flow.as_ref(), &context.budget).await?;
        let headers = browser::action_cookies(&state.config, receipt.action_id, receipt.proof, receipt.expires_at, crate::service::security::repository::now(clock.as_ref())?)?;
        Ok::<_, IdentityError>((headers, Json(serde_json::json!({"action_id":receipt.action_id,"expires_at":receipt.expires_at / 1000,"oidc_flow_id":receipt.oidc_flow_id,"next":format!("/identity/actions/{}",receipt.action_id)}))))
    }.await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
async fn inspect(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let proof = browser::read_action_proof(&state.config, &headers, id)?;
        let binding = browser::read_browser_binding(&state.config, &headers)?;
        let receipt = accounts(&state)?
            .inspect_action(
                ActionProof {
                    action_id: id,
                    proof: &proof,
                },
                &binding,
                &context.budget,
            )
            .await?;
        let clock = state.security_clock()?;
        let challenge = browser::write_challenge(&state.config, clock.as_ref(), &headers)?;
        let mut value = serde_json::json!(receipt);
        value["expires_at"] = serde_json::json!(receipt.expires_at / 1000);
        value["csrf_token"] = serde_json::json!(challenge.csrf_token.expose_secret());
        Ok::<_, IdentityError>((challenge.headers, Json(value)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptRequest {
    action_id: Uuid,
    existing_account: bool,
    username: Option<String>,
    password: Option<String>,
    nickname: Option<String>,
}
async fn accept(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<AcceptRequest>, JsonRejection>,
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
        let proof = browser::read_action_proof(&state.config, &headers, payload.action_id)?;
        let password = payload.password.map(AccountPassword::new).transpose()?;
        let acceptance = if payload.existing_account {
            if payload.username.is_some() || password.is_some() || payload.nickname.is_some() {
                return Err(IdentityError::InvalidInput);
            }
            InvitationAcceptance::ExistingAccount
        } else {
            InvitationAcceptance::NewAccount {
                username: payload
                    .username
                    .as_deref()
                    .ok_or(IdentityError::InvalidInput)?,
                password: password.as_ref().ok_or(IdentityError::InvalidInput)?,
                nickname: payload
                    .nickname
                    .as_deref()
                    .ok_or(IdentityError::InvalidInput)?,
            }
        };
        let receipt = accounts(&state)?
            .accept_invitation(
                &evidence,
                ActionProof {
                    action_id: payload.action_id,
                    proof: &proof,
                },
                acceptance,
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>((
            browser::clear_action_cookie(&state.config, payload.action_id)?,
            Json(serde_json::json!(receipt)),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetRequest {
    action_id: Uuid,
    new_password: String,
}
async fn reset(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<ResetRequest>, JsonRejection>,
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
        let proof = browser::read_action_proof(&state.config, &headers, payload.action_id)?;
        let password = AccountPassword::new(payload.new_password)?;
        let current_user_id = match browser::read_sso_cookie(&state.config, &headers) {
            Ok(secret) => super::identity_browser::sso(&state)?
                .authenticate_browser(&secret, &context.budget)
                .await
                .ok()
                .map(|authentication| authentication.actor.account().user_id),
            Err(_) => None,
        };
        let receipt = accounts(&state)?
            .reset_password(
                &evidence,
                ActionProof {
                    action_id: payload.action_id,
                    proof: &proof,
                },
                &password,
                context.id,
                &context.budget,
            )
            .await?;
        let mut response_headers = browser::clear_action_cookie(&state.config, payload.action_id)?;
        if current_user_id == Some(receipt.user_id) {
            for cookie in
                browser::clear_center_cookies(&state.config)?.get_all(http::header::SET_COOKIE)
            {
                response_headers.append(http::header::SET_COOKIE, cookie.clone());
            }
        }
        Ok::<_, IdentityError>((response_headers, Json(serde_json::json!(receipt))))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

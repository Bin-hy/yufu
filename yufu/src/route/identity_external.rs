use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{
        account::SecretDigest,
        error::IdentityError,
        external::{ExternalProviderKey, ExternalPurpose},
    },
    error::AppError,
    service::identity::{
        browser,
        external::{ExternalCallbackContext, ExternalCompletion, ExternalIdentityService},
        repository::PgIdentityRepository,
    },
};
use auth::security::budget::SecurityBudget;
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection},
    },
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use http::HeaderMap;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

fn service(state: &AppState) -> Result<ExternalIdentityService, IdentityError> {
    let clock = state.security_clock()?;
    ExternalIdentityService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
        state.config.clone(),
        state.external_registry_arc(),
        state.external_cipher_arc(),
        state.external_http().clone(),
        state.external_rate_limiter().clone(),
    )
}

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/identity/external/{provider}/start", post(start))
        .route("/identity/external/{provider}/bind/start", post(bind_start))
        .route("/account/identities", get(identities))
        .route("/account/identities/{id}/unbind", post(unbind))
}

pub fn get_callback_routes() -> Router<AppState> {
    Router::new().route("/identity/external/{provider}/callback", get(callback))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartRequest {
    flow_id: Option<Uuid>,
}

async fn start(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<StartRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let provider = provider
            .parse::<ExternalProviderKey>()
            .map_err(|_| IdentityError::InvalidInput)?;
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let started = service(&state)?
            .start_login(
                provider,
                payload.flow_id,
                &evidence.browser_binding_digest,
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>((
            browser::protected_headers(),
            Json(serde_json::json!({"authorize_url": started.authorize_url})),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindStartRequest {
    flow_id: Option<Uuid>,
}

async fn bind_start(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    payload: Option<Json<BindStartRequest>>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let provider = provider
            .parse::<ExternalProviderKey>()
            .map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        let evidence = browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let started = service(&state)?
            .start_bind(
                provider,
                payload.and_then(|Json(payload)| payload.flow_id),
                &secret,
                &evidence.browser_binding_digest,
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>((
            browser::protected_headers(),
            Json(serde_json::json!({"authorize_url": started.authorize_url})),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    /// RFC 9207 issuer identification (GitHub sends it). Informational only;
    /// trust comes from the fixed callback, state and browser binding.
    #[allow(dead_code)]
    iss: Option<String>,
}

async fn callback(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Path(provider): Path<String>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> Response {
    let provider = match provider.parse::<ExternalProviderKey>() {
        Ok(provider) => provider,
        Err(_) => {
            return redirect_response("/login?error=invalid_request", browser::protected_headers());
        }
    };
    let binding = browser::read_browser_binding(&state.config, &headers).ok();
    let center = crate::service::security::browser::cookies(&headers)
        .ok()
        .and_then(|cookies| {
            cookies
                .get(browser::sso_cookie_name(&state.config))
                .cloned()
        });
    if let Some(error) = query.error.as_deref() {
        return external_error_redirect(
            &state,
            provider,
            query.state.as_deref(),
            binding.as_ref(),
            center.is_some(),
            error,
        )
        .await;
    }
    let result = async {
        let code = query.code.as_deref().ok_or(IdentityError::InvalidInput)?;
        let state_value = query.state.as_deref().ok_or(IdentityError::InvalidInput)?;
        let binding = binding.as_ref().ok_or(IdentityError::Forbidden)?;
        let completion = service(&state)?
            .complete(
                provider,
                code,
                state_value,
                binding,
                center.as_deref(),
                context.id,
                &context.budget,
            )
            .await?;
        let clock = state.security_clock()?;
        Ok::<_, IdentityError>(match completion {
            ExternalCompletion::Login {
                cookie_secret,
                expires_at,
                oidc_flow_id,
            } => {
                let challenge = browser::authenticated_cookies(
                    &state.config,
                    clock.as_ref(),
                    cookie_secret,
                    expires_at,
                    crate::service::security::repository::now(clock.as_ref())?,
                )?;
                let location = oidc_flow_id
                    .map(|id| format!("/oidc/consent?flow_id={id}"))
                    .unwrap_or_else(|| "/account".to_owned());
                redirect_response(&location, challenge.headers)
            }
            ExternalCompletion::Bind {
                provider_key,
                oidc_flow_id,
            } => {
                let location = oidc_flow_id
                    .map(|id| format!("/oidc/consent?flow_id={id}"))
                    .unwrap_or_else(|| format!("/account/identities?bound={provider_key}"));
                redirect_response(&location, browser::protected_headers())
            }
            ExternalCompletion::Rejected {
                purpose,
                code,
                oidc_flow_id,
            } => {
                let location = match purpose {
                    ExternalPurpose::Login => format!("/login?error={code}"),
                    ExternalPurpose::Bind => oidc_flow_id
                        .map(|id| format!("/oidc/consent?flow_id={id}"))
                        .unwrap_or_else(|| {
                            format!("/account/identities?error={code}&bind={provider}")
                        }),
                };
                redirect_response(&location, browser::protected_headers())
            }
        })
    }
    .await;
    match result {
        Ok(response) => response,
        Err(reason) => {
            external_error_redirect(
                &state,
                provider,
                query.state.as_deref(),
                binding.as_ref(),
                center.is_some(),
                reason.code(),
            )
            .await
        }
    }
}

async fn external_error_redirect(
    state: &AppState,
    provider: ExternalProviderKey,
    callback_state: Option<&str>,
    binding: Option<&SecretDigest>,
    center: bool,
    code: &str,
) -> Response {
    let context = async {
        let clock = state.security_clock()?;
        let budget = SecurityBudget::new(clock)?;
        let binding = binding.ok_or(IdentityError::Forbidden)?;
        let callback_state = callback_state.ok_or(IdentityError::Forbidden)?;
        service(state)?
            .peek_flow_context(provider, callback_state, binding, &budget)
            .await
    }
    .await
    .ok()
    .flatten();
    let location = match context {
        Some(ExternalCallbackContext {
            purpose: ExternalPurpose::Bind,
            oidc_flow_id: Some(id),
        }) => format!("/oidc/consent?flow_id={id}"),
        Some(ExternalCallbackContext {
            purpose: ExternalPurpose::Bind,
            oidc_flow_id: None,
        }) => format!("/account/identities?error={code}&bind={provider}"),
        Some(ExternalCallbackContext {
            purpose: ExternalPurpose::Login,
            ..
        }) => format!("/login?error={code}"),
        None if center => format!("/account/identities?error={code}&bind={provider}"),
        None => format!("/login?error={code}"),
    };
    redirect_response(&location, browser::protected_headers())
}

fn redirect_response(location: &str, headers: HeaderMap) -> Response {
    (headers, Redirect::to(location)).into_response()
}

async fn identities(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
) -> Result<
    (
        HeaderMap,
        Json<Vec<crate::service::identity::external::ExternalIdentityView>>,
    ),
    AppError,
> {
    let result = async {
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let identities = service(&state)?
            .list_identities(&secret, &context.budget)
            .await?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(identities)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

async fn unbind(
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
        service(&state)?
            .unbind(&secret, id, context.id, &context.budget)
            .await?;
        Ok::<_, IdentityError>((
            browser::protected_headers(),
            Json(serde_json::json!({"unbound": true})),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

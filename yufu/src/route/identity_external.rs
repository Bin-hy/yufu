use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{
        error::IdentityError,
        external::{ExternalProviderKey, ExternalPurpose},
    },
    error::AppError,
    service::identity::{
        browser,
        external::{ExternalCompletion, ExternalIdentityService},
        repository::PgIdentityRepository,
    },
};
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

async fn bind_start(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    Path(provider): Path<String>,
    headers: HeaderMap,
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
    let result = async {
        let provider = provider
            .parse::<ExternalProviderKey>()
            .map_err(|_| IdentityError::InvalidInput)?;
        if query.error.is_some() {
            return Ok::<_, IdentityError>(redirect_response(
                "/login?error=external_authorization_denied",
                browser::protected_headers(),
            ));
        }
        let code = query.code.ok_or(IdentityError::InvalidInput)?;
        let state_value = query.state.ok_or(IdentityError::InvalidInput)?;
        let binding = browser::read_browser_binding(&state.config, &headers)?;
        let cookies = crate::service::security::browser::cookies(&headers)?;
        let center = cookies
            .get(browser::sso_cookie_name(&state.config))
            .map(String::as_str);
        let completion = service(&state)?
            .complete(
                provider,
                &code,
                &state_value,
                &binding,
                center,
                context.id,
                &context.budget,
            )
            .await?;
        let clock = state.security_clock()?;
        match completion {
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
                Ok(redirect_response(&location, challenge.headers))
            }
            ExternalCompletion::Bind { provider_key } => Ok(redirect_response(
                &format!("/account/identities?bound={provider_key}"),
                browser::protected_headers(),
            )),
            ExternalCompletion::Rejected { purpose, code } => {
                let location = match purpose {
                    ExternalPurpose::Login => format!("/login?error={code}"),
                    ExternalPurpose::Bind => {
                        format!("/account/identities?error={code}")
                    }
                };
                Ok(redirect_response(&location, browser::protected_headers()))
            }
        }
    }
    .await;
    match result {
        Ok(response) => response,
        Err(reason) => redirect_response(
            &format!("/login?error={}", reason.code()),
            browser::protected_headers(),
        ),
    }
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

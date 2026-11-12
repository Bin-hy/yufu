use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{
        authorization::AuthorizationParameters, error::IdentityError, session::AuthorizationOutcome,
    },
    error::AppError,
    service::identity::{
        authorization::{AuthorizationFlowProof, AuthorizationResult, AuthorizationService},
        browser,
        repository::PgIdentityRepository,
    },
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, RawQuery, State,
        rejection::{JsonRejection, PathRejection},
    },
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use http::{
    HeaderMap, HeaderValue,
    header::{COOKIE, SET_COOKIE},
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

fn service(state: &AppState) -> Result<AuthorizationService, IdentityError> {
    let clock = state.security_clock()?;
    AuthorizationService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
        state.config.clone(),
    )
}
pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/identity/flows/{id}", get(inspect))
        .route("/identity/flows/{id}/complete", post(complete))
}
fn combine(target: &mut HeaderMap, source: HeaderMap) {
    for (key, value) in &source {
        if key == SET_COOKIE {
            target.append(key.clone(), value.clone());
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}
/// The standard endpoint lives outside the API prefix and authenticates its own Cookie.
pub(crate) async fn authorize(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    RawQuery(query): RawQuery,
    request: HeaderMap,
) -> Result<Response, AppError> {
    let id = context.id;
    let result = async {
        let clock = state.security_clock()?;
        let budget = context.budget;
        let parameters =
            AuthorizationParameters::parse(query.as_deref().ok_or(IdentityError::InvalidInput)?)?;
        let parsed = crate::service::security::browser::cookies(&request)?;
        let center = parsed
            .get(browser::sso_cookie_name(&state.config))
            .map(String::as_str);
        let mut request = request.clone();
        let mut headers = browser::protected_headers();
        // Keep a separate browser binding across password reauthentication.
        let anonymous = browser::anonymous_challenge(&state.config, clock.as_ref(), &request)?;
        let mut cookie_pairs = parsed
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>();
        for value in anonymous.headers.get_all(SET_COOKIE) {
            cookie_pairs.push(
                value
                    .to_str()
                    .map_err(|_| IdentityError::DependencyUnavailable)?
                    .split(';')
                    .next()
                    .ok_or(IdentityError::DependencyUnavailable)?
                    .to_owned(),
            );
        }
        // Existing CSRF values may be replaced by the challenge; use the canonical jar.
        let mut jar = std::collections::BTreeMap::new();
        for pair in cookie_pairs {
            let (key, value) = pair
                .split_once('=')
                .ok_or(IdentityError::DependencyUnavailable)?;
            jar.insert(key.to_owned(), value.to_owned());
        }
        request.insert(
            COOKIE,
            HeaderValue::from_str(
                &jar.into_iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
            .map_err(|_| IdentityError::DependencyUnavailable)?,
        );
        let binding = browser::read_browser_binding(&state.config, &request)?;
        let result = service(&state)?
            .authorize(parameters, &binding, center, id, &budget)
            .await?;
        combine(&mut headers, anonymous.headers);
        let mut response = authorization_response(&state, result, headers, None)?;
        response.headers_mut().insert(
            "x-request-id",
            HeaderValue::from_str(&id.to_string())
                .map_err(|_| IdentityError::DependencyUnavailable)?,
        );
        Ok::<_, IdentityError>(response)
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, id))
}
fn authorization_response(
    state: &AppState,
    result: AuthorizationResult,
    mut headers: HeaderMap,
    completed: Option<Uuid>,
) -> Result<Response, IdentityError> {
    let location = match result.outcome {
        AuthorizationOutcome::RedirectToClient(response) => response.into_redirect(),
        AuthorizationOutcome::ProtocolError(error) => error.into_redirect(),
        AuthorizationOutcome::RequireLogin(flow)
        | AuthorizationOutcome::RequireConsent(flow, _) => {
            combine(
                &mut headers,
                browser::flow_cookies(
                    &state.config,
                    flow.id(),
                    result
                        .flow_proof
                        .ok_or(IdentityError::DependencyUnavailable)?,
                    flow.expires_at,
                    crate::service::security::repository::now(state.security_clock()?.as_ref())?,
                )?,
            );
            format!("/oidc/consent?flow_id={}", flow.id())
        }
    };
    if let Some(id) = completed {
        combine(&mut headers, browser::clear_flow_cookie(&state.config, id)?);
    }
    Ok((headers, Redirect::to(&location)).into_response())
}
async fn inspect(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let proof = browser::read_flow_proof(&state.config, &headers, id)?;
        let binding = browser::read_browser_binding(&state.config, &headers)?;
        let cookies = crate::service::security::browser::cookies(&headers)?;
        let secret = cookies
            .get(browser::sso_cookie_name(&state.config))
            .map(String::as_str);
        let summary = service(&state)?
            .inspect(
                AuthorizationFlowProof { id, proof: &proof },
                &binding,
                secret,
                &context.budget,
            )
            .await?;
        let challenge =
            browser::write_challenge(&state.config, state.security_clock()?.as_ref(), &headers)?;
        let mut value =
            serde_json::to_value(summary).map_err(|_| IdentityError::DependencyUnavailable)?;
        value["csrf_token"] = serde_json::json!(challenge.csrf_token.expose_secret());
        Ok::<_, IdentityError>((challenge.headers, Json(value)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteRequest {
    approved: bool,
}
async fn complete(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    payload: Result<Json<CompleteRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let proof = browser::read_flow_proof(&state.config, &headers, id)?;
        let result = service(&state)?
            .complete(
                AuthorizationFlowProof { id, proof: &proof },
                &evidence,
                payload.approved,
                context.id,
                &context.budget,
            )
            .await?;
        let location = match result.outcome {
            AuthorizationOutcome::RedirectToClient(response) => response.into_redirect(),
            AuthorizationOutcome::ProtocolError(error) => error.into_redirect(),
            _ => return Err(IdentityError::DependencyUnavailable),
        };
        Ok::<_, IdentityError>((
            browser::clear_flow_cookie(&state.config, id)?,
            Json(serde_json::json!({"redirect_url":location})),
        ))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

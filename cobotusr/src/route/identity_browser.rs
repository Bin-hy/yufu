use crate::service::identity::{
    account::AccountService,
    browser,
    repository::PgIdentityRepository,
    session::SessionService,
    sso::{LoginFlowProof, SsoService},
};
use crate::{
    AppState,
    domain::identity::{error::IdentityError, session::IdentityRevocationScope},
    error::AppError,
};
use auth::security::{budget::SecurityBudget, identity::SecretToken};
use axum::{
    Extension, Json, Router,
    extract::{Request, State, rejection::JsonRejection},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use http::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct IdentityRequest {
    pub id: Uuid,
    pub budget: SecurityBudget,
}

pub fn get_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/identity/csrf", get(csrf))
        .route("/identity/login", post(login))
        .route("/identity/logout", post(logout))
        .merge(super::account::get_routes())
        .merge(super::identity_console::get_routes())
        .merge(super::identity_actions::get_routes())
        .merge(super::enterprise_admin::get_routes())
        .merge(super::assignment::get_routes())
        .merge(super::developer::get_routes())
        .merge(super::identity_migration::get_routes())
        .merge(super::identity_authorization::get_routes())
        .layer(axum::extract::DefaultBodyLimit::max(16_384))
        .layer(middleware::from_fn_with_state(state, request_context))
}

pub(crate) async fn request_context(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let id = Uuid::new_v4();
    // These handlers return fixed deployment metadata and the public key.
    // Their availability is independent of the clock needed to issue credentials.
    if matches!(
        request.uri().path(),
        "/.well-known/openid-configuration" | "/.well-known/jwks.json"
    ) && matches!(*request.method(), http::Method::GET | http::Method::HEAD)
        && !request.headers().contains_key(http::header::ORIGIN)
    {
        return protect_response(next.run(request).await, id);
    }
    let budget = match state.security_clock().and_then(SecurityBudget::new) {
        Ok(budget) => budget,
        Err(error) => {
            return protect_response(AppError::identity(error.into(), id).into_response(), id);
        }
    };
    let cors = match super::identity_cors::protocol_headers(
        &state,
        request.uri().path(),
        request.method(),
        request.headers(),
        &budget,
    )
    .await
    {
        Ok(headers) => headers,
        Err(error) => return protect_response(AppError::identity(error, id).into_response(), id),
    };
    request
        .extensions_mut()
        .insert(IdentityRequest { id, budget });
    let mut response = if request.method() == http::Method::OPTIONS && cors.is_some() {
        http::StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    if let Some(headers) = cors {
        response.headers_mut().extend(headers);
    }
    protect_response(response, id)
}
fn protect_response(mut response: Response, id: Uuid) -> Response {
    for (name, value) in browser::protected_headers() {
        if let Some(name) = name {
            response.headers_mut().insert(name, value);
        }
    }
    if let Ok(value) = HeaderValue::from_str(&id.to_string()) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

pub(crate) fn sso(state: &AppState) -> Result<SsoService, IdentityError> {
    let clock = state.security_clock()?;
    SsoService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
        state.config.clone(),
    )
}
pub(crate) fn sessions(state: &AppState) -> Result<SessionService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(SessionService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    ))
}
pub(crate) fn accounts(state: &AppState) -> Result<AccountService, IdentityError> {
    let clock = state.security_clock()?;
    AccountService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
        state.config.clone(),
    )
}
pub(crate) fn login_flow(
    state: &AppState,
    headers: &HeaderMap,
    flow_id: Option<Uuid>,
) -> Result<Option<LoginFlowProof>, IdentityError> {
    flow_id
        .map(|id| {
            let parsed = crate::service::security::browser::cookies(headers)?;
            let name = browser::flow_cookie_name(&state.config, id);
            let proof = parsed.get(&name).ok_or(IdentityError::Forbidden)?;
            Ok(LoginFlowProof {
                id,
                proof: SecretToken::new(proof.clone())?,
            })
        })
        .transpose()
}

async fn csrf(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let result = (|| {
        let clock = state.security_clock()?;
        context.budget.remaining()?;
        browser::write_challenge(&state.config, clock.as_ref(), &headers)
    })();
    let challenge = result.map_err(|reason| AppError::identity(reason, context.id))?;
    Ok((
        challenge.headers,
        Json(serde_json::json!({"csrf_token":challenge.csrf_token.expose_secret()})),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    username: String,
    password: String,
    flow_id: Option<Uuid>,
}

async fn login(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    payload: Result<Json<LoginRequest>, JsonRejection>,
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
        let flow = login_flow(&state, &headers, payload.flow_id)?;
        let login = sso(&state)?
            .authenticate_password(
                &payload.username,
                &payload.password,
                &evidence,
                flow.as_ref(),
                context.id,
                &context.budget,
            )
            .await?;
        let challenge = browser::authenticated_cookies(
            &state.config,
            clock.as_ref(),
            login.cookie_secret,
            login.authentication.root.expires_at,
            crate::service::security::repository::now(clock.as_ref())?,
        )?;
        let response = super::account::session_response(
            &login.authentication,
            challenge.csrf_token.expose_secret(),
        );
        Ok::<_, IdentityError>((challenge.headers, Json(response)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

async fn logout(
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
                IdentityRevocationScope::CurrentCenter {
                    sso_session_id: authentication.root.id,
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

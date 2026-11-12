use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{account::SecretDigest, authorization::strict_form, error::IdentityError},
    error::AppError,
    service::identity::{
        browser,
        logout::{LogoutRequest, LogoutResult, LogoutService, LogoutStart},
        repository::PgIdentityRepository,
    },
};
use axum::{
    Extension,
    extract::{RawQuery, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use http::{
    HeaderMap,
    header::{CONTENT_TYPE, SET_COOKIE},
};
use std::sync::Arc;
use uuid::Uuid;

fn service(state: &AppState) -> Result<LogoutService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(LogoutService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
        state.id_token_signer().clone(),
        state.config.clone(),
    ))
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
fn finished(result: LogoutResult, headers: HeaderMap) -> Response {
    match result.redirect {
        Some(uri) => (headers, Redirect::to(&uri)).into_response(),
        None => (headers, Html(super::logout_page::completed())).into_response(),
    }
}
async fn start(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    request: LogoutRequest,
) -> Result<Response, IdentityError> {
    // The service validates this against the client's registered redirect URIs
    // before the confirmation page is rendered, so it is safe to use as the
    // cancel link target.
    let cancel_url = request.post_logout_redirect_uri.clone();
    let parsed = crate::service::security::browser::cookies(headers)?;
    let center = parsed
        .get(browser::sso_cookie_name(&state.config))
        .map(String::as_str);
    let binding = if center.is_some() {
        browser::read_browser_binding(&state.config, headers)?
    } else {
        SecretDigest::from_bytes(rand::random())
    };
    match service(state)?
        .start(center, binding, request, &context.budget)
        .await?
    {
        LogoutStart::Complete(result) => Ok(finished(
            result,
            browser::clear_center_cookies(&state.config)?,
        )),
        LogoutStart::Confirmation {
            flow,
            proof,
            application_name,
        } => {
            let clock = state.security_clock()?;
            let challenge = browser::write_challenge(&state.config, clock.as_ref(), headers)?;
            let mut response_headers = challenge.headers;
            combine(
                &mut response_headers,
                browser::action_cookies(
                    &state.config,
                    flow.id,
                    proof,
                    flow.expires_at,
                    crate::service::security::repository::now(clock.as_ref())?,
                )?,
            );
            // Chromium sends `Origin: null` on a same-origin form POST when the
            // document is served with `Referrer-Policy: no-referrer`, which the
            // confirm endpoint correctly rejects. The page only submits back to
            // itself, so `same-origin` keeps the referrer from leaking anywhere
            // else while preserving the real Origin.
            response_headers.insert(
                http::header::REFERRER_POLICY,
                http::HeaderValue::from_static("same-origin"),
            );
            let flow_id = flow.id.to_string();
            let html = super::logout_page::confirmation(
                application_name.as_deref(),
                "/oidc/end-session",
                &[
                    ("flow_id", flow_id.as_str()),
                    ("csrf_token", challenge.csrf_token.expose_secret()),
                ],
                cancel_url.as_deref(),
            );
            Ok((response_headers, Html(html)).into_response())
        }
    }
}
pub(crate) async fn get(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let result = async {
        start(
            &state,
            &context,
            &headers,
            LogoutRequest::parse(query.as_deref().unwrap_or(""))?,
        )
        .await
    }
    .await;
    result.map_err(|e| AppError::identity(e, context.id))
}
pub(crate) async fn post(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, AppError> {
    let result = async {
        let mut types = headers.get_all(CONTENT_TYPE).iter();
        let content_type = types
            .next()
            .and_then(|v| v.to_str().ok())
            .ok_or(IdentityError::InvalidInput)?;
        if types.next().is_some()
            || content_type.split(';').next() != Some("application/x-www-form-urlencoded")
        {
            return Err(IdentityError::InvalidInput);
        }
        let raw = std::str::from_utf8(&body).map_err(|_| IdentityError::InvalidInput)?;
        let fields = if raw.is_empty() {
            Default::default()
        } else {
            crate::domain::identity::authorization::strict_form_with_value_limit(raw, 16_384, 8192)?
        };
        if !fields.contains_key("flow_id") {
            // Standard RP POST is an initiation, just like GET. It cannot revoke.
            return start(&state, &context, &headers, LogoutRequest::parse(raw)?).await;
        }
        let fields = strict_form(raw, 16_384)?;
        if fields
            .keys()
            .any(|k| !matches!(k.as_str(), "flow_id" | "csrf_token"))
        {
            return Err(IdentityError::InvalidInput);
        }
        let flow_id = fields
            .get("flow_id")
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            fields.get("csrf_token").map(String::as_str),
            &context.budget,
        )?;
        let proof = browser::read_action_proof(&state.config, &headers, flow_id)?;
        let result = service(&state)?
            .confirm(
                &evidence,
                flow_id,
                proof.expose_secret(),
                context.id,
                &context.budget,
            )
            .await?;
        let mut response_headers = browser::clear_center_cookies(&state.config)?;
        combine(
            &mut response_headers,
            browser::clear_action_cookie(&state.config, flow_id)?,
        );
        Ok::<_, IdentityError>(finished(result, response_headers))
    }
    .await;
    result.map_err(|e| AppError::identity(e, context.id))
}

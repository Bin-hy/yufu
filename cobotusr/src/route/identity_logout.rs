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
    match result.redirect { Some(uri) => (headers,Redirect::to(&uri)).into_response(),
        None => (headers,Html("<!doctype html><html lang=\"zh\"><meta charset=\"utf-8\"><title>退出完成</title><body><p>当前浏览器的统一登录会话已结束。</p></body></html>")).into_response() }
}
async fn start(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    request: LogoutRequest,
) -> Result<Response, IdentityError> {
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
            let name = application_name
                .as_deref()
                .map(super::oidc::html_attribute)
                .unwrap_or_else(|| "当前应用".into());
            let html = format!(
                "<!doctype html><html lang=\"zh\"><meta charset=\"utf-8\"><title>确认退出</title><body><p>{name} 请求退出当前浏览器的统一登录会话及其关联应用。</p><form method=\"post\" action=\"/oidc/end-session\"><input type=\"hidden\" name=\"flow_id\" value=\"{}\"><input type=\"hidden\" name=\"csrf_token\" value=\"{}\"><button type=\"submit\">确认退出</button></form></body></html>",
                flow.id,
                super::oidc::html_attribute(challenge.csrf_token.expose_secret())
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

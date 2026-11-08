//! Browser evidence and local management authentication share the caller's budget.
use super::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository};
use crate::AppState;
use auth::security::{
    budget::SecurityBudget,
    credential::{
        AuthenticatedContext, CredentialExpectation, CredentialProfile, LocalStrictAccessState,
    },
    csrf::CsrfProtector,
    error::SecurityError,
    identity::{PrincipalKind, SecretToken, TokenUse},
};
use http::{
    HeaderMap,
    header::{COOKIE, SET_COOKIE},
};
use std::{collections::BTreeMap, sync::Arc};

pub const CSRF_COOKIE: &str = "__Host-cobotusr-csrf";

pub fn lifecycle(state: &AppState) -> Result<Arc<CredentialLifecycle>, SecurityError> {
    let clock = state.security_clock()?;
    Ok(Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    )))
}

pub fn cookies(headers: &HeaderMap) -> Result<BTreeMap<String, String>, SecurityError> {
    let mut values = BTreeMap::new();
    let mut size = 0usize;
    for header in headers.get_all(COOKIE) {
        let header = header.to_str().map_err(|_| SecurityError::Forbidden)?;
        size = size.saturating_add(header.len());
        if size > 16_384 {
            return Err(SecurityError::Forbidden);
        }
        for pair in header.split(';') {
            let (name, value) = pair
                .trim()
                .split_once('=')
                .ok_or(SecurityError::Forbidden)?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_graphic() && !b"\";,\\".contains(&b))
                || values.insert(name.to_owned(), value.to_owned()).is_some()
            {
                return Err(SecurityError::Forbidden);
            }
        }
    }
    Ok(values)
}

pub fn single_header<'a>(
    headers: &'a HeaderMap,
    name: &str,
) -> Result<Option<&'a str>, SecurityError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|v| v.to_str().map_err(|_| SecurityError::Forbidden))
        .transpose()?;
    if values.next().is_some() {
        return Err(SecurityError::Forbidden);
    }
    Ok(value)
}

fn origin(value: &str) -> Result<String, SecurityError> {
    let url = url::Url::parse(value).map_err(|_| SecurityError::Forbidden)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(SecurityError::Forbidden);
    }
    Ok(url.origin().ascii_serialization())
}

pub fn validate_csrf(
    state: &AppState,
    headers: &HeaderMap,
    form_proof: Option<&str>,
    budget: &SecurityBudget,
) -> Result<Option<SecretToken>, SecurityError> {
    budget.remaining()?;
    let requested = single_header(headers, "origin")?.ok_or(SecurityError::Forbidden)?;
    let canonical = origin(requested)?;
    if canonical != requested
        || !(canonical == origin(&state.issuer())?
            || state
                .config
                .auth
                .origins
                .iter()
                .any(|allowed| allowed == &canonical))
    {
        return Err(SecurityError::Forbidden);
    }
    let cookies = cookies(headers)?;
    let cookie = cookies.get(CSRF_COOKIE).ok_or(SecurityError::Forbidden)?;
    let header = single_header(headers, "x-csrf-token")?;
    if header.is_some() && form_proof.is_some() && header != form_proof {
        return Err(SecurityError::Forbidden);
    }
    let proof = header.or(form_proof).ok_or(SecurityError::Forbidden)?;
    if proof != cookie {
        return Err(SecurityError::Forbidden);
    }
    let refresh = cookies.get("refresh_token").map(String::as_str);
    let proof = SecretToken::new(proof.to_owned()).map_err(|_| SecurityError::Forbidden)?;
    CsrfProtector::new(state.config.auth.secret.clone())?.verify(
        &proof,
        &state.issuer(),
        refresh,
        state.security_clock()?.as_ref(),
    )?;
    budget.remaining()?;
    refresh
        .map(|value| SecretToken::new(value.to_owned()))
        .transpose()
}

pub fn issue_csrf(
    state: &AppState,
    refresh: Option<&str>,
    headers: &mut HeaderMap,
) -> Result<SecretToken, SecurityError> {
    let proof = CsrfProtector::new(state.config.auth.secret.clone())?.issue(
        &state.issuer(),
        refresh,
        state.security_clock()?.as_ref(),
    )?;
    headers.append(
        SET_COOKIE,
        format!(
            "{CSRF_COOKIE}={}; Secure; Path=/; SameSite=Lax; Max-Age=900",
            proof.expose_secret()
        )
        .parse()
        .map_err(|_| SecurityError::DependencyUnavailable)?,
    );
    headers.insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store"),
    );
    Ok(proof)
}

pub fn clear_session_cookies() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.append(
        SET_COOKIE,
        http::HeaderValue::from_static(
            "refresh_token=; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age=0",
        ),
    );
    headers.append(
        SET_COOKIE,
        http::HeaderValue::from_static(
            "__Host-cobotusr-csrf=; Secure; Path=/; SameSite=Lax; Max-Age=0",
        ),
    );
    headers.insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store"),
    );
    headers
}

pub async fn authenticate_manager(
    state: &AppState,
    headers: &HeaderMap,
    budget: &SecurityBudget,
) -> Result<AuthenticatedContext, SecurityError> {
    if headers.contains_key(COOKIE) {
        validate_csrf(state, headers, None, budget)?;
    }
    LocalStrictAccessState::new(
        state.id_token_signer().clone(),
        CredentialExpectation::new(
            state.issuer(),
            "cobotusr",
            "cobotusr",
            vec![CredentialProfile::new(
                PrincipalKind::User,
                TokenUse::UserSession,
            )?],
        )?,
        state.security_clock()?,
        lifecycle(state)?,
    )
    .authenticate_with_budget(headers, budget)
    .await
}

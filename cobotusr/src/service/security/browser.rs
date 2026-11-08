//! Browser evidence and local management authentication share the caller's budget.
use super::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository};
use crate::AppState;
use crate::error::{AppError, BrowserRejection};
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
const DEV_CSRF_COOKIE: &str = "cobotusr-dev-csrf";

fn cookie_policy(state: &AppState) -> Result<(&'static str, &'static str), SecurityError> {
    state
        .config
        .validate_browser_cookie_policy()
        .map_err(|_| SecurityError::DependencyUnavailable)?;
    if state.config.auth.allow_insecure_loopback_cookies {
        Ok((DEV_CSRF_COOKIE, ""))
    } else {
        Ok((CSRF_COOKIE, " Secure;"))
    }
}

pub fn refresh_cookie(state: &AppState, token: &str) -> Result<http::HeaderValue, SecurityError> {
    let (_, secure) = cookie_policy(state)?;
    format!(
        "refresh_token={token}; HttpOnly;{secure} Path=/; SameSite=Lax; Max-Age={}",
        state.config.auth.refresh_exp_seconds
    )
    .parse()
    .map_err(|_| SecurityError::DependencyUnavailable)
}

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

fn reject(reason: BrowserRejection) -> AppError {
    tracing::warn!(reason = reason.code(), "browser authentication rejected");
    AppError::BrowserRejected(reason)
}

fn origin_allowed(config: &crate::config::Config, issuer: &str, requested: &str) -> bool {
    let Ok(requested_url) = url::Url::parse(requested) else {
        return false;
    };
    let dev = config.auth.allow_insecure_loopback_cookies;
    if dev
        && !requested_url
            .host_str()
            .is_some_and(crate::config::is_loopback_host)
    {
        return false;
    }
    if requested == issuer
        || config
            .auth
            .origins
            .iter()
            .any(|allowed| allowed == requested)
    {
        return true;
    }
    let Ok(issuer_url) = url::Url::parse(issuer) else {
        return false;
    };
    // Only explicit loopback development permits host aliases. A different port or
    // scheme still needs an explicit origin; '*' never authorizes cookie writes.
    dev && config.http.listen.ip().is_loopback()
        && issuer_url
            .host_str()
            .is_some_and(crate::config::is_loopback_host)
        && requested_url.scheme() == issuer_url.scheme()
        && requested_url.port_or_known_default() == issuer_url.port_or_known_default()
}

pub fn validate_csrf(
    state: &AppState,
    headers: &HeaderMap,
    form_proof: Option<&str>,
    budget: &SecurityBudget,
) -> Result<Option<SecretToken>, AppError> {
    budget.remaining()?;
    let requested = single_header(headers, "origin")
        .map_err(|_| reject(BrowserRejection::OriginInvalid))?
        .ok_or_else(|| reject(BrowserRejection::OriginMissing))?;
    let canonical = origin(requested).map_err(|_| reject(BrowserRejection::OriginInvalid))?;
    if canonical != requested {
        return Err(reject(BrowserRejection::OriginInvalid));
    }
    let issuer = origin(&state.issuer())?;
    if !origin_allowed(&state.config, &issuer, &canonical) {
        tracing::warn!(request_origin = %canonical, issuer_origin = %issuer, "browser origin is not authorized");
        return Err(reject(BrowserRejection::OriginNotAllowed));
    }
    let cookies = cookies(headers).map_err(|_| reject(BrowserRejection::CookieInvalid))?;
    let (cookie_name, _) = cookie_policy(state)?;
    let cookie = cookies.get(cookie_name).ok_or_else(|| {
        reject(
            if !state.config.auth.allow_insecure_loopback_cookies
                && canonical.starts_with("http://")
            {
                BrowserRejection::CookieRequiresHttps
            } else {
                BrowserRejection::CookieMissing
            },
        )
    })?;
    let header = single_header(headers, "x-csrf-token")
        .map_err(|_| reject(BrowserRejection::ProofInvalid))?;
    if header.is_some() && form_proof.is_some() && header != form_proof {
        return Err(reject(BrowserRejection::ProofMismatch));
    }
    let proof = header
        .or(form_proof)
        .ok_or_else(|| reject(BrowserRejection::ProofMissing))?;
    if proof != cookie {
        return Err(reject(BrowserRejection::ProofMismatch));
    }
    let refresh = cookies.get("refresh_token").map(String::as_str);
    let proof =
        SecretToken::new(proof.to_owned()).map_err(|_| reject(BrowserRejection::ProofInvalid))?;
    CsrfProtector::new(state.config.auth.secret.clone())?
        .verify(
            &proof,
            &state.issuer(),
            refresh,
            state.security_clock()?.as_ref(),
        )
        .map_err(|error| {
            if error == SecurityError::Forbidden {
                reject(BrowserRejection::ProofInvalid)
            } else {
                error.into()
            }
        })?;
    budget.remaining()?;
    refresh
        .map(|value| SecretToken::new(value.to_owned()))
        .transpose()
        .map_err(Into::into)
}

pub fn issue_csrf(
    state: &AppState,
    refresh: Option<&str>,
    headers: &mut HeaderMap,
) -> Result<SecretToken, SecurityError> {
    let (cookie_name, secure) = cookie_policy(state)?;
    let proof = CsrfProtector::new(state.config.auth.secret.clone())?.issue(
        &state.issuer(),
        refresh,
        state.security_clock()?.as_ref(),
    )?;
    headers.append(
        SET_COOKIE,
        format!(
            "{cookie_name}={};{secure} Path=/; SameSite=Lax; Max-Age=900",
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

pub fn clear_session_cookies(state: &AppState) -> Result<HeaderMap, SecurityError> {
    let (cookie_name, secure) = cookie_policy(state)?;
    let mut headers = HeaderMap::new();
    headers.append(
        SET_COOKIE,
        format!("refresh_token=; HttpOnly;{secure} Path=/; SameSite=Lax; Max-Age=0")
            .parse()
            .map_err(|_| SecurityError::DependencyUnavailable)?,
    );
    headers.append(
        SET_COOKIE,
        format!("{cookie_name}=;{secure} Path=/; SameSite=Lax; Max-Age=0")
            .parse()
            .map_err(|_| SecurityError::DependencyUnavailable)?,
    );
    headers.insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store"),
    );
    Ok(headers)
}

pub async fn authenticate_manager(
    state: &AppState,
    headers: &HeaderMap,
    budget: &SecurityBudget,
) -> Result<AuthenticatedContext, AppError> {
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
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin_aliases_require_explicit_loopback_mode_and_matching_scheme_port() {
        let mut config = crate::config::Config::default();
        config.http.listen = "127.0.0.1:4000".parse().unwrap();
        config.auth.origins = vec!["*".into()];
        let issuer = "http://127.0.0.1:4000";
        assert!(origin_allowed(&config, issuer, issuer));
        assert!(!origin_allowed(&config, issuer, "http://localhost:4000"));
        config.auth.allow_insecure_loopback_cookies = true;
        for alias in [
            "http://localhost:4000",
            "http://127.0.0.1:4000",
            "http://[::1]:4000",
        ] {
            assert!(origin_allowed(&config, issuer, alias));
        }
        for denied in [
            "http://localhost:4001",
            "https://localhost:4000",
            "http://remote.test:4000",
            "http://localhost.remote.test:4000",
            "null",
        ] {
            assert!(!origin_allowed(&config, issuer, denied), "{denied}");
        }
        config.auth.origins.push("http://localhost:5173".into());
        assert!(origin_allowed(&config, issuer, "http://localhost:5173"));
        config.auth.origins.push("http://remote.test:4000".into());
        assert!(!origin_allowed(&config, issuer, "http://remote.test:4000"));
        config.auth.allow_insecure_loopback_cookies = false;
        assert!(origin_allowed(&config, issuer, "http://remote.test:4000"));
    }
}

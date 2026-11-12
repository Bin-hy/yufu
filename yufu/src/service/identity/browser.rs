//! Cookie and CSRF adapters for same-origin center APIs.
use crate::service::security::browser::{cookies, single_header};
use crate::{
    config::Config,
    domain::identity::{
        account::{IssuedSecret, SecretDigest},
        error::IdentityError,
    },
};
use auth::security::{
    budget::{Clock, SecurityBudget},
    csrf::{CsrfBinding, CsrfProtector},
    identity::SecretToken,
};
use http::{HeaderMap, HeaderValue, header::SET_COOKIE};

pub const SSO_COOKIE: &str = "__Host-yufu-sso";
pub const DEV_SSO_COOKIE: &str = "yufu-dev-sso";

#[derive(Clone, Copy)]
enum Context {
    Center,
    Anonymous,
}

/// Only this adapter can construct evidence after signature, double-submit
/// and exact Origin checks. It carries digests rather than raw cookies.
pub struct BrowserWriteEvidence {
    pub(crate) current_center_digest: Option<SecretDigest>,
    pub(crate) browser_binding_digest: SecretDigest,
}

pub struct BrowserChallenge {
    pub headers: HeaderMap,
    pub csrf_token: SecretToken,
}

pub fn sso_cookie_name(config: &Config) -> &'static str {
    if config.auth.allow_insecure_loopback_cookies {
        DEV_SSO_COOKIE
    } else {
        SSO_COOKIE
    }
}
pub fn flow_cookie_name(config: &Config, id: uuid::Uuid) -> String {
    format!(
        "{}-{id}",
        if config.auth.allow_insecure_loopback_cookies {
            "yufu-dev-flow"
        } else {
            "__Host-yufu-flow"
        }
    )
}
pub fn action_cookie_name(config: &Config, id: uuid::Uuid) -> String {
    format!(
        "{}-{id}",
        if config.auth.allow_insecure_loopback_cookies {
            "yufu-dev-action"
        } else {
            "__Host-yufu-action"
        }
    )
}
pub fn read_action_proof(
    config: &Config,
    request: &HeaderMap,
    id: uuid::Uuid,
) -> Result<SecretToken, IdentityError> {
    let parsed = cookies(request)?;
    let proof = parsed
        .get(&action_cookie_name(config, id))
        .ok_or(IdentityError::Forbidden)?;
    super::sso::validate_opaque(proof)?;
    SecretToken::new(proof.clone()).map_err(Into::into)
}
pub fn read_browser_binding(
    config: &Config,
    request: &HeaderMap,
) -> Result<SecretDigest, IdentityError> {
    let parsed = cookies(request)?;
    let secret = parsed
        .get(browser_cookie_name(config))
        .or_else(|| parsed.get(sso_cookie_name(config)))
        .ok_or(IdentityError::Forbidden)?;
    super::sso::validate_opaque(secret)?;
    Ok(SecretDigest::of(secret))
}
pub fn read_flow_proof(
    config: &Config,
    headers: &HeaderMap,
    id: uuid::Uuid,
) -> Result<SecretToken, IdentityError> {
    let parsed = cookies(headers)?;
    let value = parsed
        .get(&flow_cookie_name(config, id))
        .ok_or(IdentityError::Forbidden)?;
    super::sso::validate_opaque(value)?;
    SecretToken::new(value.clone()).map_err(Into::into)
}
pub fn flow_cookies(
    config: &Config,
    id: uuid::Uuid,
    proof: IssuedSecret,
    expires_at: u64,
    at: u64,
) -> Result<HeaderMap, IdentityError> {
    let max_age = expires_at.saturating_sub(at) / 1000;
    if max_age == 0 {
        return Err(IdentityError::Expired);
    }
    let mut headers = protected_headers();
    headers.append(
        SET_COOKIE,
        set_cookie(
            config,
            &flow_cookie_name(config, id),
            &proof.into_response(),
            true,
            max_age,
        )?,
    );
    Ok(headers)
}
pub fn clear_flow_cookie(config: &Config, id: uuid::Uuid) -> Result<HeaderMap, IdentityError> {
    let mut headers = protected_headers();
    headers.append(
        SET_COOKIE,
        set_cookie(config, &flow_cookie_name(config, id), "", true, 0)?,
    );
    Ok(headers)
}
pub fn action_cookies(
    config: &Config,
    action_id: uuid::Uuid,
    proof: IssuedSecret,
    expires_at: u64,
    now_millis: u64,
) -> Result<HeaderMap, IdentityError> {
    let max_age = expires_at.saturating_sub(now_millis) / 1_000;
    if max_age == 0 {
        return Err(IdentityError::Expired);
    }
    let mut headers = protected_headers();
    headers.append(
        SET_COOKIE,
        set_cookie(
            config,
            &action_cookie_name(config, action_id),
            &proof.into_response(),
            true,
            max_age,
        )?,
    );
    Ok(headers)
}
pub fn clear_action_cookie(
    config: &Config,
    action_id: uuid::Uuid,
) -> Result<HeaderMap, IdentityError> {
    let mut headers = protected_headers();
    headers.append(
        SET_COOKIE,
        set_cookie(config, &action_cookie_name(config, action_id), "", true, 0)?,
    );
    Ok(headers)
}
fn browser_cookie_name(config: &Config) -> &'static str {
    if config.auth.allow_insecure_loopback_cookies {
        "yufu-dev-browser"
    } else {
        "__Host-yufu-browser"
    }
}
fn csrf_cookie_name(config: &Config, context: Context) -> &'static str {
    match (config.auth.allow_insecure_loopback_cookies, context) {
        (false, Context::Center) => "__Host-yufu-center-csrf",
        (false, Context::Anonymous) => "__Host-yufu-anonymous-csrf",
        (true, Context::Center) => "yufu-dev-center-csrf",
        (true, Context::Anonymous) => "yufu-dev-anonymous-csrf",
    }
}
fn set_cookie(
    config: &Config,
    name: &str,
    value: &str,
    http_only: bool,
    max_age: u64,
) -> Result<HeaderValue, IdentityError> {
    format!(
        "{name}={value}; Path=/; SameSite=Lax; Max-Age={max_age}{}{}",
        if http_only { "; HttpOnly" } else { "" },
        if config.auth.allow_insecure_loopback_cookies {
            ""
        } else {
            "; Secure"
        }
    )
    .parse()
    .map_err(|_| IdentityError::DependencyUnavailable)
}
pub fn protected_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("frame-ancestors 'none'; base-uri 'self'"),
    );
    headers
}

pub fn read_sso_cookie(config: &Config, headers: &HeaderMap) -> Result<String, IdentityError> {
    let parsed = cookies(headers)?;
    let secret = parsed
        .get(sso_cookie_name(config))
        .ok_or(IdentityError::Unauthenticated)?;
    super::sso::validate_opaque(secret)?;
    Ok(secret.clone())
}

fn csrf_challenge(
    config: &Config,
    clock: &dyn Clock,
    request: &HeaderMap,
    digest: &SecretDigest,
    context: Context,
) -> Result<BrowserChallenge, IdentityError> {
    let existing = cookies(request)?;
    let name = csrf_cookie_name(config, context);
    let protector = CsrfProtector::new(config.auth.secret.clone())?;
    let binding = match context {
        Context::Center => CsrfBinding::CenterRootDigest(digest.as_bytes()),
        Context::Anonymous => CsrfBinding::AnonymousBrowserDigest(digest.as_bytes()),
    };
    let mut headers = protected_headers();
    if let Some(existing) = existing.get(name) {
        let token = SecretToken::new(existing.clone())?;
        if protector
            .verify_bound(&token, &config.issuer(), binding, clock)
            .is_ok()
        {
            return Ok(BrowserChallenge {
                headers,
                csrf_token: token,
            });
        }
    }
    let csrf_token = protector.issue_bound(&config.issuer(), binding, clock)?;
    headers.append(
        SET_COOKIE,
        set_cookie(config, name, csrf_token.expose_secret(), false, 900)?,
    );
    Ok(BrowserChallenge {
        headers,
        csrf_token,
    })
}

pub fn anonymous_challenge(
    config: &Config,
    clock: &dyn Clock,
    request: &HeaderMap,
) -> Result<BrowserChallenge, IdentityError> {
    let existing = cookies(request)?;
    let name = browser_cookie_name(config);
    if let Some(secret) = existing.get(name) {
        super::sso::validate_opaque(secret)?;
        return csrf_challenge(
            config,
            clock,
            request,
            &SecretDigest::of(secret),
            Context::Anonymous,
        );
    }
    let secret = IssuedSecret::from_entropy(rand::random());
    let mut challenge =
        csrf_challenge(config, clock, request, &secret.digest(), Context::Anonymous)?;
    challenge.headers.append(
        SET_COOKIE,
        set_cookie(
            config,
            name,
            &secret.into_response(),
            true,
            config.identity.sso_session_ttl_seconds,
        )?,
    );
    Ok(challenge)
}

pub fn center_challenge(
    config: &Config,
    clock: &dyn Clock,
    request: &HeaderMap,
    authenticated_root: &SecretDigest,
) -> Result<BrowserChallenge, IdentityError> {
    csrf_challenge(config, clock, request, authenticated_root, Context::Center)
}
pub fn write_challenge(
    config: &Config,
    clock: &dyn Clock,
    request: &HeaderMap,
) -> Result<BrowserChallenge, IdentityError> {
    let parsed = cookies(request)?;
    match parsed.get(sso_cookie_name(config)) {
        Some(secret) => {
            super::sso::validate_opaque(secret)?;
            center_challenge(config, clock, request, &SecretDigest::of(secret))
        }
        None => anonymous_challenge(config, clock, request),
    }
}

pub fn verify_write(
    config: &Config,
    clock: &dyn Clock,
    request: &HeaderMap,
    form_proof: Option<&str>,
    budget: &SecurityBudget,
) -> Result<BrowserWriteEvidence, IdentityError> {
    budget.remaining()?;
    let requested = single_header(request, "origin")?.ok_or(IdentityError::Forbidden)?;
    let issuer =
        url::Url::parse(&config.issuer()).map_err(|_| IdentityError::DependencyUnavailable)?;
    let issuer_origin = issuer.origin().ascii_serialization();
    if !crate::service::security::browser::origin_allowed(config, &issuer_origin, requested) {
        if !config.auth.allow_insecure_loopback_cookies
            && crate::service::security::browser::origin_requires_https(&issuer_origin, requested)
        {
            return Err(IdentityError::CookieRequiresHttps);
        }
        return Err(IdentityError::Forbidden);
    }
    let missing_cookie = || {
        if !config.auth.allow_insecure_loopback_cookies && requested.starts_with("http://") {
            IdentityError::CookieRequiresHttps
        } else {
            IdentityError::Forbidden
        }
    };
    let parsed = cookies(request)?;
    let current_center_digest = parsed
        .get(sso_cookie_name(config))
        .map(|secret| {
            super::sso::validate_opaque(secret)?;
            Ok::<_, IdentityError>(SecretDigest::of(secret))
        })
        .transpose()?;
    let anonymous = parsed
        .get(browser_cookie_name(config))
        .map(|secret| {
            super::sso::validate_opaque(secret)?;
            Ok::<_, IdentityError>(SecretDigest::of(secret))
        })
        .transpose()?;
    let (digest, context) = if let Some(center) = &current_center_digest {
        (center, Context::Center)
    } else {
        (
            anonymous.as_ref().ok_or_else(missing_cookie)?,
            Context::Anonymous,
        )
    };
    let cookie = parsed
        .get(csrf_cookie_name(config, context))
        .ok_or_else(missing_cookie)?;
    let header = single_header(request, "x-csrf-token")?;
    if header.is_some() && form_proof.is_some() && header != form_proof {
        return Err(IdentityError::Forbidden);
    }
    let proof = header.or(form_proof).ok_or(IdentityError::Forbidden)?;
    if proof != cookie {
        return Err(IdentityError::Forbidden);
    }
    let proof = SecretToken::new(proof.to_owned())?;
    let binding = match context {
        Context::Center => CsrfBinding::CenterRootDigest(digest.as_bytes()),
        Context::Anonymous => CsrfBinding::AnonymousBrowserDigest(digest.as_bytes()),
    };
    CsrfProtector::new(config.auth.secret.clone())?.verify_bound(
        &proof,
        &config.issuer(),
        binding,
        clock,
    )?;
    budget.remaining()?;
    let browser_binding_digest = anonymous.as_ref().unwrap_or(digest).clone();
    Ok(BrowserWriteEvidence {
        browser_binding_digest,
        current_center_digest,
    })
}

/// Called only with the committed result of password authentication.
pub fn authenticated_cookies(
    config: &Config,
    clock: &dyn Clock,
    root_secret: IssuedSecret,
    expires_at: u64,
    now_millis: u64,
) -> Result<BrowserChallenge, IdentityError> {
    let max_age = expires_at.saturating_sub(now_millis) / 1000;
    if max_age == 0 {
        return Err(IdentityError::Unauthenticated);
    }
    let mut challenge = center_challenge(config, clock, &HeaderMap::new(), &root_secret.digest())?;
    challenge.headers.append(
        SET_COOKIE,
        set_cookie(
            config,
            sso_cookie_name(config),
            &root_secret.into_response(),
            true,
            max_age,
        )?,
    );
    Ok(challenge)
}

pub fn clear_center_cookies(config: &Config) -> Result<HeaderMap, IdentityError> {
    let mut headers = protected_headers();
    for (name, http_only) in [
        (sso_cookie_name(config), true),
        (csrf_cookie_name(config, Context::Center), false),
    ] {
        headers.append(SET_COOKIE, set_cookie(config, name, "", http_only, 0)?);
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::security::{budget::ClockReading, error::SecurityError};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::Duration,
    };

    struct TestClock(AtomicU64);
    impl Clock for TestClock {
        fn now(&self) -> Result<ClockReading, SecurityError> {
            let millis = self.0.load(Ordering::SeqCst);
            Ok(ClockReading {
                monotonic: Duration::from_millis(millis),
                unix_millis: 1_800_000_000_000 + millis,
                synchronized: true,
            })
        }
    }
    fn config() -> Config {
        let mut config = Config::default();
        config.oidc.issuer = "https://identity.example/issuer".into();
        config.auth.secret = "adapter-test-key".into();
        config
    }
    fn request_from(challenge: &BrowserChallenge) -> HeaderMap {
        let cookies: Vec<_> = challenge
            .headers
            .get_all(SET_COOKIE)
            .iter()
            .map(|header| header.to_str().unwrap().split(';').next().unwrap())
            .collect();
        HeaderMap::from_iter([
            (http::header::COOKIE, cookies.join("; ").parse().unwrap()),
            (
                http::header::ORIGIN,
                HeaderValue::from_static("https://identity.example"),
            ),
            (
                http::header::HeaderName::from_static("x-csrf-token"),
                challenge.csrf_token.expose_secret().parse().unwrap(),
            ),
        ])
    }

    #[test]
    fn identity_browser_cookie_policy_and_read_challenge_reuse() {
        let config = config();
        let clock = TestClock(AtomicU64::new(0));
        let anonymous = anonymous_challenge(&config, &clock, &HeaderMap::new()).unwrap();
        for cookie in anonymous.headers.get_all(SET_COOKIE) {
            let cookie = cookie.to_str().unwrap();
            assert!(cookie.starts_with("__Host-"));
            assert!(cookie.contains("; Secure"));
            assert!(cookie.contains("; Path=/;"));
            assert!(!cookie.contains("Domain="));
            if cookie.starts_with("__Host-yufu-browser=") {
                assert!(cookie.contains("; HttpOnly"));
            }
        }
        let request = request_from(&anonymous);
        for _ in 0..4 {
            let again = anonymous_challenge(&config, &clock, &request).unwrap();
            assert!(!again.headers.contains_key(SET_COOKIE));
            assert_eq!(
                again.csrf_token.expose_secret(),
                anonymous.csrf_token.expose_secret()
            );
        }
        let authenticated = authenticated_cookies(
            &config,
            &clock,
            IssuedSecret::from_entropy([7; 32]),
            1_800_000_300_000,
            1_800_000_000_000,
        )
        .unwrap();
        let request = request_from(&authenticated);
        let secret = read_sso_cookie(&config, &request).unwrap();
        let center =
            center_challenge(&config, &clock, &request, &SecretDigest::of(&secret)).unwrap();
        assert!(!center.headers.contains_key(SET_COOKIE));
        let root = authenticated
            .headers
            .get_all(SET_COOKIE)
            .iter()
            .map(|c| c.to_str().unwrap())
            .find(|c| c.starts_with(SSO_COOKIE))
            .unwrap();
        assert!(root.contains("; HttpOnly; Secure"));
        assert!(root.contains("Max-Age=300"));
        for cookie in clear_center_cookies(&config).unwrap().get_all(SET_COOKIE) {
            assert!(cookie.to_str().unwrap().contains("Max-Age=0"));
        }
    }

    #[test]
    fn identity_browser_write_rejects_origin_duplicates_context_swaps_and_expiry() {
        let config = config();
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let anonymous = anonymous_challenge(&config, clock.as_ref(), &HeaderMap::new()).unwrap();
        let valid = request_from(&anonymous);
        assert!(
            verify_write(
                &config,
                clock.as_ref(),
                &valid,
                None,
                &SecurityBudget::new(clock.clone()).unwrap()
            )
            .is_ok()
        );
        for origin in [
            "null",
            "http://identity.example",
            "https://identity.example/",
            "https://identity.example:444",
            "https://identity.example.attacker.test",
            "https://identity.example/path",
        ] {
            let mut invalid = valid.clone();
            invalid.insert(http::header::ORIGIN, origin.parse().unwrap());
            assert!(
                verify_write(
                    &config,
                    clock.as_ref(),
                    &invalid,
                    None,
                    &SecurityBudget::new(clock.clone()).unwrap()
                )
                .is_err()
            );
        }
        for name in ["origin", "x-csrf-token"] {
            let mut invalid = valid.clone();
            let header = http::header::HeaderName::from_bytes(name.as_bytes()).unwrap();
            invalid.append(header.clone(), invalid.get(&header).unwrap().clone());
            assert!(
                verify_write(
                    &config,
                    clock.as_ref(),
                    &invalid,
                    None,
                    &SecurityBudget::new(clock.clone()).unwrap()
                )
                .is_err()
            );
        }
        assert!(
            verify_write(
                &config,
                clock.as_ref(),
                &valid,
                Some("other-proof"),
                &SecurityBudget::new(clock.clone()).unwrap()
            )
            .is_err()
        );
        let mut swapped = valid.clone();
        let raw_cookie = swapped
            .get(http::header::COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        swapped.insert(
            http::header::COOKIE,
            format!(
                "{raw_cookie}; {SSO_COOKIE}={}",
                IssuedSecret::from_entropy([4; 32]).into_response()
            )
            .parse()
            .unwrap(),
        );
        assert!(
            verify_write(
                &config,
                clock.as_ref(),
                &swapped,
                None,
                &SecurityBudget::new(clock.clone()).unwrap()
            )
            .is_err()
        );
        let mut legacy = valid.clone();
        let old = CsrfProtector::new(config.auth.secret.clone())
            .unwrap()
            .issue(&config.issuer(), None, clock.as_ref())
            .unwrap();
        legacy.insert("x-csrf-token", old.expose_secret().parse().unwrap());
        legacy.insert(
            http::header::COOKIE,
            raw_cookie
                .replace(anonymous.csrf_token.expose_secret(), old.expose_secret())
                .parse()
                .unwrap(),
        );
        assert!(
            verify_write(
                &config,
                clock.as_ref(),
                &legacy,
                None,
                &SecurityBudget::new(clock.clone()).unwrap()
            )
            .is_err()
        );
        clock.0.store(900_000, Ordering::SeqCst);
        assert!(
            verify_write(
                &config,
                clock.as_ref(),
                &valid,
                None,
                &SecurityBudget::new(clock.clone()).unwrap()
            )
            .is_err()
        );
    }
}

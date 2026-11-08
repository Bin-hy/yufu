use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
    budget::{Clock, ClockReading, healthy_reading},
    error::SecurityError,
    identity::{IdentityBinding, PrincipalKind, PrincipalRef, SecretToken, TokenUse},
    policy::CredentialBounds,
};

/// The registry key, never a substitute for querying current credential state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "CredentialRefWire")]
pub struct CredentialRef {
    issuer: String,
    jti: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialRefWire {
    issuer: String,
    jti: String,
}
fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
impl CredentialRef {
    pub fn new(issuer: impl Into<String>, jti: impl Into<String>) -> Result<Self, SecurityError> {
        let issuer = issuer.into();
        let jti = jti.into();
        PrincipalRef::new(&issuer, PrincipalKind::Machine, "credential-reference")?;
        if !valid_identifier(&jti) {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self { issuer, jti })
    }
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn jti(&self) -> &str {
        &self.jti
    }
}
impl TryFrom<CredentialRefWire> for CredentialRef {
    type Error = SecurityError;
    fn try_from(raw: CredentialRefWire) -> Result<Self, Self::Error> {
        Self::new(raw.issuer, raw.jti)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredentialProfile {
    kind: PrincipalKind,
    token_use: TokenUse,
}
impl CredentialProfile {
    pub fn new(kind: PrincipalKind, token_use: TokenUse) -> Result<Self, SecurityError> {
        let valid = matches!(
            (kind, token_use),
            (
                PrincipalKind::User,
                TokenUse::UserSession | TokenUse::McpAccess | TokenUse::StreamConnect
            ) | (
                PrincipalKind::Machine,
                TokenUse::SecurityService | TokenUse::PermissionSync
            ) | (PrincipalKind::Device, TokenUse::DeviceAccess)
        );
        if !valid {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self { kind, token_use })
    }
}

/// Server configuration only. Never derive these expectations from token contents or Host.
#[derive(Debug, Clone)]
pub struct CredentialExpectation {
    issuer: String,
    audience: String,
    application: String,
    profiles: Vec<CredentialProfile>,
}
impl CredentialExpectation {
    pub fn new(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        application: impl Into<String>,
        profiles: Vec<CredentialProfile>,
    ) -> Result<Self, SecurityError> {
        let issuer = issuer.into();
        let audience = audience.into();
        let application = application.into();
        PrincipalRef::new(&issuer, PrincipalKind::Machine, "verifier-expectation")?;
        if !valid_identifier(&audience) || !valid_identifier(&application) || profiles.is_empty() {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self {
            issuer,
            audience,
            application,
            profiles,
        })
    }
}

/// New access profile: security-relevant fields are mandatory, no legacy defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityAccessClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>,
    pub jti: String,
    pub principal_kind: PrincipalKind,
    pub token_use: TokenUse,
    pub application: String,
    pub grant_id: String,
    pub bounds: CredentialBounds,
    pub binding: IdentityBinding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_id: Option<String>,
}

/// Implementations load an authoritative consistent snapshot and verify all revocation
/// roots and the signing-key binding. Return registered claims, never echo the request.
#[async_trait::async_trait]
pub trait CurrentCredentialChecker: Send + Sync {
    async fn check_current(
        &self,
        verified: &VerifiedCredential,
        budget: &super::budget::SecurityBudget,
    ) -> Result<CurrentCredentialStatus, SecurityError>;
}

/// Trusted repository/transport result, not a public endpoint's deserialization type.
pub struct CurrentCredentialStatus {
    pub claims: SecurityAccessClaims,
    pub active: bool,
    pub validity: DecisionValidity,
}

#[derive(Clone)]
pub struct AuthenticatedContext {
    verified: VerifiedCredential,
    evidence: Arc<SecretToken>,
    validity: ActiveValidity,
}
impl std::fmt::Debug for AuthenticatedContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthenticatedContext")
            .field("principal", self.principal())
            .field("credential", self.credential())
            .finish_non_exhaustive()
    }
}
impl AuthenticatedContext {
    pub fn principal(&self) -> &PrincipalRef {
        self.verified.principal()
    }
    pub fn credential(&self) -> &CredentialRef {
        self.verified.reference()
    }
    pub fn token_use(&self) -> TokenUse {
        self.verified.claims.token_use
    }
    pub fn audience(&self) -> &str {
        &self.verified.claims.aud
    }
    pub fn application(&self) -> &str {
        &self.verified.claims.application
    }
    pub fn bounds(&self) -> &CredentialBounds {
        &self.verified.claims.bounds
    }
    pub fn binding(&self) -> &IdentityBinding {
        &self.verified.claims.binding
    }
    pub fn expires_at(&self) -> u64 {
        self.verified.claims.exp
    }
    pub fn evidence(&self) -> &SecretToken {
        &self.evidence
    }
    pub fn ensure_current(&self) -> Result<(), SecurityError> {
        self.validity.remaining().map(|_| ())
    }
}

/// Header extraction only. URL/query parameters and request extensions are not evidence.
pub fn extract_bearer(headers: &http::HeaderMap) -> Result<SecretToken, SecurityError> {
    let mut values = headers.get_all(http::header::AUTHORIZATION).iter();
    let value = values.next().ok_or(SecurityError::InvalidCredential)?;
    if values.next().is_some() {
        return Err(SecurityError::InvalidCredential);
    }
    let (scheme, token) = value
        .to_str()
        .map_err(|_| SecurityError::InvalidCredential)?
        .split_once(' ')
        .ok_or(SecurityError::InvalidCredential)?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(SecurityError::InvalidCredential);
    }
    if token.len() > 65_536 {
        return Err(SecurityError::InvalidCredential);
    }
    SecretToken::new(token.to_owned())
}

pub struct StrictAccessState {
    jwks: Arc<crate::jwks::JwksClient>,
    expected: CredentialExpectation,
    clock: Arc<dyn Clock>,
    checker: Arc<dyn CurrentCredentialChecker>,
}
impl StrictAccessState {
    pub fn new(
        jwks: Arc<crate::jwks::JwksClient>,
        expected: CredentialExpectation,
        clock: Arc<dyn Clock>,
        checker: Arc<dyn CurrentCredentialChecker>,
    ) -> Self {
        Self {
            jwks,
            expected,
            clock,
            checker,
        }
    }
    pub async fn authenticate(
        &self,
        headers: &http::HeaderMap,
    ) -> Result<AuthenticatedContext, SecurityError> {
        let budget = super::budget::SecurityBudget::new(self.clock.clone())?;
        let token = extract_bearer(headers)?;
        let verified = self
            .jwks
            .verify_credential(&token, &self.expected, self.clock.as_ref())?;
        budget.remaining()?;
        let status = budget
            .run(self.checker.check_current(&verified, &budget))
            .await??;
        if !status.active {
            return Err(SecurityError::CredentialRevoked);
        }
        if status.claims != verified.claims {
            return Err(SecurityError::InvalidCredential);
        }
        let expires = verified
            .claims
            .exp
            .checked_mul(1000)
            .ok_or(SecurityError::InvalidCredential)?;
        if status.validity.valid_until() > expires {
            return Err(SecurityError::InvalidCredential);
        }
        let validity = status
            .validity
            .bind_to_clock(self.clock.clone())
            .map_err(|error| {
                if error == SecurityError::DeadlineExceeded {
                    SecurityError::InvalidCredential
                } else {
                    error
                }
            })?;
        budget.remaining()?;
        Ok(AuthenticatedContext {
            verified,
            evidence: Arc::new(token),
            validity,
        })
    }
}

/// Only proves cryptographic/profile validity. A registry/status check is still required.
/// There is deliberately no Deserialize or public constructor for this type.
#[derive(Debug, Clone)]
pub struct VerifiedCredential {
    claims: SecurityAccessClaims,
    principal: PrincipalRef,
    reference: CredentialRef,
    signing_key_id: String,
}
impl VerifiedCredential {
    pub fn claims(&self) -> &SecurityAccessClaims {
        &self.claims
    }
    pub fn principal(&self) -> &PrincipalRef {
        &self.principal
    }
    pub fn reference(&self) -> &CredentialRef {
        &self.reference
    }
    pub fn signing_key_id(&self) -> &str {
        &self.signing_key_id
    }
}

pub(crate) fn access_key_id(token: &SecretToken) -> Result<String, SecurityError> {
    if token.expose_secret().len() > 65_536 {
        return Err(SecurityError::InvalidCredential);
    }
    restricted_jws_key_id(token.expose_secret(), "at+jwt")
}

/// Only this profile's supported JOSE parameters are accepted. In particular,
/// unknown `crit`/`b64` extensions must not be silently ignored by the JWT library.
pub(crate) fn restricted_jws_key_id(
    token: &str,
    expected_type: &str,
) -> Result<String, SecurityError> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SupportedHeader {
        alg: String,
        typ: String,
        kid: String,
    }
    let mut parts = token.split('.');
    let raw = parts
        .next()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(SecurityError::InvalidCredential)?;
    if parts.next().is_none_or(str::is_empty)
        || parts.next().is_none_or(str::is_empty)
        || parts.next().is_some()
    {
        return Err(SecurityError::InvalidCredential);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| SecurityError::InvalidCredential)?;
    let header: SupportedHeader =
        serde_json::from_slice(&bytes).map_err(|_| SecurityError::InvalidCredential)?;
    if header.alg != "RS256"
        || header.typ != expected_type
        || header.kid.is_empty()
        || header.kid.len() > 128
    {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(header.kid)
}

pub(crate) fn verify_access_with_key(
    token: &SecretToken,
    expected: &CredentialExpectation,
    key: &jsonwebtoken::DecodingKey,
    clock: &dyn Clock,
) -> Result<VerifiedCredential, SecurityError> {
    use jsonwebtoken::{Algorithm, Validation, decode};
    let signing_key_id = access_key_id(token)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&expected.issuer]);
    validation.set_audience(&[&expected.audience]);
    validation.set_required_spec_claims(&["iss", "sub", "aud", "iat", "exp", "jti"]);
    // Use the injected, health-checked clock instead of jsonwebtoken's process wall clock.
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.leeway = 0;
    healthy_reading(clock)?;
    let claims = decode::<SecurityAccessClaims>(token.expose_secret(), key, &validation)
        .map_err(|_| SecurityError::InvalidCredential)?
        .claims;
    let reading = healthy_reading(clock)?;
    let latest = reading
        .unix_millis
        .checked_add(CLOCK_ERROR_MILLIS)
        .ok_or(SecurityError::DependencyUnavailable)?;
    let earliest = reading.unix_millis.saturating_sub(CLOCK_ERROR_MILLIS);
    let expires = claims
        .exp
        .checked_mul(1000)
        .ok_or(SecurityError::InvalidCredential)?;
    let issued = claims
        .iat
        .checked_mul(1000)
        .ok_or(SecurityError::InvalidCredential)?;
    let profile = CredentialProfile::new(claims.principal_kind, claims.token_use)?;
    if claims.iss != expected.issuer
        || claims.aud != expected.audience
        || claims.application != expected.application
        || !expected.profiles.contains(&profile)
        || claims.exp <= claims.iat
        || expires <= latest
        || issued > latest
        || !valid_identifier(&claims.grant_id)
        || (claims.token_use == TokenUse::UserSession && claims.session_id.is_none())
        || (claims.token_use != TokenUse::UserSession
            && (claims.session_id.is_some() || claims.family_id.is_some()))
        || claims
            .session_id
            .as_ref()
            .is_some_and(|v| !valid_identifier(v))
        || claims
            .family_id
            .as_ref()
            .is_some_and(|v| !valid_identifier(v))
    {
        return Err(SecurityError::InvalidCredential);
    }
    if let Some(nbf) = claims.nbf {
        let not_before = nbf
            .checked_mul(1000)
            .ok_or(SecurityError::InvalidCredential)?;
        if not_before > earliest || not_before >= expires {
            return Err(SecurityError::InvalidCredential);
        }
    }
    let binding_valid = match (&claims.binding, claims.principal_kind) {
        (IdentityBinding::None, PrincipalKind::User | PrincipalKind::Machine) => true,
        (IdentityBinding::Gateway { gateway_id }, PrincipalKind::Machine) => {
            valid_identifier(gateway_id)
        }
        (
            IdentityBinding::Device {
                gateway_id,
                device_id,
            },
            PrincipalKind::Device,
        ) => {
            valid_identifier(gateway_id)
                && valid_identifier(device_id)
                && claims.aud == format!("cobotlet:{gateway_id}")
        }
        _ => false,
    };
    if !binding_valid {
        return Err(SecurityError::InvalidCredential);
    }
    let principal = PrincipalRef::new(&claims.iss, claims.principal_kind, &claims.sub)?;
    let reference = CredentialRef::new(&claims.iss, &claims.jti)?;
    Ok(VerifiedCredential {
        claims,
        principal,
        reference,
        signing_key_id,
    })
}

pub const MAX_DECISION_AGE_MILLIS: u64 = 5000;
pub const CLOCK_ERROR_MILLIS: u64 = 1000;

/// A freshness interval, not proof of a successful authorization or a signature.
/// Its source must be authenticated before it can authorize any operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ValidityWire")]
pub struct DecisionValidity {
    checked_at: u64,
    valid_until: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidityWire {
    checked_at: u64,
    valid_until: u64,
}

impl TryFrom<ValidityWire> for DecisionValidity {
    type Error = SecurityError;
    fn try_from(value: ValidityWire) -> Result<Self, Self::Error> {
        let upper = value
            .checked_at
            .checked_add(MAX_DECISION_AGE_MILLIS)
            .ok_or(SecurityError::InvalidCredential)?;
        if value.valid_until <= value.checked_at || value.valid_until > upper {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self {
            checked_at: value.checked_at,
            valid_until: value.valid_until,
        })
    }
}

impl DecisionValidity {
    /// checked_at is captured before the source's consistent primary-database query.
    pub fn from_query_start(
        checked_at: u64,
        credential_expires: u64,
        parent_expires: Option<u64>,
        inherited: Option<&Self>,
    ) -> Result<Self, SecurityError> {
        let mut valid_until = checked_at
            .checked_add(MAX_DECISION_AGE_MILLIS)
            .ok_or(SecurityError::InvalidCredential)?
            .min(credential_expires);
        if let Some(parent) = parent_expires {
            valid_until = valid_until.min(parent);
        }
        if let Some(original) = inherited {
            valid_until = valid_until.min(original.valid_until);
        }
        if valid_until <= checked_at {
            return Err(SecurityError::DeadlineExceeded);
        }
        Ok(Self {
            checked_at,
            valid_until,
        })
    }
    pub fn checked_at(&self) -> u64 {
        self.checked_at
    }
    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }

    /// Immediate check against a trusted clock; cache insertion must still pin a
    /// monotonic deadline with bind_to_clock.
    pub fn ensure_fresh(&self, clock: &dyn Clock) -> Result<(), SecurityError> {
        self.remaining_at(healthy_reading(clock)?).map(|_| ())
    }

    fn remaining_at(&self, reading: ClockReading) -> Result<Duration, SecurityError> {
        if !reading.synchronized {
            return Err(SecurityError::DependencyUnavailable);
        }
        let conservative_now = reading
            .unix_millis
            .checked_add(CLOCK_ERROR_MILLIS)
            .ok_or(SecurityError::DependencyUnavailable)?;
        if conservative_now < self.checked_at {
            return Err(SecurityError::DependencyUnavailable);
        }
        let millis = self
            .valid_until
            .checked_sub(conservative_now)
            .filter(|left| *left > 0)
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(Duration::from_millis(millis))
    }

    /// Pin a received interval to the local monotonic clock once, on cache insertion.
    /// Cache hits reuse the returned object, never recreate its duration from now.
    pub fn bind_to_clock(&self, clock: Arc<dyn Clock>) -> Result<ActiveValidity, SecurityError> {
        let reading = healthy_reading(clock.as_ref())?;
        let left = self.remaining_at(reading)?;
        let deadline = reading
            .monotonic
            .checked_add(left)
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(ActiveValidity {
            clock,
            validity: *self,
            bound_at: reading.monotonic,
            deadline,
        })
    }
}

#[derive(Clone)]
pub struct ActiveValidity {
    clock: Arc<dyn Clock>,
    validity: DecisionValidity,
    bound_at: Duration,
    deadline: Duration,
}
impl ActiveValidity {
    pub fn remaining(&self) -> Result<Duration, SecurityError> {
        let reading = healthy_reading(self.clock.as_ref())?;
        if reading.monotonic < self.bound_at {
            return Err(SecurityError::DependencyUnavailable);
        }
        let mono_left = self
            .deadline
            .checked_sub(reading.monotonic)
            .filter(|left| !left.is_zero())
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(mono_left.min(self.validity.remaining_at(reading)?))
    }
    pub fn validity(&self) -> DecisionValidity {
        self.validity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::budget::tests::ManualClock;

    #[test]
    fn security_t06_parent_expiry_and_original_window_bound_decision() {
        let validity =
            DecisionValidity::from_query_start(100_000, 200_000, Some(103_000), None).unwrap();
        assert_eq!(validity.valid_until(), 103_000);
        let refreshed =
            DecisionValidity::from_query_start(101_000, 200_000, None, Some(&validity)).unwrap();
        assert_eq!(refreshed.valid_until(), 103_000);
        assert!(DecisionValidity::from_query_start(u64::MAX - 1, u64::MAX, None, None).is_err());
    }

    #[test]
    fn security_t06_delayed_response_and_expiry_have_no_grace() {
        let clock = Arc::new(ManualClock::new());
        let validity = DecisionValidity::from_query_start(100_000, 200_000, None, None).unwrap();
        assert_eq!(validity.valid_until(), 105_000);
        clock.advance(4000);
        assert!(validity.bind_to_clock(clock).is_err());
        let invalid = serde_json::json!({"checked_at":100_000,"valid_until":110_000});
        assert!(serde_json::from_value::<DecisionValidity>(invalid).is_err());
    }

    #[test]
    fn security_t06_cache_cannot_survive_wall_rollback_or_rebind_expired_decision() {
        let clock = Arc::new(ManualClock::new());
        let validity = DecisionValidity::from_query_start(100_000, 200_000, None, None).unwrap();
        let bound = validity.bind_to_clock(clock.clone()).unwrap();
        clock.advance(3999);
        assert_eq!(bound.remaining().unwrap(), Duration::from_millis(1));
        clock.advance(1);
        clock.0.lock().unwrap().unix_millis = 100_000;
        assert_eq!(bound.remaining(), Err(SecurityError::DeadlineExceeded));
        clock.0.lock().unwrap().unix_millis = 106_000;
        assert!(validity.bind_to_clock(clock).is_err());
    }

    #[test]
    fn security_t06_future_source_and_unhealthy_clock_fail_closed() {
        let clock = Arc::new(ManualClock::new());
        let future = DecisionValidity::from_query_start(110_000, 200_000, None, None).unwrap();
        assert!(future.bind_to_clock(clock.clone()).is_err());
        clock.0.lock().unwrap().synchronized = false;
        let valid = DecisionValidity::from_query_start(100_000, 200_000, None, None).unwrap();
        assert!(valid.bind_to_clock(clock).is_err());
    }
}

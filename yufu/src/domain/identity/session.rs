use super::account::{AccountSnapshot, SecretDigest, ensure_enabled};
use super::admission::ManagedAdmission;
use super::application::{ExactRedirectUri, IdentityScopeSet};
use super::error::IdentityError;
use auth::security::identity::{PrincipalKind, PrincipalRef};
use uuid::Uuid;

pub const DEFAULT_SSO_TTL_SECONDS: u64 = 12 * 60 * 60;
pub const DEFAULT_FLOW_TTL_SECONDS: u64 = 10 * 60;
pub const MAX_PENDING_FLOWS: usize = 8;
pub const DEFAULT_CODE_TTL_SECONDS: u64 = 60;
pub const DEFAULT_ACCESS_TTL_SECONDS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSessionSnapshot {
    pub id: Uuid,
    pub principal: PrincipalRef,
    pub user_id: i32,
    pub secret_digest: SecretDigest,
    pub auth_epoch: u64,
    pub authenticated_at: u64,
    pub expires_at: u64,
    pub revoked_at: Option<u64>,
}
impl BrowserSessionSnapshot {
    pub fn validate(
        &self,
        account: &AccountSnapshot,
        now_millis: u64,
    ) -> Result<(), IdentityError> {
        ensure_enabled(account)?;
        if self.revoked_at.is_some()
            || now_millis >= self.expires_at
            || self.authenticated_at > now_millis
            || self.expires_at <= self.authenticated_at
            || self.user_id != account.user_id
            || self.principal.kind() != PrincipalKind::User
            || self.principal.subject() != account.user_id.to_string()
            || self.auth_epoch != account.auth_epoch
        {
            return Err(IdentityError::Unauthenticated);
        }
        Ok(())
    }
    /// Restore is a read: no rotation, expiry extension or change to auth_time.
    pub fn restore(
        &self,
        account: &AccountSnapshot,
        now_millis: u64,
    ) -> Result<Self, IdentityError> {
        self.validate(account, now_millis)?;
        Ok(self.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowPurpose {
    Authorization,
    Invitation,
    PasswordReset,
    LogoutConfirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTarget {
    Invitation(Uuid),
    PasswordReset(Uuid),
    LogoutConfirmation(Uuid),
}
impl ActionTarget {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Invitation(_) => "invitation",
            Self::PasswordReset(_) => "password_reset",
            Self::LogoutConfirmation(_) => "logout_confirmation",
        }
    }
    pub fn id(&self) -> Uuid {
        match self {
            Self::Invitation(id) | Self::PasswordReset(id) | Self::LogoutConfirmation(id) => *id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionFlowSnapshot {
    pub id: Uuid,
    pub target: ActionTarget,
    pub browser_binding_digest: SecretDigest,
    pub browser_proof_digest: SecretDigest,
    pub expires_at: u64,
    pub consumed_at: Option<u64>,
    pub request_payload: serde_json::Value,
}
impl ActionFlowSnapshot {
    pub fn verify(
        &self,
        target: ActionTarget,
        browser: &SecretDigest,
        proof: &SecretDigest,
        now_millis: u64,
    ) -> Result<(), IdentityError> {
        if self.target != target
            || &self.browser_binding_digest != browser
            || &self.browser_proof_digest != proof
        {
            return Err(IdentityError::Forbidden);
        }
        if self.consumed_at.is_some() {
            return Err(IdentityError::AlreadyConsumed);
        }
        if now_millis >= self.expires_at {
            return Err(IdentityError::Expired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FlowHandle {
    pub(crate) id: Uuid,
    pub(crate) purpose: FlowPurpose,
    pub(crate) browser_proof_digest: SecretDigest,
    pub(crate) expires_at: u64,
    pub(crate) completed_at: Option<u64>,
}

pub struct AuthorizationFlowSnapshot {
    pub handle: FlowHandle,
    pub environment_id: Uuid,
    pub config_version_id: Uuid,
    pub browser_binding_digest: SecretDigest,
    pub request_payload: serde_json::Value,
    pub bound_sso_session_id: Option<Uuid>,
    pub reauth_completed_at: Option<u64>,
}
impl FlowHandle {
    pub fn pending(
        id: Uuid,
        purpose: FlowPurpose,
        browser_proof_digest: SecretDigest,
        expires_at: u64,
    ) -> Result<Self, IdentityError> {
        if id.is_nil() || expires_at == 0 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self {
            id,
            purpose,
            browser_proof_digest,
            expires_at,
            completed_at: None,
        })
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn verify(
        &self,
        purpose: FlowPurpose,
        browser_proof: &str,
        now_millis: u64,
    ) -> Result<(), IdentityError> {
        if self.purpose != purpose || !self.browser_proof_digest.matches(browser_proof) {
            return Err(IdentityError::Forbidden);
        }
        if self.completed_at.is_some() {
            return Err(IdentityError::AlreadyConsumed);
        }
        if now_millis >= self.expires_at {
            return Err(IdentityError::Expired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Prompt {
    pub login: bool,
    pub consent: bool,
    pub none: bool,
}
impl Prompt {
    pub fn parse(value: Option<&str>) -> Result<Self, IdentityError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        if value.is_empty() {
            return Err(IdentityError::InvalidInput);
        }
        let mut prompt = Self::default();
        for part in value.split(' ') {
            match part {
                "login" if !prompt.login => prompt.login = true,
                "consent" if !prompt.consent => prompt.consent = true,
                "none" if !prompt.none => prompt.none = true,
                _ => return Err(IdentityError::InvalidInput),
            }
        }
        if prompt.none && (prompt.login || prompt.consent) {
            return Err(IdentityError::InvalidInput);
        }
        Ok(prompt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionDecision {
    Continue,
    Login,
    Consent,
    LoginRequired,
    ConsentRequired,
}

/// max_age=0 and prompt=login are satisfied only by reauthentication in this flow.
pub fn interaction_decision(
    prompt: Prompt,
    max_age: Option<u64>,
    authenticated_at: Option<u64>,
    now_seconds: u64,
    reauthenticated_in_flow: bool,
    consent_current: bool,
) -> Result<InteractionDecision, IdentityError> {
    let aged = match authenticated_at {
        Some(at) => {
            let age = now_seconds
                .checked_sub(at)
                .ok_or(IdentityError::Unauthenticated)?;
            max_age.is_some_and(|max| age > max || max == 0)
        }
        None => true,
    };
    let needs_login =
        authenticated_at.is_none() || ((prompt.login || aged) && !reauthenticated_in_flow);
    if needs_login {
        return Ok(if prompt.none {
            InteractionDecision::LoginRequired
        } else {
            InteractionDecision::Login
        });
    }
    if prompt.consent || !consent_current {
        return Ok(if prompt.none {
            InteractionDecision::ConsentRequired
        } else {
            InteractionDecision::Consent
        });
    }
    Ok(InteractionDecision::Continue)
}

#[derive(Debug, Clone)]
pub struct IdentityTokenContext {
    pub admission: ManagedAdmission,
    pub client_id: String,
    pub protocol_sid: Uuid,
    pub nonce: Option<String>,
    pub root_expires_at: u64,
}
impl IdentityTokenContext {
    pub fn expiry_seconds(
        &self,
        now_seconds: u64,
        requested_ttl: u64,
    ) -> Result<u64, IdentityError> {
        let root_expires_seconds = self.root_expires_at / 1000;
        if self.admission.auth_time > now_seconds
            || now_seconds >= root_expires_seconds
            || requested_ttl == 0
        {
            return Err(IdentityError::Unauthenticated);
        }
        Ok(now_seconds
            .checked_add(requested_ttl)
            .ok_or(IdentityError::CapacityExceeded)?
            .min(root_expires_seconds))
    }
    pub fn refresh(
        &self,
        current: ManagedAdmission,
        now_seconds: u64,
    ) -> Result<Self, IdentityError> {
        if !self.admission.still_current(&current) {
            return Err(IdentityError::AccessDenied);
        }
        self.expiry_seconds(now_seconds, DEFAULT_ACCESS_TTL_SECONDS)?;
        Ok(Self {
            admission: current,
            nonce: self.nonce.clone(),
            ..self.clone()
        })
    }
}

#[derive(Debug, Clone)]
pub struct ConsentSummary {
    pub application_name: String,
    pub scopes: IdentityScopeSet,
}
pub struct ValidatedAuthorizationResponse {
    pub(crate) redirect_uri: ExactRedirectUri,
    pub(crate) code: super::account::IssuedSecret,
    pub(crate) state: String,
}
impl ValidatedAuthorizationResponse {
    pub fn into_redirect(self) -> String {
        let code = self.code.into_response();
        append_query(
            self.redirect_uri.as_str(),
            &[("code", &code), ("state", &self.state)],
        )
    }
}
#[derive(Debug, Clone)]
pub struct ValidatedAuthorizationError {
    pub(crate) redirect_uri: ExactRedirectUri,
    pub(crate) error: &'static str,
    pub(crate) state: Option<String>,
}
impl ValidatedAuthorizationError {
    pub fn into_redirect(self) -> String {
        let mut pairs = vec![("error", self.error)];
        if let Some(state) = self.state.as_deref() {
            pairs.push(("state", state));
        }
        append_query(self.redirect_uri.as_str(), &pairs)
    }
}
fn append_query(uri: &str, pairs: &[(&str, &str)]) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().copied())
        .finish();
    format!("{uri}{}{query}", if uri.contains('?') { '&' } else { '?' })
}
pub enum AuthorizationOutcome {
    RedirectToClient(ValidatedAuthorizationResponse),
    RequireLogin(FlowHandle),
    RequireConsent(FlowHandle, ConsentSummary),
    ProtocolError(ValidatedAuthorizationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityRevocationScope {
    CurrentCenter { sso_session_id: Uuid },
    UserAll { user_id: i32 },
    ApplicationLocal { rp_session_id: Uuid },
    Environment { environment_id: Uuid },
    EnvironmentUser { environment_id: Uuid, user_id: i32 },
    Grant { grant_id: Uuid },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::account::{PersonKind, stable_principal};

    #[test]
    fn test_action_requires_same_target_browser_proof_and_live_one_use_flow() {
        let target = ActionTarget::Invitation(Uuid::new_v4());
        let browser = SecretDigest::of("one-browser");
        let proof = SecretDigest::of("one-action-proof");
        let mut flow = ActionFlowSnapshot {
            id: Uuid::new_v4(),
            target,
            browser_binding_digest: browser.clone(),
            browser_proof_digest: proof.clone(),
            expires_at: 1_000,
            consumed_at: None,
            request_payload: serde_json::json!({}),
        };
        assert_eq!(flow.verify(target, &browser, &proof, 999), Ok(()));
        assert_eq!(
            flow.verify(target, &SecretDigest::of("other-browser"), &proof, 999),
            Err(IdentityError::Forbidden)
        );
        assert_eq!(
            flow.verify(target, &browser, &SecretDigest::of("other-proof"), 999),
            Err(IdentityError::Forbidden)
        );
        assert_eq!(
            flow.verify(
                ActionTarget::PasswordReset(target.id()),
                &browser,
                &proof,
                999
            ),
            Err(IdentityError::Forbidden)
        );
        assert_eq!(
            flow.verify(target, &browser, &proof, 1_000),
            Err(IdentityError::Expired)
        );
        flow.consumed_at = Some(900);
        assert_eq!(
            flow.verify(target, &browser, &proof, 999),
            Err(IdentityError::AlreadyConsumed)
        );
    }

    fn account() -> AccountSnapshot {
        AccountSnapshot {
            user_id: 7,
            enabled: true,
            person_kind: PersonKind::Employee,
            developer_enabled: true,
            administrator: false,
            auth_epoch: 2,
        }
    }
    fn root() -> BrowserSessionSnapshot {
        BrowserSessionSnapshot {
            id: Uuid::new_v4(),
            principal: stable_principal("https://id.example", 7).unwrap(),
            user_id: 7,
            secret_digest: SecretDigest::of("root"),
            auth_epoch: 2,
            authenticated_at: 100_000,
            expires_at: 200_000,
            revoked_at: None,
        }
    }

    #[test]
    fn test_restore_does_not_rotate_or_extend_root_or_authentication_time() {
        let root = root();
        let a = account();
        assert_eq!(root.restore(&a, 150_000).unwrap(), root);
        assert_eq!(root.restore(&a, 190_000).unwrap(), root);
        assert!(root.restore(&a, 200_000).is_err());
        assert!(root.restore(&a, 99_000).is_err());
        let mut changed = a.clone();
        changed.auth_epoch += 1;
        assert!(root.restore(&changed, 150_000).is_err());
        let mut revoked = root;
        revoked.revoked_at = Some(140_000);
        assert!(revoked.restore(&a, 150_000).is_err());
    }

    #[test]
    fn test_flow_proofs_are_separate_bound_and_one_use() {
        let mut flow = FlowHandle {
            id: Uuid::new_v4(),
            purpose: FlowPurpose::Authorization,
            browser_proof_digest: SecretDigest::of("browser-one"),
            expires_at: 1000,
            completed_at: None,
        };
        assert_eq!(flow.id(), flow.id);
        assert!(
            flow.verify(FlowPurpose::Authorization, "browser-one", 999)
                .is_ok()
        );
        assert_eq!(
            flow.verify(FlowPurpose::Invitation, "browser-one", 999),
            Err(IdentityError::Forbidden)
        );
        assert_eq!(
            flow.verify(FlowPurpose::Authorization, "browser-two", 999),
            Err(IdentityError::Forbidden)
        );
        assert_eq!(
            flow.verify(FlowPurpose::Authorization, "browser-one", 1000),
            Err(IdentityError::Expired)
        );
        flow.completed_at = Some(900);
        assert_eq!(
            flow.verify(FlowPurpose::Authorization, "browser-one", 999),
            Err(IdentityError::AlreadyConsumed)
        );
    }

    #[test]
    fn test_authorization_redirect_encodes_state_and_preserves_registered_query() {
        use crate::domain::identity::application::AppEnvironmentKind;
        let redirect = ExactRedirectUri::parse(
            "https://app.example/cb?channel=one",
            AppEnvironmentKind::Production,
        )
        .unwrap();
        let response = ValidatedAuthorizationResponse {
            redirect_uri: redirect.clone(),
            code: super::super::account::IssuedSecret::from_entropy([7; 32]),
            state: "state&other=bad".into(),
        };
        let url = url::Url::parse(&response.into_redirect()).unwrap();
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "state").unwrap().1,
            "state&other=bad"
        );
        assert!(!url.query_pairs().any(|(key, _)| key == "other"));
        let response = ValidatedAuthorizationError {
            redirect_uri: redirect,
            error: "login_required",
            state: Some("s".into()),
        };
        assert_eq!(
            response.into_redirect(),
            "https://app.example/cb?channel=one&error=login_required&state=s"
        );
    }

    #[test]
    fn test_prompt_validation_and_noninteractive_standard_errors() {
        for value in [
            "",
            "none login",
            "none consent",
            "login login",
            "select_account",
            "login  consent",
        ] {
            assert!(Prompt::parse(Some(value)).is_err(), "{value}");
        }
        assert!(Prompt::parse(Some("login consent")).unwrap().consent);
        let none = Prompt::parse(Some("none")).unwrap();
        assert_eq!(
            interaction_decision(none, None, None, 100, false, true).unwrap(),
            InteractionDecision::LoginRequired
        );
        assert_eq!(
            interaction_decision(none, None, Some(90), 100, false, false).unwrap(),
            InteractionDecision::ConsentRequired
        );
        assert_eq!(
            interaction_decision(none, None, Some(90), 100, false, true).unwrap(),
            InteractionDecision::Continue
        );
    }

    #[test]
    fn test_max_age_and_prompt_login_require_actual_reauthentication() {
        let prompt = Prompt::default();
        assert_eq!(
            interaction_decision(prompt, Some(10), Some(90), 100, false, true).unwrap(),
            InteractionDecision::Continue
        );
        assert_eq!(
            interaction_decision(prompt, Some(9), Some(90), 100, false, true).unwrap(),
            InteractionDecision::Login
        );
        assert_eq!(
            interaction_decision(prompt, Some(0), Some(100), 100, false, true).unwrap(),
            InteractionDecision::Login
        );
        assert_eq!(
            interaction_decision(prompt, Some(0), Some(100), 100, true, true).unwrap(),
            InteractionDecision::Continue
        );
        assert!(interaction_decision(prompt, None, Some(101), 100, false, true).is_err());
        let prompt = Prompt::parse(Some("login")).unwrap();
        assert_eq!(
            interaction_decision(prompt, None, Some(100), 100, false, true).unwrap(),
            InteractionDecision::Login
        );
        assert_eq!(
            interaction_decision(prompt, None, Some(100), 100, true, true).unwrap(),
            InteractionDecision::Continue
        );
    }

    #[test]
    fn test_refresh_preserves_auth_time_sid_scopes_and_root_expiry() {
        let root = root();
        let admission = ManagedAdmission {
            principal: root.principal.clone(),
            sso_session_id: root.id,
            environment_id: Uuid::new_v4(),
            config_version_id: Uuid::new_v4(),
            auth_epoch: 2,
            lifecycle_epoch: 3,
            access_generation: 4,
            scopes: IdentityScopeSet::parse("openid email").unwrap(),
            auth_time: 100,
        };
        let context = IdentityTokenContext {
            admission: admission.clone(),
            client_id: "rp-one".into(),
            protocol_sid: Uuid::new_v4(),
            nonce: Some("nonce-one".into()),
            root_expires_at: root.expires_at,
        };
        assert_eq!(context.expiry_seconds(150, 300).unwrap(), 200);
        let refreshed = context.refresh(admission.clone(), 150).unwrap();
        assert_eq!(refreshed.admission, context.admission);
        assert_eq!(refreshed.protocol_sid, context.protocol_sid);
        assert_eq!(refreshed.nonce, context.nonce);
        assert_eq!(refreshed.root_expires_at, context.root_expires_at);
        assert!(context.refresh(admission.clone(), 200).is_err());
        let mut obsolete = admission;
        obsolete.access_generation += 1;
        assert!(context.refresh(obsolete, 150).is_err());
    }
}

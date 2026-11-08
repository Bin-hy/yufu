//! Credential/root liveness. Signature proof and current resource policy remain
//! separate required steps; looking up an arbitrary reference is not authentication.
use std::sync::Arc;

use super::repository::{
    CredentialSnapshot, RegisteredState, SecurityRepository, SecurityTransaction,
    SessionRegistration, decode, now, refresh_digest,
};
use auth::security::{
    budget::{Clock, SecurityBudget},
    credential::{
        CLOCK_ERROR_MILLIS, CredentialProfile, CredentialRef, CurrentCredentialChecker,
        CurrentCredentialStatus, DecisionValidity, SecurityAccessClaims, VerifiedCredential,
    },
    error::SecurityError,
    identity::{IdentityBinding, PrincipalKind, PrincipalRef, SecretToken, TokenUse},
    policy::{CredentialBounds, ResourceRef, ResourceTarget},
};

/// Transport input only; identity, roots and bounds are loaded on the primary.
pub struct CodeExchangeRequest<'a> {
    pub code: &'a SecretToken,
    pub client_id: &'a str,
    pub client_secret: Option<&'a SecretToken>,
    pub redirect_uri: Option<&'a str>,
    pub code_verifier: Option<&'a SecretToken>,
}

pub struct SessionTokenConfig {
    pub issuer: String,
    pub access_ttl_seconds: u64,
    pub refresh_ttl_seconds: u64,
}
pub struct RefreshRequest<'a> {
    pub refresh_token: &'a SecretToken,
    pub client_id: &'a str,
    pub client_secret: Option<&'a SecretToken>,
}
#[derive(Clone, Copy)]
enum SessionClient<'a> {
    Local,
    Oidc {
        client_id: &'a str,
        client_secret: Option<&'a SecretToken>,
    },
}

/// bcrypt consumes at most 72 bytes. Reject longer input instead of accepting
/// a silently truncated password; spaces remain valid password characters.
pub struct LocalPassword(String);
impl LocalPassword {
    pub fn new(value: String) -> Result<Self, SecurityError> {
        if value.is_empty() || value.len() > 72 {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self(value))
    }
}
impl std::fmt::Debug for LocalPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalPassword([REDACTED])")
    }
}
pub struct LocalLoginRequest<'a> {
    pub username: &'a str,
    pub password: &'a LocalPassword,
}
pub struct LocalRegisterRequest<'a> {
    pub username: &'a str,
    pub password: &'a LocalPassword,
    pub nickname: &'a str,
}
pub struct LocalSession {
    pub tokens: SessionTokenSet,
    pub user: crate::entity::user::UserVO,
}

/// Injectable signing boundary. Implementations must use the dedicated RS256
/// access profile; secrets and backend errors never enter Debug or storage.
#[async_trait::async_trait]
#[allow(clippy::double_must_use)]
pub trait SessionTokenSigner: Send + Sync {
    fn key_id(&self) -> &str;
    async fn sign_access(
        &self,
        claims: &SecurityAccessClaims,
    ) -> Result<SecretToken, SecurityError>;
    async fn sign_id(
        &self,
        claims: &SecurityAccessClaims,
        client_id: &str,
        nonce: Option<&str>,
        access: &SecretToken,
    ) -> Result<SecretToken, SecurityError>;
}

#[derive(Debug)]
pub struct SessionTokenSet {
    pub access_token: SecretToken,
    pub id_token: Option<SecretToken>,
    pub refresh_token: SecretToken,
    pub claims: SecurityAccessClaims,
}

pub struct CredentialLifecycle {
    repository: Arc<dyn SecurityRepository>,
    clock: Arc<dyn Clock>,
}
impl CredentialLifecycle {
    pub fn new(repository: Arc<dyn SecurityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn login_local(
        &self,
        request: LocalLoginRequest<'_>,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<LocalSession, SecurityError> {
        validate_local_username(request.username)?;
        let mut txn = self.repository.begin(budget).await?;
        let user = txn
            .local_user(request.username)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let password_hash = user
            .password
            .clone()
            .ok_or(SecurityError::InvalidCredential)?;
        let password = request.password.0.clone();
        let valid = budget
            .run(tokio::task::spawn_blocking(move || {
                bcrypt::verify(password, &password_hash)
            }))
            .await?
            .map_err(|_| SecurityError::DependencyUnavailable)?
            .map_err(|_| SecurityError::InvalidCredential)?;
        if !valid {
            return Err(SecurityError::InvalidCredential);
        }
        let tokens = self
            .mint_local_session(txn.as_mut(), user.id, config, signer, budget)
            .await?;
        txn.commit().await?;
        Ok(LocalSession {
            tokens,
            user: user.into(),
        })
    }
    pub async fn register_local(
        &self,
        request: LocalRegisterRequest<'_>,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<LocalSession, SecurityError> {
        validate_local_username(request.username)?;
        if request.nickname.chars().count() > 255 {
            return Err(SecurityError::InvalidCredential);
        }
        let password = request.password.0.clone();
        let password_hash = budget
            .run(tokio::task::spawn_blocking(move || {
                bcrypt::hash(password, bcrypt::DEFAULT_COST)
            }))
            .await?
            .map_err(|_| SecurityError::DependencyUnavailable)?
            .map_err(|_| SecurityError::DependencyUnavailable)?;
        let password_hash = SecretToken::new(password_hash)?;
        let mut txn = self.repository.begin(budget).await?;
        let user = txn
            .register_local_user(request.username, &password_hash, request.nickname)
            .await?;
        let tokens = self
            .mint_local_session(txn.as_mut(), user.id, config, signer, budget)
            .await?;
        txn.commit().await?;
        Ok(LocalSession {
            tokens,
            user: user.into(),
        })
    }
    async fn mint_local_session(
        &self,
        txn: &mut dyn SecurityTransaction,
        user_id: crate::IdType,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        let mut claims = session_claims(
            config,
            user_id,
            "cobotusr".into(),
            "cobotusr".into(),
            now(self.clock.as_ref())?,
        )?;
        claims.bounds = txn.session_bounds(&claims).await?;
        let (set, registration) = mint_session_pair(
            SessionMint {
                claims,
                client_id: None,
                nonce: None,
                minted_by: None,
                new_roots: true,
            },
            config,
            signer,
            budget,
        )
        .await?;
        txn.register_session_pair(registration).await?;
        budget.remaining()?;
        Ok(set)
    }
    pub async fn rotate_local_refresh(
        &self,
        refresh: &SecretToken,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        self.rotate_session(refresh, SessionClient::Local, config, signer, budget)
            .await
    }
    /// Preserve the existing OIDC refresh revocation endpoint while it migrates
    /// to the shared registry. Broader logout/application scopes belong to T25.
    pub async fn revoke_session_refresh(
        &self,
        request: RefreshRequest<'_>,
        issuer: &str,
        budget: &SecurityBudget,
    ) -> Result<(), SecurityError> {
        let mut txn = self.repository.begin(budget).await?;
        let client = txn
            .oidc_client(request.client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        authenticate_session_client(&client, request.client_secret, "refresh_token", budget)
            .await?;
        if let Some(snapshot) = txn.lock_refresh(issuer, request.refresh_token).await? {
            let row = &snapshot.current.credential;
            if row.oidc_client_id.as_deref() == Some(client.client_id.as_str()) {
                validate_refresh_roots(&snapshot.current)?;
                let paired = CredentialRef::new(
                    &row.issuer,
                    row.paired_access_jti
                        .as_deref()
                        .ok_or(SecurityError::InvalidCredential)?,
                )?;
                let access = txn
                    .credential(&paired)
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                validate_session_access(row, &access.current.credential)?;
                txn.revoke_session_pair(snapshot.reference(), now(self.clock.as_ref())?)
                    .await?;
            }
        }
        txn.commit().await
    }
    pub async fn rotate_refresh(
        &self,
        request: RefreshRequest<'_>,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        self.rotate_session(
            request.refresh_token,
            SessionClient::Oidc {
                client_id: request.client_id,
                client_secret: request.client_secret,
            },
            config,
            signer,
            budget,
        )
        .await
    }
    async fn rotate_session(
        &self,
        refresh: &SecretToken,
        client: SessionClient<'_>,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        // A REPEATABLE READ loser must reopen its snapshot to observe a consumed
        // refresh. Every attempt inherits the caller's original deadline.
        for attempt in 0..4 {
            budget.remaining()?;
            match self
                .rotate_refresh_once(refresh, client, config, signer, budget)
                .await
            {
                Err(SecurityError::Conflict) if attempt < 3 => tokio::task::yield_now().await,
                result => return result,
            }
        }
        Err(SecurityError::Conflict)
    }

    async fn rotate_refresh_once(
        &self,
        refresh: &SecretToken,
        source: SessionClient<'_>,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        let mut txn = self.repository.begin(budget).await?;
        let (client_id, audience, application) = match source {
            SessionClient::Local => (None, "cobotusr".to_owned(), "cobotusr".to_owned()),
            SessionClient::Oidc {
                client_id,
                client_secret,
            } => {
                let client = txn
                    .oidc_client(client_id)
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                authenticate_session_client(&client, client_secret, "refresh_token", budget)
                    .await?;
                let binding = txn
                    .session_client(&config.issuer, &client.client_id)
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                (
                    Some(client.client_id),
                    binding.audience,
                    binding.application,
                )
            }
        };
        let snapshot = txn
            .lock_refresh(&config.issuer, refresh)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let row = &snapshot.current.credential;
        if row.audience != audience
            || row.application != application
            || row.oidc_client_id != client_id
            || row.principal_kind != "user"
            || row.token_use != "user_session"
            || row.revocation_parent_jti.is_some()
        {
            return Err(SecurityError::InvalidCredential);
        }
        let at = now(self.clock.as_ref())?;
        validate_refresh_roots(&snapshot.current)?;
        if row.consumed_at.is_some() {
            // Expired consumed summaries remain evidence of replay. Do not
            // check their expiry before protecting the live family.
            txn.revoke_refresh_family(snapshot.reference(), at).await?;
            txn.commit().await?;
            return Err(SecurityError::CredentialRevoked);
        }
        let mut access_state = snapshot.current.clone();
        access_state.credential.credential_kind = "access".into();
        access_state.credential.signing_key_id = Some(signer.key_id().into());
        let mut claims = active_claims(&access_state, at)?;
        let paired = CredentialRef::new(
            &row.issuer,
            row.paired_access_jti
                .as_deref()
                .ok_or(SecurityError::InvalidCredential)?,
        )?;
        let paired_snapshot = txn
            .credential(&paired)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let access = &paired_snapshot.current.credential;
        validate_session_access(row, access)?;
        txn.consume_refresh_pair(snapshot.reference(), &paired, at)
            .await?;
        claims.jti = uuid::Uuid::new_v4().to_string();
        claims.iat = at / 1000;
        claims.exp = expiry(claims.iat, config.access_ttl_seconds)?;
        claims.nbf = None;
        // A replacement retains the original upper bound, independently of any
        // roles added since issuance. Current policy is still checked on access.
        let (set, registration) = mint_session_pair(
            SessionMint {
                claims,
                client_id: client_id.as_deref(),
                nonce: None,
                minted_by: Some(snapshot.reference().clone()),
                new_roots: false,
            },
            config,
            signer,
            budget,
        )
        .await?;
        let replacement = registration.refresh_jti.clone();
        txn.register_session_pair(registration).await?;
        txn.link_refresh_replacement(snapshot.reference(), &replacement)
            .await?;
        budget.remaining()?;
        txn.commit().await?;
        Ok(set)
    }

    pub async fn consume_authorization_code(
        &self,
        request: CodeExchangeRequest<'_>,
        config: &SessionTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        let mut txn = self.repository.begin(budget).await?;
        let client = txn
            .oidc_client(request.client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        authenticate_session_client(&client, request.client_secret, "authorization_code", budget)
            .await?;
        let binding = txn
            .session_client(&config.issuer, &client.client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let code = txn
            .authorization_code(request.code)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let at = now(self.clock.as_ref())?;
        if code.used
            || code.client_id != client.client_id
            || code.redirect_uri.as_deref() != request.redirect_uri
            || request.redirect_uri.is_none_or(|uri| {
                !crate::service::oidc::OidcService::client_has_redirect(&client, uri)
            })
            || code.expires_at.and_utc().timestamp_millis()
                <= i64::try_from(at.saturating_add(1000))
                    .map_err(|_| SecurityError::InvalidCredential)?
            || !crate::service::oidc::OidcService::public_client_requires_s256_pkce(
                &client,
                Some(&code.code_challenge),
                &code.code_challenge_method,
            )
        {
            return Err(SecurityError::InvalidCredential);
        }
        if !code.code_challenge.is_empty() {
            let verifier = request
                .code_verifier
                .ok_or(SecurityError::InvalidCredential)?
                .expose_secret();
            if !(43..=128).contains(&verifier.len())
                || !verifier
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
                || !crate::service::oidc::OidcService::verify_pkce(
                    &code.code_challenge,
                    &code.code_challenge_method,
                    verifier,
                )
            {
                return Err(SecurityError::InvalidCredential);
            }
        }
        let mut claims = session_claims(
            config,
            code.user_id,
            binding.audience,
            binding.application,
            at,
        )?;
        claims.bounds = txn.session_bounds(&claims).await?;
        txn.consume_code(code.id, now(self.clock.as_ref())?).await?;
        let (set, registration) = mint_session_pair(
            SessionMint {
                claims,
                client_id: Some(&client.client_id),
                nonce: code.nonce.as_deref(),
                minted_by: None,
                new_roots: true,
            },
            config,
            signer,
            budget,
        )
        .await?;
        txn.register_session_pair(registration).await?;
        budget.remaining()?;
        txn.commit().await?;
        Ok(set)
    }

    /// Authoritative storage status only. Callers must first verify the relevant
    /// access/operation signature, then compare all registered bindings and policy.
    pub async fn check_registered(
        &self,
        reference: &CredentialRef,
        budget: &SecurityBudget,
    ) -> Result<CurrentCredentialStatus, SecurityError> {
        self.load_active(reference, budget)
            .await
            .map(|(status, _)| status)
    }

    async fn load_active(
        &self,
        reference: &CredentialRef,
        budget: &SecurityBudget,
    ) -> Result<(CurrentCredentialStatus, String), SecurityError> {
        let mut transaction = self.repository.begin(budget).await?;
        let snapshot = transaction
            .credential(reference)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let (status, key) = status_for_snapshot(&snapshot, now(self.clock.as_ref())?)?;
        transaction.commit().await?;
        budget.remaining()?;
        status.validity.ensure_fresh(self.clock.as_ref())?;
        Ok((status, key))
    }
}

fn validate_session_access(
    row: &crate::entity::security_credential::Model,
    access: &crate::entity::security_credential::Model,
) -> Result<(), SecurityError> {
    if access.credential_kind != "access"
        || access.oidc_client_id != row.oidc_client_id
        || access.principal_kind != row.principal_kind
        || access.subject != row.subject
        || access.audience != row.audience
        || access.application != row.application
        || access.grant_id != row.grant_id
        || access.session_id != row.session_id
        || access.family_id != row.family_id
        || access.bounds != row.bounds
        || access.binding != row.binding
        || access.revocation_parent_jti.is_some()
    {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(())
}

fn validate_refresh_roots(state: &RegisteredState) -> Result<(), SecurityError> {
    let row = &state.credential;
    let session = state
        .session
        .as_ref()
        .ok_or(SecurityError::InvalidCredential)?;
    let family = state
        .family
        .as_ref()
        .ok_or(SecurityError::InvalidCredential)?;
    if row.credential_kind != "refresh"
        || row.binding
            != serde_json::to_value(IdentityBinding::None)
                .map_err(|_| SecurityError::InvalidCredential)?
        || state.grant.id != row.grant_id
        || state.grant.grant_kind != "session"
        || state.grant.issuer != row.issuer
        || state.grant.principal_kind != row.principal_kind
        || state.grant.subject != row.subject
        || state.grant.application != row.application
        || Some(session.id) != row.session_id
        || session.issuer != row.issuer
        || session.principal_kind != row.principal_kind
        || session.subject != row.subject
        || Some(family.id) != row.family_id
        || family.grant_id != row.grant_id
        || Some(family.session_id) != row.session_id
        || family.issuer != row.issuer
        || family.principal_kind != row.principal_kind
        || family.subject != row.subject
        || family.application != row.application
    {
        return Err(SecurityError::InvalidCredential);
    }
    PrincipalRef::new(&row.issuer, PrincipalKind::User, &row.subject)?;
    Ok(())
}

pub(super) fn status_for_snapshot(
    snapshot: &CredentialSnapshot,
    time: u64,
) -> Result<(CurrentCredentialStatus, String), SecurityError> {
    let claims = active_claims(&snapshot.current, time)?;
    let parent_exp = validate_parent(snapshot, &claims, time)?;
    let key = snapshot
        .current
        .credential
        .signing_key_id
        .clone()
        .ok_or(SecurityError::InvalidCredential)?;
    let validity = DecisionValidity::from_query_start(
        snapshot.checked_at,
        claims
            .exp
            .checked_mul(1000)
            .ok_or(SecurityError::InvalidCredential)?,
        parent_exp,
        None,
    )?;
    Ok((
        CurrentCredentialStatus {
            claims,
            active: true,
            validity,
        },
        key,
    ))
}
#[async_trait::async_trait]
impl CurrentCredentialChecker for CredentialLifecycle {
    async fn check_current(
        &self,
        verified: &VerifiedCredential,
        budget: &SecurityBudget,
    ) -> Result<CurrentCredentialStatus, SecurityError> {
        let (status, key) = self.load_active(verified.reference(), budget).await?;
        if &status.claims != verified.claims() || key != verified.signing_key_id() {
            return Err(SecurityError::InvalidCredential);
        }
        budget.remaining()?;
        status.validity.ensure_fresh(self.clock.as_ref())?;
        Ok(status)
    }
}

fn seconds(millis: i64) -> Result<u64, SecurityError> {
    let value = u64::try_from(millis).map_err(|_| SecurityError::DependencyUnavailable)?;
    if value % 1000 != 0 {
        return Err(SecurityError::DependencyUnavailable);
    }
    Ok(value / 1000)
}
fn identity_matches(
    issuer: &str,
    kind: &str,
    subject: &str,
    claims: &SecurityAccessClaims,
) -> bool {
    issuer == claims.iss
        && subject == claims.sub
        && match claims.principal_kind {
            PrincipalKind::User => kind == "user",
            PrincipalKind::Machine => kind == "machine",
            PrincipalKind::Device => kind == "device",
        }
}

fn active_claims(
    state: &RegisteredState,
    time: u64,
) -> Result<SecurityAccessClaims, SecurityError> {
    let row = &state.credential;
    if row.credential_kind != "access" {
        return Err(SecurityError::InvalidCredential);
    }
    let kind: PrincipalKind = decode(row.principal_kind.clone().into())?;
    let token_use: TokenUse = decode(row.token_use.clone().into())?;
    PrincipalRef::new(&row.issuer, kind, &row.subject)?;
    CredentialRef::new(&row.issuer, &row.jti)?;
    let claims = SecurityAccessClaims {
        iss: row.issuer.clone(),
        sub: row.subject.clone(),
        aud: row.audience.clone(),
        iat: seconds(row.issued_at)?,
        exp: seconds(row.expires_at)?,
        nbf: row.not_before.map(seconds).transpose()?,
        jti: row.jti.clone(),
        principal_kind: kind,
        token_use,
        application: row.application.clone(),
        grant_id: row.grant_id.to_string(),
        bounds: decode(row.bounds.clone())?,
        binding: decode(row.binding.clone())?,
        session_id: row.session_id.map(|v| v.to_string()),
        family_id: row.family_id.map(|v| v.to_string()),
    };
    if row.revoked_at.is_some()
        || row.consumed_at.is_some()
        || row.replacement_jti.is_some()
        || state.grant.revoked_at.is_some()
    {
        return Err(SecurityError::CredentialRevoked);
    }
    let latest = time
        .checked_add(CLOCK_ERROR_MILLIS)
        .ok_or(SecurityError::DependencyUnavailable)?;
    let earliest = time.saturating_sub(CLOCK_ERROR_MILLIS);
    if u64::try_from(row.expires_at).map_err(|_| SecurityError::DependencyUnavailable)? <= latest
        || u64::try_from(row.issued_at).map_err(|_| SecurityError::DependencyUnavailable)? > latest
        || row
            .not_before
            .is_some_and(|nbf| nbf < 0 || nbf as u64 > earliest)
    {
        return Err(SecurityError::InvalidCredential);
    }
    if !identity_matches(
        &state.grant.issuer,
        &state.grant.principal_kind,
        &state.grant.subject,
        &claims,
    ) || state.grant.application != claims.application
        || state.grant.id != row.grant_id
    {
        return Err(SecurityError::InvalidCredential);
    }
    match (row.session_id, &state.session) {
        (Some(id), Some(session))
            if session.id == id
                && identity_matches(
                    &session.issuer,
                    &session.principal_kind,
                    &session.subject,
                    &claims,
                ) =>
        {
            if session.revoked_at.is_some() {
                return Err(SecurityError::CredentialRevoked);
            }
        }
        (None, None) => {}
        _ => return Err(SecurityError::InvalidCredential),
    }
    match (row.family_id, &state.family) {
        (Some(id), Some(family))
            if family.id == id
                && family.grant_id == row.grant_id
                && Some(family.session_id) == row.session_id
                && family.application == row.application
                && identity_matches(
                    &family.issuer,
                    &family.principal_kind,
                    &family.subject,
                    &claims,
                ) =>
        {
            if family.revoked_at.is_some() {
                return Err(SecurityError::CredentialRevoked);
            }
        }
        (None, None) => {}
        _ => return Err(SecurityError::InvalidCredential),
    }
    if token_use == TokenUse::GatewayOperation {
        if !matches!(kind, PrincipalKind::User | PrincipalKind::Machine)
            || row.revocation_parent_jti.is_none()
        {
            return Err(SecurityError::InvalidCredential);
        }
        if let IdentityBinding::Gateway { gateway_id } = &claims.binding {
            ResourceRef::new(
                &claims.application,
                ResourceTarget::Gateway {
                    gateway_id: gateway_id.clone(),
                },
            )?;
            if claims.aud != format!("cobotlet:{gateway_id}") {
                return Err(SecurityError::InvalidCredential);
            }
        } else {
            return Err(SecurityError::InvalidCredential);
        }
        return Ok(claims);
    }
    CredentialProfile::new(kind, token_use)?;
    if row.revocation_parent_jti.is_some() {
        return Err(SecurityError::InvalidCredential);
    }
    let expected_root = match token_use {
        TokenUse::UserSession => "session",
        TokenUse::McpAccess => "mcp",
        TokenUse::SecurityService | TokenUse::PermissionSync => "service",
        TokenUse::DeviceAccess => "device",
        // The restricted browser ticket lifecycle is not yet wired. Never infer
        // missing session/parent security facts for an access-shaped stream token.
        TokenUse::StreamConnect | TokenUse::GatewayOperation => {
            return Err(SecurityError::InvalidCredential);
        }
    };
    if state.grant.grant_kind != expected_root
        || (token_use == TokenUse::UserSession && row.session_id.is_none())
        || (token_use != TokenUse::UserSession
            && (row.session_id.is_some() || row.family_id.is_some()))
    {
        return Err(SecurityError::InvalidCredential);
    }
    match kind {
        PrincipalKind::User => {
            if claims.binding != IdentityBinding::None {
                return Err(SecurityError::InvalidCredential);
            }
        }
        PrincipalKind::Machine => {
            let binding = state
                .service_binding
                .as_ref()
                .ok_or(SecurityError::CredentialRevoked)?;
            if binding.identity() != &claims.binding {
                return Err(SecurityError::InvalidCredential);
            }
        }
        PrincipalKind::Device => {
            let binding = state
                .device_binding
                .as_ref()
                .ok_or(SecurityError::CredentialRevoked)?;
            if claims.binding
                != (IdentityBinding::Device {
                    gateway_id: binding.gateway_id.clone(),
                    device_id: binding.device_id.clone(),
                })
                || claims.aud != format!("cobotlet:{}", binding.gateway_id)
            {
                return Err(SecurityError::InvalidCredential);
            }
        }
    }
    Ok(claims)
}

fn validate_parent(
    snapshot: &CredentialSnapshot,
    claims: &SecurityAccessClaims,
    time: u64,
) -> Result<Option<u64>, SecurityError> {
    let current = &snapshot.current.credential;
    if current.revocation_parent_jti.is_none() {
        if snapshot.parent.is_some() {
            return Err(SecurityError::InvalidCredential);
        }
        return Ok(None);
    }
    let parent = snapshot
        .parent
        .as_ref()
        .ok_or(SecurityError::InvalidCredential)?;
    if parent.credential.revocation_parent_jti.is_some()
        || parent.credential.token_use == "gateway_operation"
    {
        return Err(SecurityError::InvalidCredential);
    }
    let parent_claims = active_claims(parent, time)?;
    if claims.token_use != TokenUse::GatewayOperation
        || !matches!(
            parent_claims.token_use,
            TokenUse::UserSession | TokenUse::McpAccess | TokenUse::SecurityService
        )
        || current.revocation_parent_jti.as_deref() != Some(parent_claims.jti.as_str())
        || claims.iss != parent_claims.iss
        || claims.sub != parent_claims.sub
        || claims.principal_kind != parent_claims.principal_kind
        || claims.application != parent_claims.application
        || claims.grant_id != parent_claims.grant_id
        || claims.session_id != parent_claims.session_id
        || claims.family_id != parent_claims.family_id
        || claims.exp > parent_claims.exp
        || claims.iat < parent_claims.iat
        || claims.jti == parent_claims.jti
    {
        return Err(SecurityError::InvalidCredential);
    }
    if let IdentityBinding::Gateway { gateway_id } = &parent_claims.binding
        && claims.binding
            != (IdentityBinding::Gateway {
                gateway_id: gateway_id.clone(),
            })
    {
        return Err(SecurityError::InvalidCredential);
    }
    parent_claims
        .exp
        .checked_mul(1000)
        .map(Some)
        .ok_or(SecurityError::InvalidCredential)
}

async fn authenticate_session_client(
    client: &crate::entity::oauth_client::Model,
    secret: Option<&SecretToken>,
    grant: &str,
    budget: &SecurityBudget,
) -> Result<(), SecurityError> {
    use crate::service::oidc::OidcService;
    if !OidcService::client_allows_grant_type(client, grant) {
        return Err(SecurityError::InvalidCredential);
    }
    if client.public_client {
        if secret.is_some() {
            return Err(SecurityError::InvalidCredential);
        }
    } else {
        let secret = secret
            .ok_or(SecurityError::InvalidCredential)?
            .expose_secret()
            .to_owned();
        let hash = client.client_secret_hash.clone();
        let valid = budget
            .run(tokio::task::spawn_blocking(move || {
                bcrypt::verify(secret, &hash).unwrap_or(false)
            }))
            .await?
            .map_err(|_| SecurityError::DependencyUnavailable)?;
        if !valid {
            return Err(SecurityError::InvalidCredential);
        }
    }
    budget.remaining()?;
    Ok(())
}
fn expiry(at: u64, ttl: u64) -> Result<u64, SecurityError> {
    if ttl <= 1 {
        return Err(SecurityError::InvalidCredential);
    }
    at.checked_add(ttl)
        .filter(|v| v.checked_mul(1000).is_some_and(|m| m <= i64::MAX as u64))
        .ok_or(SecurityError::InvalidCredential)
}
fn validate_local_username(username: &str) -> Result<(), SecurityError> {
    if username.trim().is_empty()
        || username.chars().count() > 255
        || username.chars().any(char::is_control)
    {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(())
}
fn session_claims(
    config: &SessionTokenConfig,
    user_id: crate::IdType,
    audience: String,
    application: String,
    at: u64,
) -> Result<SecurityAccessClaims, SecurityError> {
    if user_id <= 0 {
        return Err(SecurityError::InvalidCredential);
    }
    PrincipalRef::new(&config.issuer, PrincipalKind::User, user_id.to_string())?;
    Ok(SecurityAccessClaims {
        iss: config.issuer.clone(),
        sub: user_id.to_string(),
        aud: audience,
        application,
        iat: at / 1000,
        exp: expiry(at / 1000, config.access_ttl_seconds)?,
        nbf: None,
        jti: uuid::Uuid::new_v4().to_string(),
        principal_kind: PrincipalKind::User,
        token_use: TokenUse::UserSession,
        grant_id: uuid::Uuid::new_v4().to_string(),
        session_id: Some(uuid::Uuid::new_v4().to_string()),
        family_id: Some(uuid::Uuid::new_v4().to_string()),
        binding: IdentityBinding::None,
        bounds: CredentialBounds::new(vec![]),
    })
}
struct SessionMint<'a> {
    claims: SecurityAccessClaims,
    client_id: Option<&'a str>,
    nonce: Option<&'a str>,
    minted_by: Option<CredentialRef>,
    new_roots: bool,
}
async fn mint_session_pair(
    mint: SessionMint<'_>,
    config: &SessionTokenConfig,
    signer: &dyn SessionTokenSigner,
    budget: &SecurityBudget,
) -> Result<(SessionTokenSet, SessionRegistration), SecurityError> {
    use rand::RngCore;
    let SessionMint {
        claims,
        client_id,
        nonce,
        minted_by,
        new_roots,
    } = mint;
    let refresh_exp = expiry(claims.iat, config.refresh_ttl_seconds)?;
    if claims.exp > refresh_exp || signer.key_id().is_empty() {
        return Err(SecurityError::InvalidCredential);
    }
    let mut random = [0; 32];
    rand::rngs::OsRng.fill_bytes(&mut random);
    let refresh_token = SecretToken::new(hex::encode(random))?;
    budget.remaining()?;
    let access_token = budget.run(signer.sign_access(&claims)).await??;
    budget.remaining()?;
    let id_token = match client_id {
        Some(client_id) => Some(
            budget
                .run(signer.sign_id(&claims, client_id, nonce, &access_token))
                .await??,
        ),
        None => None,
    };
    budget.remaining()?;
    let registration = SessionRegistration {
        client_id: client_id.map(str::to_owned),
        claims: claims.clone(),
        key_id: signer.key_id().into(),
        refresh_jti: uuid::Uuid::new_v4().to_string(),
        refresh_digest: refresh_digest(&refresh_token),
        refresh_expires_at: refresh_exp * 1000,
        minted_by,
        new_roots,
    };
    Ok((
        SessionTokenSet {
            access_token,
            id_token,
            refresh_token,
            claims,
        },
        registration,
    ))
}

#[async_trait::async_trait]
impl SessionTokenSigner for auth::token_manager::IdTokenSigner {
    fn key_id(&self) -> &str {
        &self.kid
    }
    async fn sign_access(
        &self,
        claims: &SecurityAccessClaims,
    ) -> Result<SecretToken, SecurityError> {
        let signer = self.clone();
        let claims = claims.clone();
        tokio::task::spawn_blocking(move || signer.sign_security_access(&claims))
            .await
            .map_err(|_| SecurityError::DependencyUnavailable)?
    }
    async fn sign_id(
        &self,
        claims: &SecurityAccessClaims,
        client_id: &str,
        nonce: Option<&str>,
        access: &SecretToken,
    ) -> Result<SecretToken, SecurityError> {
        if !self.is_rsa() {
            return Err(SecurityError::DependencyUnavailable);
        }
        let signer = self.clone();
        let claims = claims.clone();
        let client_id = client_id.to_owned();
        let nonce = nonce.map(str::to_owned);
        let access = SecretToken::new(access.expose_secret().to_owned())?;
        tokio::task::spawn_blocking(move || {
            SecretToken::new(
                signer
                    .sign_id_token(
                        &claims.sub,
                        &claims.iss,
                        Some(&client_id),
                        nonce.as_deref(),
                        Some(access.expose_secret()),
                    )
                    .map_err(|_| SecurityError::DependencyUnavailable)?,
            )
        })
        .await
        .map_err(|_| SecurityError::DependencyUnavailable)?
    }
}

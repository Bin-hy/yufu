//! Credential/root liveness. Signature proof and current resource policy remain
//! separate required steps; looking up an arbitrary reference is not authentication.
use std::sync::Arc;

use super::repository::{CredentialSnapshot, RegisteredState, SecurityRepository, decode, now};
use auth::security::{
    budget::{Clock, SecurityBudget},
    credential::{
        CLOCK_ERROR_MILLIS, CredentialProfile, CredentialRef, CurrentCredentialChecker,
        CurrentCredentialStatus, DecisionValidity, SecurityAccessClaims, VerifiedCredential,
    },
    error::SecurityError,
    identity::{IdentityBinding, PrincipalKind, PrincipalRef, TokenUse},
    policy::{ResourceRef, ResourceTarget},
};

pub struct CredentialLifecycle {
    repository: Arc<dyn SecurityRepository>,
    clock: Arc<dyn Clock>,
}
impl CredentialLifecycle {
    pub fn new(repository: Arc<dyn SecurityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
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
        let current_now = now(self.clock.as_ref())?;
        let claims = active_claims(&snapshot.current, current_now)?;
        let parent_exp = validate_parent(&snapshot, &claims, current_now)?;
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
        transaction.commit().await?;
        budget.remaining()?;
        validity.ensure_fresh(self.clock.as_ref())?;
        Ok((
            CurrentCredentialStatus {
                claims,
                active: true,
                validity,
            },
            key,
        ))
    }
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

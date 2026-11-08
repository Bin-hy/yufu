//! MCP management uses an interactive IdP credential and an independent grant.
use super::lifecycle::{CredentialLifecycle, SessionTokenSigner, status_for_snapshot};
use super::repository::{McpRegistration, SecurityTransaction, now};
use super::revocation::StoredRevocationScope;
use auth::security::{
    budget::SecurityBudget,
    credential::{AuthenticatedContext, SecurityAccessClaims},
    error::SecurityError,
    identity::{IdentityBinding, PrincipalKind, SecretToken, TokenUse},
    policy::{CredentialBounds, OperationId, ResourceRef, ResourceTarget},
};

pub struct McpTokenConfig {
    pub issuer: String,
    pub max_ttl_days: u32,
}
pub struct McpIssueRequest<'a> {
    pub application: &'a str,
    pub name: &'a str,
    pub ttl_days: Option<u32>,
}
pub struct McpRotationRequest {
    pub id: crate::IdType,
    pub ttl_days: Option<u32>,
}
#[derive(Debug)]
pub struct McpTokenSet {
    pub access_token: SecretToken,
    pub claims: SecurityAccessClaims,
    pub metadata: crate::entity::mcp_token::Model,
}
impl CredentialLifecycle {
    pub async fn list_mcp_tokens(
        &self,
        manager: &AuthenticatedContext,
        budget: &SecurityBudget,
    ) -> Result<Vec<crate::entity::mcp_token::McpTokenVO>, SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        let mut txn = self.repository.begin(&budget).await?;
        self.check_manager(txn.as_mut(), manager).await?;
        self.authorize_revocation_uri(txn.as_mut(), manager, "GET", "/oidc/mcp-tokens")
            .await?;
        let rows = txn.list_mcp(manager.principal()).await?;
        manager.ensure_current()?;
        txn.commit().await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
    pub async fn rename_mcp_token(
        &self,
        manager: &AuthenticatedContext,
        id: crate::IdType,
        name: &str,
        budget: &SecurityBudget,
    ) -> Result<(), SecurityError> {
        validate_name(name)?;
        let budget = manager.budget().child(budget.remaining()?)?;
        let mut txn = self.repository.begin(&budget).await?;
        self.check_manager(txn.as_mut(), manager).await?;
        self.authorize_revocation_uri(txn.as_mut(), manager, "PATCH", "/oidc/mcp-tokens/{id}")
            .await?;
        self.owned_mcp_reference(txn.as_mut(), manager, id).await?;
        txn.rename_mcp(manager.principal(), id, name).await?;
        manager.ensure_current()?;
        txn.commit().await
    }
    pub async fn delete_mcp_token(
        &self,
        manager: &AuthenticatedContext,
        id: crate::IdType,
        budget: &SecurityBudget,
    ) -> Result<(), SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        let mut txn = self.repository.begin(&budget).await?;
        self.check_manager(txn.as_mut(), manager).await?;
        self.authorize_revocation_uri(txn.as_mut(), manager, "DELETE", "/oidc/mcp-tokens/{id}")
            .await?;
        let reference = self.owned_mcp_reference(txn.as_mut(), manager, id).await?;
        txn.revoke_scope(
            manager.principal(),
            &StoredRevocationScope::Credential(reference),
            now(self.clock.as_ref())?,
        )
        .await?;
        manager.ensure_current()?;
        txn.commit().await
    }
    async fn owned_mcp_reference(
        &self,
        txn: &mut dyn SecurityTransaction,
        manager: &AuthenticatedContext,
        id: crate::IdType,
    ) -> Result<auth::security::credential::CredentialRef, SecurityError> {
        let row = txn
            .mcp_metadata(manager.principal(), id)
            .await?
            .ok_or(SecurityError::Forbidden)?;
        let reference =
            auth::security::credential::CredentialRef::new(manager.principal().issuer(), &row.jti)?;
        let snapshot = txn
            .lock_credential(&reference)
            .await?
            .ok_or(SecurityError::Forbidden)?;
        let c = &snapshot.current.credential;
        if c.principal_kind != "user"
            || c.subject != manager.principal().subject()
            || c.token_use != "mcp_access"
            || c.credential_kind != "access"
            || c.application != row.aud
            || c.audience != row.aud
            || snapshot.current.grant.grant_kind != "mcp"
            || snapshot.current.grant.issuer != manager.principal().issuer()
            || snapshot.current.grant.subject != manager.principal().subject()
            || snapshot.current.grant.application != row.aud
            || c.session_id.is_some()
            || c.family_id.is_some()
            || c.revocation_parent_jti.is_some()
            || c.expires_at != row.expires_at.and_utc().timestamp_millis()
        {
            return Err(SecurityError::Forbidden);
        }
        Ok(reference)
    }
    pub async fn issue_mcp(
        &self,
        manager: &AuthenticatedContext,
        request: McpIssueRequest<'_>,
        config: &McpTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<McpTokenSet, SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        validate_target(manager, request.application, config)?;
        validate_name(request.name)?;
        let mut txn = self.repository.begin(&budget).await?;
        self.check_manager(txn.as_mut(), manager).await?;
        self.authorize_revocation_uri(txn.as_mut(), manager, "POST", "/oidc/mcp-tokens")
            .await?;
        let bounds = self
            .mcp_bounds(txn.as_mut(), manager, request.application)
            .await?;
        let claims = mcp_claims(
            manager,
            request.application,
            bounds,
            config,
            request.ttl_days,
            now(self.clock.as_ref())?,
        )?;
        let access_token = budget.run(signer.sign_access(&claims)).await??;
        txn.prepare_mcp_creation(manager.principal()).await?;
        let metadata = txn
            .register_mcp(McpRegistration {
                claims: claims.clone(),
                key_id: signer.key_id().into(),
                name: request.name.into(),
                minted_by: manager.credential().clone(),
                new_root: true,
            })
            .await?;
        manager.ensure_current()?;
        budget.remaining()?;
        txn.commit().await?;
        Ok(McpTokenSet {
            access_token,
            claims,
            metadata,
        })
    }
    pub async fn rotate_mcp(
        &self,
        manager: &AuthenticatedContext,
        request: McpRotationRequest,
        config: &McpTokenConfig,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<McpTokenSet, SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        let mut txn = self.repository.begin(&budget).await?;
        self.check_manager(txn.as_mut(), manager).await?;
        self.authorize_revocation_uri(
            txn.as_mut(),
            manager,
            "POST",
            "/oidc/mcp-tokens/{id}/refresh",
        )
        .await?;
        let row = txn
            .mcp_metadata(manager.principal(), request.id)
            .await?
            .ok_or(SecurityError::Forbidden)?;
        validate_target(manager, &row.aud, config)?;
        if row.revoked {
            return Err(SecurityError::Conflict);
        }
        let reference =
            auth::security::credential::CredentialRef::new(manager.principal().issuer(), &row.jti)?;
        let snapshot = txn
            .lock_credential(&reference)
            .await?
            .ok_or(SecurityError::Forbidden)?;
        let old = &snapshot.current.credential;
        if old.consumed_at.is_some() || old.replacement_jti.is_some() {
            return Err(SecurityError::Conflict);
        }
        let (status, _) = status_for_snapshot(&snapshot, now(self.clock.as_ref())?)?;
        if status.claims.principal_kind != PrincipalKind::User
            || status.claims.sub != manager.principal().subject()
            || status.claims.token_use != TokenUse::McpAccess
            || status.claims.application != row.aud
            || old.expires_at != row.expires_at.and_utc().timestamp_millis()
        {
            return Err(SecurityError::Forbidden);
        }
        let bounds = status
            .claims
            .bounds
            .intersection(&self.mcp_bounds(txn.as_mut(), manager, &row.aud).await?);
        require_mcp_access(&bounds, &row.aud)?;
        let mut claims = mcp_claims(
            manager,
            &row.aud,
            bounds,
            config,
            request.ttl_days,
            now(self.clock.as_ref())?,
        )?;
        claims.grant_id = status.claims.grant_id;
        let access_token = budget.run(signer.sign_access(&claims)).await??;
        let at = now(self.clock.as_ref())?;
        txn.consume_mcp(&reference, at).await?;
        txn.revoke_scope(
            manager.principal(),
            &StoredRevocationScope::Credential(reference),
            at,
        )
        .await?;
        let metadata = txn
            .register_mcp(McpRegistration {
                claims: claims.clone(),
                key_id: signer.key_id().into(),
                name: row.name,
                minted_by: manager.credential().clone(),
                new_root: false,
            })
            .await?;
        manager.ensure_current()?;
        budget.remaining()?;
        txn.commit().await?;
        Ok(McpTokenSet {
            access_token,
            claims,
            metadata,
        })
    }
    async fn mcp_bounds(
        &self,
        txn: &mut dyn SecurityTransaction,
        manager: &AuthenticatedContext,
        application: &str,
    ) -> Result<CredentialBounds, SecurityError> {
        let current = txn
            .application_bounds(manager.verified().claims(), application)
            .await?;
        let bounds = manager.verified().claims().bounds.intersection(&current);
        require_mcp_access(&bounds, application)?;
        Ok(bounds)
    }
}

fn validate_target(
    manager: &AuthenticatedContext,
    application: &str,
    config: &McpTokenConfig,
) -> Result<(), SecurityError> {
    manager.ensure_current()?;
    if application != "cobotsrv" || config.issuer != manager.principal().issuer() {
        return Err(SecurityError::Forbidden);
    }
    Ok(())
}
fn validate_name(name: &str) -> Result<(), SecurityError> {
    if name.trim().is_empty() || name.chars().count() > 255 || name.chars().any(char::is_control) {
        return Err(SecurityError::InvalidResource);
    }
    Ok(())
}
fn require_mcp_access(bounds: &CredentialBounds, application: &str) -> Result<(), SecurityError> {
    if !bounds.allows(
        &OperationId::new("mcp:access")?,
        &ResourceRef::new(application, ResourceTarget::Application)?,
    ) {
        return Err(SecurityError::Forbidden);
    }
    Ok(())
}
fn mcp_claims(
    manager: &AuthenticatedContext,
    application: &str,
    bounds: CredentialBounds,
    config: &McpTokenConfig,
    ttl_days: Option<u32>,
    at: u64,
) -> Result<SecurityAccessClaims, SecurityError> {
    if config.max_ttl_days == 0 {
        return Err(SecurityError::DependencyUnavailable);
    }
    let ttl = u64::from(ttl_days.unwrap_or(30).clamp(1, config.max_ttl_days)) * 86_400;
    let iat = at / 1000;
    Ok(SecurityAccessClaims {
        iss: config.issuer.clone(),
        sub: manager.principal().subject().into(),
        aud: application.into(),
        application: application.into(),
        jti: uuid::Uuid::new_v4().to_string(),
        principal_kind: PrincipalKind::User,
        token_use: TokenUse::McpAccess,
        grant_id: uuid::Uuid::new_v4().to_string(),
        session_id: None,
        family_id: None,
        binding: IdentityBinding::None,
        bounds,
        iat,
        exp: iat
            .checked_add(ttl)
            .ok_or(SecurityError::InvalidCredential)?,
        nbf: None,
    })
}

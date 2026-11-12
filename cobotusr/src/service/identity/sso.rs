//! Center authentication never uses a management refresh token to restore a browser.
use super::{
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::config::Config;
use crate::domain::identity::{
    account::{IssuedSecret, SecretDigest, VerifiedActor, ensure_enabled, stable_principal},
    error::IdentityError,
    session::{BrowserSessionSnapshot, FlowPurpose, IdentityRevocationScope},
};
use crate::service::security::repository::now;
use auth::{
    claims::IdentityProfile,
    security::budget::{Clock, SecurityBudget},
};
use std::sync::Arc;

pub struct SsoService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
    config: Config,
}

pub struct BrowserAuthentication {
    pub actor: VerifiedActor,
    pub root: BrowserSessionSnapshot,
    pub profile: IdentityProfile,
}

pub struct PasswordLogin {
    pub authentication: BrowserAuthentication,
    pub cookie_secret: IssuedSecret,
}

pub struct LoginFlowProof {
    pub id: uuid::Uuid,
    pub proof: auth::security::identity::SecretToken,
}

impl SsoService {
    pub fn new(
        repository: Arc<dyn IdentityRepository>,
        clock: Arc<dyn Clock>,
        config: Config,
    ) -> Result<Self, IdentityError> {
        config
            .validate_identity_policy()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(Self {
            repository,
            clock,
            config,
        })
    }

    pub async fn authenticate_browser(
        &self,
        cookie_secret: &str,
        budget: &SecurityBudget,
    ) -> Result<BrowserAuthentication, IdentityError> {
        validate_opaque(cookie_secret)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &RestoreBrowser {
                digest: SecretDigest::of(cookie_secret),
                issuer: &self.config.issuer(),
                clock: self.clock.as_ref(),
            },
        )
        .await
    }

    pub async fn authenticate_password(
        &self,
        username: &str,
        password: &str,
        evidence: &super::browser::BrowserWriteEvidence,
        flow: Option<&LoginFlowProof>,
        request_id: uuid::Uuid,
        budget: &SecurityBudget,
    ) -> Result<PasswordLogin, IdentityError> {
        let verified = self.verify_password(username, password, budget).await?;
        // Bcrypt is local CPU work and may outlive the request budget under
        // load. The write still gets a fresh, checked two-second deadline.
        let write_budget = budget.renewed()?;
        execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &AuthenticatePassword {
                username,
                verified: &verified,
                evidence,
                flow,
                request_id,
                authenticated_at: now(self.clock.as_ref())?,
                clock: self.clock.as_ref(),
                issuer: &self.config.issuer(),
                ttl_seconds: self.config.identity.sso_session_ttl_seconds,
                entropy: rand::random(),
                root_id: uuid::Uuid::new_v4(),
            },
        )
        .await
    }

    /// Verification is separated from the subsequent locked write. That write
    /// must compare both the password hash and auth_epoch with this evidence.
    pub(crate) async fn verify_password(
        &self,
        username: &str,
        password: &str,
        budget: &SecurityBudget,
    ) -> Result<PasswordAccount, IdentityError> {
        if username.is_empty() || username.len() > 128 || password.is_empty() || password.len() > 72
        {
            return Err(IdentityError::Unauthenticated);
        }
        let mut transaction = self.repository.begin(budget).await?;
        let result = transaction.load_password_account(username).await;
        transaction.rollback().await?;
        let account = result?.ok_or(IdentityError::Unauthenticated)?;
        ensure_enabled(&account.account)?;
        let password = password.to_owned();
        let password_hash = account.password_hash.expose_secret().to_owned();
        let valid = super::password::verify(password, password_hash)
            .await
            .map_err(|_| IdentityError::Unauthenticated)?;
        if !valid {
            return Err(IdentityError::Unauthenticated);
        }
        Ok(account)
    }
}

struct AuthenticatePassword<'a> {
    username: &'a str,
    verified: &'a PasswordAccount,
    evidence: &'a super::browser::BrowserWriteEvidence,
    flow: Option<&'a LoginFlowProof>,
    request_id: uuid::Uuid,
    authenticated_at: u64,
    clock: &'a dyn Clock,
    issuer: &'a str,
    ttl_seconds: u64,
    entropy: [u8; 32],
    root_id: uuid::Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for AuthenticatePassword<'_> {
    type Output = PasswordLogin;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<PasswordLogin, IdentityError> {
        let old_root = match &self.evidence.current_center_digest {
            Some(digest) => transaction.load_browser_session(digest).await?,
            None => None,
        };
        let revocation_scope =
            old_root
                .as_ref()
                .map(|root| IdentityRevocationScope::CurrentCenter {
                    sso_session_id: root.id,
                });
        let revocation = match &revocation_scope {
            Some(scope) => Some(transaction.revocation_plan(scope).await?),
            None => None,
        };
        let mut keys = revocation
            .as_ref()
            .map(|plan| plan.locks.keys().to_vec())
            .unwrap_or_default();
        keys.push(IdentityLockKey::User(self.verified.account.user_id));
        if let Some(flow) = self.flow {
            keys.push(IdentityLockKey::Flow(flow.id));
        }
        transaction
            .lock_identity_scope(&IdentityLockScope::new(keys)?)
            .await?;
        let current = transaction
            .load_password_account(self.username)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        ensure_enabled(&current.account)?;
        if current.account.user_id != self.verified.account.user_id
            || current.account.auth_epoch != self.verified.account.auth_epoch
            || current.password_hash.expose_secret() != self.verified.password_hash.expose_secret()
        {
            return Err(IdentityError::Unauthenticated);
        }
        if let Some(flow) = self.flow {
            let current_flow = transaction
                .load_authorization_flow(flow.id)
                .await?
                .ok_or(IdentityError::NotFound)?;
            current_flow.handle.verify(
                FlowPurpose::Authorization,
                flow.proof.expose_secret(),
                now(self.clock)?,
            )?;
            if current_flow.browser_binding_digest != self.evidence.browser_binding_digest {
                return Err(IdentityError::Forbidden);
            }
            if current_flow.reauth_completed_at.is_some() {
                return Err(IdentityError::AlreadyConsumed);
            }
        }
        if let (Some(scope), Some(plan), Some(root)) = (&revocation_scope, &revocation, &old_root) {
            let current_plan = transaction.revocation_plan(scope).await?;
            if current_plan.locks.keys() != plan.locks.keys()
                || current_plan.rp_session_ids != plan.rp_session_ids
            {
                return Err(IdentityError::Conflict);
            }
            if root.principal.issuer() != self.issuer {
                return Err(IdentityError::Unauthenticated);
            }
            transaction
                .apply(IdentityWrite::Revoke(scope.clone()))
                .await?;
            let event_id = uuid::Uuid::new_v4();
            transaction
                .append_audit(&AuditEvent {
                    id: event_id,
                    request_id: self.request_id,
                    actor_id: Some(current.account.user_id),
                    action: "session.reauthenticate",
                    object_kind: "session",
                    object_id: root.id.to_string(),
                    result: AuditResult::Success,
                    reason: None,
                    change_summary: serde_json::json!({"affected_count":plan.rp_session_ids.len()}),
                    operation_id: None,
                    commit_marker: Some(event_id),
                })
                .await?;
            transaction
                .enqueue_logout(&LogoutEvent {
                    audit_event_id: event_id,
                    rp_session_ids: plan.rp_session_ids.clone(),
                })
                .await?;
        }
        let cookie_secret = IssuedSecret::from_entropy(self.entropy);
        let root = BrowserSessionSnapshot {
            id: self.root_id,
            principal: stable_principal(self.issuer, current.account.user_id)?,
            user_id: current.account.user_id,
            secret_digest: cookie_secret.digest(),
            auth_epoch: current.account.auth_epoch,
            authenticated_at: self.authenticated_at,
            expires_at: self
                .authenticated_at
                .checked_add(self.ttl_seconds * 1000)
                .ok_or(IdentityError::CapacityExceeded)?,
            revoked_at: None,
        };
        transaction
            .apply(IdentityWrite::CreateCenter(root.clone()))
            .await?;
        if let Some(flow) = self.flow {
            transaction
                .apply(IdentityWrite::BindFlow {
                    flow_id: flow.id,
                    sso_session_id: root.id,
                    reauthenticated: true,
                })
                .await?;
        }
        let event_id = uuid::Uuid::new_v4();
        transaction
            .append_audit(&AuditEvent {
                id: event_id,
                request_id: self.request_id,
                actor_id: Some(current.account.user_id),
                action: "session.login",
                object_kind: "session",
                object_id: root.id.to_string(),
                result: AuditResult::Success,
                reason: None,
                change_summary: serde_json::json!({"user_id":current.account.user_id}),
                operation_id: None,
                commit_marker: Some(event_id),
            })
            .await?;
        let profile = transaction
            .load_profile(current.account.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        Ok(PasswordLogin {
            authentication: BrowserAuthentication {
                actor: VerifiedActor::verified(current.account, self.issuer, root.id)?,
                root,
                profile,
            },
            cookie_secret,
        })
    }
}

struct RestoreBrowser<'a> {
    digest: SecretDigest,
    issuer: &'a str,
    clock: &'a dyn Clock,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for RestoreBrowser<'_> {
    type Output = BrowserAuthentication;
    async fn apply(
        &self,
        transaction: &mut dyn super::repository::IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let root = transaction
            .load_browser_session(&self.digest)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        if root.principal.issuer() != self.issuer {
            return Err(IdentityError::Unauthenticated);
        }
        let account = transaction
            .load_account(root.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        root.validate(&account, now(self.clock)?)?;
        let profile = transaction
            .load_profile(root.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        Ok(BrowserAuthentication {
            actor: VerifiedActor::verified(account, self.issuer, root.id)?,
            root,
            profile,
        })
    }
}

pub(crate) fn validate_opaque(secret: &str) -> Result<(), IdentityError> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let bytes = URL_SAFE_NO_PAD
        .decode(secret)
        .map_err(|_| IdentityError::Unauthenticated)?;
    if secret.len() != 43 || bytes.len() != 32 || URL_SAFE_NO_PAD.encode(bytes) != secret {
        return Err(IdentityError::Unauthenticated);
    }
    Ok(())
}

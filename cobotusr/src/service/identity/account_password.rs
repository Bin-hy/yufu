use super::actions::{
    action_flow, audit, continuation, current_digest, hash_password, secret_digest,
};
use super::{AccountContinuation, AccountService, ActionProof, authenticate_current_actor};
use crate::domain::identity::{
    account::{AccountPassword, SecretDigest},
    error::IdentityError,
    session::{ActionTarget, IdentityRevocationScope},
};
use crate::service::identity::{
    browser::BrowserWriteEvidence,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::service::security::repository::now;
use auth::security::{budget::SecurityBudget, identity::SecretToken};
use uuid::Uuid;

impl AccountService {
    pub async fn change_password(
        &self,
        evidence: &BrowserWriteEvidence,
        old_password: &AccountPassword,
        new_password: &AccountPassword,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AccountContinuation, IdentityError> {
        let digest = current_digest(evidence)?;
        let mut tx = self.repository.begin(budget).await?;
        let root = tx
            .load_browser_session(digest)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let account = tx
            .load_account(root.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        root.validate(&account, now(self.clock.as_ref())?)?;
        let username = tx
            .load_profile(account.user_id)
            .await?
            .and_then(|profile| profile.preferred_username)
            .ok_or(IdentityError::Unauthenticated)?;
        tx.rollback().await?;
        let verified = self
            .sso()?
            .verify_password(&username, old_password.expose_secret(), budget)
            .await?;
        let hash = hash_password(new_password, budget).await?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &PasswordMutation {
                service: self,
                evidence,
                proof: PasswordEvidence::Current {
                    username: &username,
                    verified: &verified,
                },
                hash: &hash,
                request_id,
            },
        )
        .await
    }
    pub async fn reset_password(
        &self,
        evidence: &BrowserWriteEvidence,
        action: ActionProof<'_>,
        new_password: &AccountPassword,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AccountContinuation, IdentityError> {
        crate::service::identity::sso::validate_opaque(action.proof.expose_secret())?;
        let hash = hash_password(new_password, budget).await?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &PasswordMutation {
                service: self,
                evidence,
                proof: PasswordEvidence::Reset {
                    id: action.action_id,
                    digest: secret_digest(action.proof),
                },
                hash: &hash,
                request_id,
            },
        )
        .await
    }
}
enum PasswordEvidence<'a> {
    Current {
        username: &'a str,
        verified: &'a PasswordAccount,
    },
    Reset {
        id: Uuid,
        digest: SecretDigest,
    },
}
struct PasswordMutation<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    proof: PasswordEvidence<'a>,
    hash: &'a SecretToken,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for PasswordMutation<'_> {
    type Output = AccountContinuation;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<AccountContinuation, IdentityError> {
        let initial_action = match &self.proof {
            PasswordEvidence::Current { .. } => None,
            PasswordEvidence::Reset { id, digest } => Some(
                action_flow(
                    transaction,
                    *id,
                    digest,
                    self.evidence,
                    now(self.service.clock.as_ref())?,
                )
                .await?,
            ),
        };
        let user_id = match (&self.proof, &initial_action) {
            (PasswordEvidence::Current { verified, .. }, _) => verified.account.user_id,
            (PasswordEvidence::Reset { .. }, Some(flow)) => {
                let ActionTarget::PasswordReset(reset_id) = flow.target else {
                    return Err(IdentityError::Forbidden);
                };
                transaction
                    .load_reset(reset_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?
                    .user_id
            }
            _ => return Err(IdentityError::Unauthenticated),
        };
        let scope = IdentityRevocationScope::UserAll { user_id };
        let plan = transaction.revocation_plan(&scope).await?;
        let mut keys = plan.locks.keys().to_vec();
        if let Some(flow) = &initial_action {
            keys.push(IdentityLockKey::Action(flow.id));
        }
        let locks = IdentityLockScope::new(keys)?;
        let (actor_id, oidc_flow_id) = match &self.proof {
            PasswordEvidence::Current { username, verified } => {
                let actor = authenticate_current_actor(
                    transaction,
                    current_digest(self.evidence)?,
                    now(self.service.clock.as_ref())?,
                    &locks,
                )
                .await?;
                let current = transaction
                    .load_password_account(username)
                    .await?
                    .ok_or(IdentityError::Unauthenticated)?;
                if actor.account().user_id != verified.account.user_id
                    || current.account != verified.account
                    || current.password_hash.expose_secret()
                        != verified.password_hash.expose_secret()
                {
                    return Err(IdentityError::Unauthenticated);
                }
                (Some(actor.account().user_id), None)
            }
            PasswordEvidence::Reset { id, digest } => {
                transaction.lock_identity_scope(&locks).await?;
                let flow = action_flow(
                    transaction,
                    *id,
                    digest,
                    self.evidence,
                    now(self.service.clock.as_ref())?,
                )
                .await?;
                let ActionTarget::PasswordReset(reset_id) = flow.target else {
                    return Err(IdentityError::Forbidden);
                };
                let reset = transaction
                    .load_reset(reset_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                let account = transaction
                    .load_account(user_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                reset.validate(&account, now(self.service.clock.as_ref())?)?;
                transaction
                    .apply(IdentityWrite::ConsumeReset {
                        reset_id,
                        expected_auth_epoch: account.auth_epoch,
                    })
                    .await?;
                transaction
                    .apply(IdentityWrite::ConsumeActionFlow {
                        id: *id,
                        target: flow.target,
                        browser_binding_digest: self.evidence.browser_binding_digest.clone(),
                        browser_proof_digest: digest.clone(),
                    })
                    .await?;
                (None, continuation(&flow)?)
            }
        };
        let current_plan = transaction.revocation_plan(&scope).await?;
        if current_plan.locks.keys() != plan.locks.keys() {
            return Err(IdentityError::Conflict);
        }
        let account = transaction
            .load_account(user_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        transaction
            .apply(IdentityWrite::ChangePassword {
                user_id,
                password_hash: SecretToken::new(self.hash.expose_secret().to_owned())?,
                expected_auth_epoch: account.auth_epoch,
            })
            .await?;
        transaction
            .apply(IdentityWrite::RevokeAccountSessions { user_id })
            .await?;
        let event_id = audit(transaction, actor_id, self.request_id, if matches!(self.proof, PasswordEvidence::Reset { .. }) { "account.password.reset" } else { "account.password.change" }, "account", user_id.to_string(), serde_json::json!({"auth_epoch":account.auth_epoch+1,"affected_count":current_plan.rp_session_ids.len()})).await?;
        transaction
            .enqueue_logout(&LogoutEvent {
                audit_event_id: event_id,
                rp_session_ids: current_plan.rp_session_ids,
            })
            .await?;
        Ok(AccountContinuation {
            user_id,
            oidc_flow_id,
            requires_login: true,
        })
    }
}

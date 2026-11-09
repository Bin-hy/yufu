use super::{
    AccountService,
    actions::{audit, current_digest},
    authenticate_current_actor,
};
use crate::domain::identity::{
    account::validate_account_change, error::IdentityError, operation::IdentityOperation,
    session::IdentityRevocationScope,
};
use crate::service::identity::{
    browser::BrowserWriteEvidence,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::service::security::repository::now;
use auth::security::budget::SecurityBudget;
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct AccountChangeResult {
    pub user_id: i32,
    pub enabled: bool,
    pub developer_enabled: bool,
    pub revision: u64,
    pub auth_epoch: u64,
    pub revoked_rp_count: usize,
    pub requires_login: bool,
}
enum Attribute {
    Enabled(bool),
    Developer(bool),
}
impl AccountService {
    pub async fn set_enabled(
        &self,
        evidence: &BrowserWriteEvidence,
        user_id: i32,
        enabled: bool,
        expected_revision: u64,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AccountChangeResult, IdentityError> {
        self.change_attribute(
            evidence,
            user_id,
            Attribute::Enabled(enabled),
            expected_revision,
            request_id,
            budget,
        )
        .await
    }
    pub async fn set_developer(
        &self,
        evidence: &BrowserWriteEvidence,
        user_id: i32,
        developer_enabled: bool,
        expected_revision: u64,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AccountChangeResult, IdentityError> {
        self.change_attribute(
            evidence,
            user_id,
            Attribute::Developer(developer_enabled),
            expected_revision,
            request_id,
            budget,
        )
        .await
    }
    async fn change_attribute(
        &self,
        evidence: &BrowserWriteEvidence,
        user_id: i32,
        attribute: Attribute,
        expected_revision: u64,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AccountChangeResult, IdentityError> {
        if user_id <= 0 || expected_revision == u64::MAX {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &AccountMutation {
                service: self,
                evidence,
                user_id,
                attribute,
                expected_revision,
                request_id,
            },
        )
        .await
    }
}
struct AccountMutation<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    user_id: i32,
    attribute: Attribute,
    expected_revision: u64,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for AccountMutation<'_> {
    type Output = AccountChangeResult;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let digest = current_digest(self.evidence)?;
        let root = transaction
            .load_browser_session(digest)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let account = transaction
            .load_account(root.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        root.validate(&account, now(self.service.clock.as_ref())?)?;
        let preflight = crate::domain::identity::account::VerifiedActor::verified(
            account,
            root.principal.issuer(),
            root.id,
        )?;
        let operation = match self.attribute {
            Attribute::Enabled(_) => IdentityOperation::AccountEnable,
            Attribute::Developer(_) => IdentityOperation::AccountDeveloper,
        };
        transaction
            .authorize_identity_operation(&preflight, operation)
            .await?;
        let scope = IdentityRevocationScope::UserAll {
            user_id: self.user_id,
        };
        let initial = transaction.account_governance(self.user_id).await?;
        let plan = transaction.revocation_plan(&scope).await?;
        let mut keys = initial.locks.keys().to_vec();
        keys.extend_from_slice(plan.locks.keys());
        let actor = authenticate_current_actor(
            transaction,
            digest,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(keys)?,
        )
        .await?;
        transaction
            .authorize_identity_operation(&actor, operation)
            .await?;
        let current = transaction.account_governance(self.user_id).await?;
        let current_plan = transaction.revocation_plan(&scope).await?;
        if current.revision != self.expected_revision
            || current.locks.keys() != initial.locks.keys()
            || current_plan.locks.keys() != plan.locks.keys()
        {
            return Err(IdentityError::Conflict);
        }
        let mut proposed = current.account.clone();
        match self.attribute {
            Attribute::Enabled(value) => proposed.enabled = value,
            Attribute::Developer(value) => proposed.developer_enabled = value,
        }
        validate_account_change(
            &current.account,
            &proposed,
            current.enabled_administrator_ids.len(),
            &current.active_owned_modes,
        )?;
        let disabling = current.account.enabled && !proposed.enabled;
        let IdentityWriteResult::Account(updated) = transaction
            .apply(IdentityWrite::SetAccount {
                account: proposed,
                expected_revision: self.expected_revision,
            })
            .await?
        else {
            return Err(IdentityError::DependencyUnavailable);
        };
        if disabling {
            transaction
                .apply(IdentityWrite::RevokeAccountSessions {
                    user_id: self.user_id,
                })
                .await?;
        }
        for &environment_id in &current.affected_environments {
            transaction
                .apply(IdentityWrite::RecomputeGeneration {
                    environment_id,
                    user_id: self.user_id,
                })
                .await?;
        }
        let remaining = transaction.account_governance(self.user_id).await?;
        let revoked: Vec<_> = current
            .live_rp_session_ids
            .iter()
            .copied()
            .filter(|id| !remaining.live_rp_session_ids.contains(id))
            .collect();
        let event_id = audit(transaction,Some(actor.account().user_id),self.request_id,match self.attribute { Attribute::Enabled(_) => "account.enabled.change",Attribute::Developer(_) => "account.developer.change" },"account",self.user_id.to_string(),serde_json::json!({"enabled":updated.enabled,"developer_enabled":updated.developer_enabled,"auth_epoch":updated.auth_epoch,"revision":remaining.revision,"affected_count":revoked.len()})).await?;
        transaction
            .enqueue_logout(&LogoutEvent {
                audit_event_id: event_id,
                rp_session_ids: revoked.clone(),
            })
            .await?;
        Ok(AccountChangeResult {
            user_id: self.user_id,
            enabled: updated.enabled,
            developer_enabled: updated.developer_enabled,
            revision: remaining.revision,
            auth_epoch: updated.auth_epoch,
            revoked_rp_count: revoked.len(),
            requires_login: disabling && actor.account().user_id == self.user_id,
        })
    }
}

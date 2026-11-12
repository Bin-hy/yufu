use super::{
    account::authenticate_current_actor,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::domain::identity::{
    account::SecretDigest, error::IdentityError, session::IdentityRevocationScope,
};
use crate::service::security::repository::now;
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;

pub struct SessionService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}
impl SessionService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    /// Personal scope is limited to the current root, the actor's own roots,
    /// or one of their RP sessions/grants. Enterprise scopes use separate admin operations.
    pub async fn revoke_personal(
        &self,
        browser_secret: &str,
        scope: IdentityRevocationScope,
        request_id: uuid::Uuid,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        super::sso::validate_opaque(browser_secret)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &PersonalRevocation {
                digest: SecretDigest::of(browser_secret),
                scope,
                request_id,
                clock: self.clock.as_ref(),
            },
        )
        .await
    }
}

struct PersonalRevocation<'a> {
    digest: SecretDigest,
    scope: IdentityRevocationScope,
    request_id: uuid::Uuid,
    clock: &'a dyn Clock,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for PersonalRevocation<'_> {
    type Output = ();
    async fn apply(&self, transaction: &mut dyn IdentityTransaction) -> Result<(), IdentityError> {
        let plan = transaction.revocation_plan(&self.scope).await?;
        let actor =
            authenticate_current_actor(transaction, &self.digest, now(self.clock)?, &plan.locks)
                .await?;
        let user_id = actor.account().user_id;
        let allowed = match self.scope {
            IdentityRevocationScope::CurrentCenter { sso_session_id } => {
                sso_session_id == actor.sso_session_id()
                    || (!plan.user_ids.is_empty()
                        && plan.user_ids.iter().all(|target| *target == user_id))
            }
            IdentityRevocationScope::UserAll { user_id: target } => target == user_id,
            IdentityRevocationScope::ApplicationLocal { .. }
            | IdentityRevocationScope::Grant { .. } => {
                !plan.user_ids.is_empty() && plan.user_ids.iter().all(|target| *target == user_id)
            }
            _ => false,
        };
        if !allowed {
            return Err(IdentityError::Forbidden);
        }
        let current = transaction.revocation_plan(&self.scope).await?;
        if current.locks.keys() != plan.locks.keys()
            || current.rp_session_ids != plan.rp_session_ids
        {
            return Err(IdentityError::Conflict);
        }
        transaction
            .apply(IdentityWrite::Revoke(self.scope.clone()))
            .await?;
        let event_id = uuid::Uuid::new_v4();
        transaction
            .append_audit(&AuditEvent {
                id: event_id,
                request_id: self.request_id,
                actor_id: Some(user_id),
                action: "session.revoke",
                object_kind: "session",
                object_id: actor.sso_session_id().to_string(),
                result: AuditResult::Success,
                reason: None,
                change_summary: serde_json::json!({"affected_count":current.rp_session_ids.len()}),
                operation_id: None,
                commit_marker: Some(event_id),
            })
            .await?;
        transaction
            .enqueue_logout(&LogoutEvent {
                audit_event_id: event_id,
                rp_session_ids: current.rp_session_ids,
            })
            .await?;
        Ok(())
    }
}

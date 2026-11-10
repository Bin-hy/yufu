use super::{
    account::{authenticate_current_actor, read_current_actor},
    application::finish_read,
    browser::BrowserWriteEvidence,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::domain::identity::{
    account::SecretDigest, console::*, error::IdentityError, operation::IdentityOperation,
    session::IdentityRevocationScope,
};
use crate::service::security::repository::now;
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;
use uuid::Uuid;

pub struct ConsoleService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}
impl ConsoleService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn list(
        &self,
        secret: &str,
        kind: ConsoleKind,
        after: Option<&str>,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<ConsoleResult, IdentityError> {
        validate_limit(limit)?;
        super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            let operation = match kind {
                ConsoleKind::Accounts => Some(IdentityOperation::AccountRead),
                ConsoleKind::Audit => Some(IdentityOperation::AuditRead),
                ConsoleKind::Deliveries => Some(IdentityOperation::DeliveryRead),
                ConsoleKind::AppDeliveries => return Err(IdentityError::InvalidInput),
                _ => None,
            };
            if let Some(operation) = operation {
                if !actor.account().administrator {
                    return Err(IdentityError::Forbidden);
                }
                tx.authorize_identity_operation(&actor, operation).await?;
            }
            let user_id = actor.account().user_id;
            let root_id = actor.sso_session_id();
            let uuid_cursor = || {
                after
                    .map(|s| s.parse::<Uuid>().map_err(|_| IdentityError::InvalidInput))
                    .transpose()
            };
            let integer_cursor = || -> Result<i32, IdentityError> {
                let v = after
                    .map(|s| s.parse::<i32>().map_err(|_| IdentityError::InvalidInput))
                    .transpose()?
                    .unwrap_or(0);
                if v < 0 {
                    Err(IdentityError::InvalidInput)
                } else {
                    Ok(v)
                }
            };
            let query = match kind {
                ConsoleKind::Applications => ConsoleQuery::Applications {
                    user_id,
                    root_id,
                    after: uuid_cursor()?,
                    limit,
                },
                ConsoleKind::Consents => ConsoleQuery::Consents {
                    user_id,
                    after: integer_cursor()?,
                    limit,
                },
                ConsoleKind::Centers => ConsoleQuery::Centers {
                    user_id,
                    current_id: root_id,
                    after: uuid_cursor()?,
                    limit,
                },
                ConsoleKind::RelyingParties => ConsoleQuery::RelyingParties {
                    user_id,
                    after: uuid_cursor()?,
                    limit,
                },
                ConsoleKind::Accounts => ConsoleQuery::Accounts {
                    after: integer_cursor()?,
                    limit,
                },
                ConsoleKind::Audit => ConsoleQuery::Audit {
                    after: uuid_cursor()?,
                    limit,
                },
                ConsoleKind::Deliveries => ConsoleQuery::Deliveries {
                    after: uuid_cursor()?,
                    limit,
                },
                ConsoleKind::AppDeliveries => return Err(IdentityError::InvalidInput),
            };
            tx.console_read(query).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    /// Owner-scoped delivery overview: a developer sees only the environment
    /// they hold membership for, an administrator needs `DeliveryRead`.
    pub async fn list_app_deliveries(
        &self,
        secret: &str,
        application_id: i32,
        environment_id: Uuid,
        after: Option<&str>,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<ConsoleResult, IdentityError> {
        if application_id <= 0 {
            return Err(IdentityError::InvalidInput);
        }
        validate_limit(limit)?;
        super::sso::validate_opaque(secret)?;
        let after = after
            .map(|value| {
                value
                    .parse::<Uuid>()
                    .map_err(|_| IdentityError::InvalidInput)
            })
            .transpose()?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            let workspace = tx.application_workspace(application_id).await?;
            if actor.account().administrator {
                tx.authorize_identity_operation(&actor, IdentityOperation::DeliveryRead)
                    .await?;
            } else {
                super::application::require_membership(&workspace, &actor)?;
            }
            if !workspace
                .environments
                .iter()
                .any(|e| e.environment_id == environment_id)
            {
                return Err(IdentityError::Forbidden);
            }
            tx.console_read(ConsoleQuery::AppDeliveries {
                application_id,
                environment_id,
                after,
                limit,
            })
            .await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn withdraw_consent(
        &self,
        evidence: &BrowserWriteEvidence,
        consent_id: i32,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        if consent_id <= 0 {
            return Err(IdentityError::InvalidInput);
        }
        let digest = evidence
            .current_center_digest
            .clone()
            .ok_or(IdentityError::Unauthenticated)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &Withdrawal {
                digest,
                consent_id,
                request_id,
                clock: self.clock.as_ref(),
            },
        )
        .await
    }
    pub async fn retry_delivery(
        &self,
        evidence: &BrowserWriteEvidence,
        delivery_id: Uuid,
        request_id: Uuid,
        max_age_seconds: u64,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        if delivery_id.is_nil() || !(1..=86_400).contains(&max_age_seconds) {
            return Err(IdentityError::InvalidInput);
        }
        let digest = evidence
            .current_center_digest
            .clone()
            .ok_or(IdentityError::Unauthenticated)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &Retry {
                digest,
                delivery_id,
                request_id,
                max_age_seconds,
                clock: self.clock.as_ref(),
            },
        )
        .await
    }
}
struct Withdrawal<'a> {
    digest: SecretDigest,
    consent_id: i32,
    request_id: Uuid,
    clock: &'a dyn Clock,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Withdrawal<'_> {
    type Output = ();
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<(), IdentityError> {
        let initial = read_current_actor(tx, &self.digest, now(self.clock)?).await?;
        let user_id = initial.account().user_id;
        let target = tx
            .personal_consent_environment(user_id, self.consent_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let scope = IdentityRevocationScope::EnvironmentUser {
            environment_id: target.1,
            user_id,
        };
        let plan = tx.revocation_plan(&scope).await?;
        let workspace = tx.application_workspace(target.0).await?;
        let mut keys = plan.locks.keys().to_vec();
        keys.extend_from_slice(workspace.locks.keys());
        keys.push(IdentityLockKey::Policy);
        let actor = authenticate_current_actor(
            tx,
            &self.digest,
            now(self.clock)?,
            &IdentityLockScope::new(keys)?,
        )
        .await?;
        if actor.account().user_id != user_id
            || tx
                .personal_consent_environment(user_id, self.consent_id)
                .await?
                != Some(target)
        {
            return Err(IdentityError::NotFound);
        }
        let current = tx.revocation_plan(&scope).await?;
        if plan.locks.keys() != current.locks.keys()
            || plan.rp_session_ids != current.rp_session_ids
        {
            return Err(IdentityError::Conflict);
        }
        tx.apply(IdentityWrite::WithdrawConsent {
            user_id,
            environment_id: target.1,
        })
        .await?;
        tx.apply(IdentityWrite::Revoke(scope)).await?;
        let event_id = Uuid::new_v4();
        tx.append_audit(&AuditEvent{id:event_id,request_id:self.request_id,actor_id:Some(user_id),action:"consent.withdraw",object_kind:"consent",object_id:self.consent_id.to_string(),result:AuditResult::Success,reason:None,change_summary:serde_json::json!({"environment_id":target.1.to_string(),"affected_count":current.rp_session_ids.len()}),operation_id:None,commit_marker:Some(event_id)}).await?;
        tx.enqueue_logout(&LogoutEvent {
            audit_event_id: event_id,
            rp_session_ids: current.rp_session_ids,
        })
        .await?;
        Ok(())
    }
}
struct Retry<'a> {
    digest: SecretDigest,
    delivery_id: Uuid,
    request_id: Uuid,
    max_age_seconds: u64,
    clock: &'a dyn Clock,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Retry<'_> {
    type Output = ();
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<(), IdentityError> {
        let actor = authenticate_current_actor(
            tx,
            &self.digest,
            now(self.clock)?,
            &IdentityLockScope::new(vec![IdentityLockKey::Policy])?,
        )
        .await?;
        if !actor.account().administrator {
            return Err(IdentityError::Forbidden);
        }
        tx.authorize_identity_operation(&actor, IdentityOperation::DeliveryRetry)
            .await?;
        tx.apply(IdentityWrite::RetryLogout {
            delivery_id: self.delivery_id,
            max_age_seconds: self.max_age_seconds,
        })
        .await?;
        let id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: "logout.delivery.retry",
            object_kind: "logout_delivery",
            object_id: self.delivery_id.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: serde_json::json!({"status":"pending"}),
            operation_id: None,
            commit_marker: Some(id),
        })
        .await?;
        Ok(())
    }
}

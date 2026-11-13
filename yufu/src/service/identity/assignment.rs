//! Explicit access sources; every mutation and revocation commits in one identity transaction.
use super::{
    account::{authenticate_current_actor, read_current_actor},
    application::finish_read,
    browser::BrowserWriteEvidence,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::{
    domain::identity::{
        account::{SecretDigest, VerifiedActor},
        error::IdentityError,
        operation::IdentityOperation,
        registration::validate_idempotency_key,
        session::IdentityRevocationScope,
    },
    service::security::repository::now,
};
use auth::security::budget::{Clock, SecurityBudget};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
#[path = "assignment_change.rs"]
mod change;
use change::AssignmentPlan;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssignmentMutation {
    CreateGroup {
        name: String,
        description: Option<String>,
    },
    UpdateGroup {
        group_id: Uuid,
        name: String,
        description: Option<String>,
        expected_revision: u64,
    },
    SetGroupMember {
        group_id: Uuid,
        user_id: i32,
        present: bool,
        expected_revision: u64,
    },
    DeleteGroup {
        group_id: Uuid,
        expected_revision: u64,
    },
    SetAssignment {
        application_id: i32,
        target: AssignmentTarget,
        present: bool,
        expected_revision: u64,
    },
    SetTestUser {
        application_id: i32,
        environment_id: Uuid,
        user_id: i32,
        present: bool,
        expected_revision: u64,
    },
}
impl AssignmentMutation {
    fn kind(&self) -> &'static str {
        match self {
            Self::CreateGroup { .. } => "group.create",
            Self::UpdateGroup { .. } => "group.update",
            Self::SetGroupMember { .. } => "group.member.change",
            Self::DeleteGroup { .. } => "group.delete",
            Self::SetAssignment { .. } => "application.assignment.change",
            Self::SetTestUser { .. } => "application.test_user.change",
        }
    }
    fn operation(&self) -> IdentityOperation {
        match self {
            Self::CreateGroup { .. } => IdentityOperation::GroupCreate,
            Self::UpdateGroup { .. } => IdentityOperation::GroupConfigure,
            Self::SetGroupMember { .. } => IdentityOperation::GroupMember,
            Self::DeleteGroup { .. } => IdentityOperation::GroupDelete,
            Self::SetAssignment { .. } => IdentityOperation::ApplicationAssignment,
            Self::SetTestUser { .. } => IdentityOperation::ApplicationTestUser,
        }
    }
    fn normalize(&mut self) -> Result<(), IdentityError> {
        match self {
            Self::CreateGroup { name, description }
            | Self::UpdateGroup {
                name, description, ..
            } => {
                *name = name.trim().to_owned();
                *description = description
                    .take()
                    .map(|v| v.trim().to_owned())
                    .filter(|v| !v.is_empty());
                if name.is_empty()
                    || name.len() > 128
                    || name.chars().any(char::is_control)
                    || description
                        .as_ref()
                        .is_some_and(|v| v.len() > 2000 || v.chars().any(char::is_control))
                {
                    return Err(IdentityError::InvalidInput);
                }
            }
            _ => {}
        }
        let (revision, valid) = match self {
            Self::CreateGroup { .. } => (0, true),
            Self::UpdateGroup {
                expected_revision, ..
            }
            | Self::DeleteGroup {
                expected_revision, ..
            } => (*expected_revision, true),
            Self::SetGroupMember {
                expected_revision,
                user_id,
                ..
            } => (*expected_revision, *user_id > 0),
            Self::SetAssignment {
                expected_revision,
                application_id,
                target,
                ..
            } => (
                *expected_revision,
                *application_id > 0
                    && match target {
                        AssignmentTarget::User(id) => *id > 0,
                        AssignmentTarget::Group(_) => true,
                    },
            ),
            Self::SetTestUser {
                expected_revision,
                application_id,
                user_id,
                ..
            } => (*expected_revision, *application_id > 0 && *user_id > 0),
        };
        if !valid || revision >= i64::MAX as u64 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}
pub struct AssignmentService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}
#[derive(Serialize)]
pub struct GroupPage {
    pub items: Vec<GroupWorkspace>,
    pub next: Option<Uuid>,
}
impl AssignmentService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn mutate(
        &self,
        evidence: &BrowserWriteEvidence,
        mut mutation: AssignmentMutation,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<SafeOperationReceipt, IdentityError> {
        validate_idempotency_key(key)?;
        mutation.normalize()?;
        let hash = hex::encode(Sha256::digest(
            serde_json::to_vec(&mutation).map_err(|_| IdentityError::InvalidInput)?,
        ));
        execute_identity(
            self.repository.as_ref(),
            budget,
            &AssignmentUnit {
                service: self,
                evidence,
                mutation: &mutation,
                key,
                request_id,
                hash: &hash,
                operation_id: Uuid::new_v4(),
                new_group_id: Uuid::new_v4(),
            },
        )
        .await
    }
    async fn reader(
        &self,
        tx: &mut dyn IdentityTransaction,
        secret: &str,
        operation: IdentityOperation,
    ) -> Result<VerifiedActor, IdentityError> {
        super::sso::validate_opaque(secret)?;
        let actor =
            read_current_actor(tx, &SecretDigest::of(secret), now(self.clock.as_ref())?).await?;
        tx.authorize_identity_operation(&actor, operation).await?;
        Ok(actor)
    }
    pub async fn groups(
        &self,
        secret: &str,
        after: Option<Uuid>,
        limit: u32,
        search: Option<&str>,
        budget: &SecurityBudget,
    ) -> Result<GroupPage, IdentityError> {
        if limit == 0 || limit > 50 {
            return Err(IdentityError::InvalidInput);
        }
        let search = super::search::normalize_query(search)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            self.reader(tx.as_mut(), secret, IdentityOperation::GroupRead)
                .await?;
            let mut ids = tx.group_ids(after, limit + 1, search.as_deref()).await?;
            let next = if ids.len() > limit as usize {
                ids.truncate(limit as usize);
                ids.last().copied()
            } else {
                None
            };
            let mut items = Vec::new();
            for id in ids {
                items.push(tx.group_workspace(id).await?);
            }
            Ok(GroupPage { items, next })
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn group(
        &self,
        secret: &str,
        id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<GroupWorkspace, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            self.reader(tx.as_mut(), secret, IdentityOperation::GroupRead)
                .await?;
            tx.group_workspace(id).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn assignments(
        &self,
        secret: &str,
        app: i32,
        budget: &SecurityBudget,
    ) -> Result<Vec<AssignmentTargetSummary>, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            self.reader(tx.as_mut(), secret, IdentityOperation::ApplicationRead)
                .await?;
            tx.application_workspace(app).await?;
            tx.assignment_target_summaries(app).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
}
struct AssignmentUnit<'a> {
    service: &'a AssignmentService,
    evidence: &'a BrowserWriteEvidence,
    mutation: &'a AssignmentMutation,
    key: &'a str,
    request_id: Uuid,
    hash: &'a str,
    operation_id: Uuid,
    new_group_id: Uuid,
}
fn checked_receipt(
    receipt: SafeOperationReceipt,
    hash: &str,
) -> Result<SafeOperationReceipt, IdentityError> {
    if receipt.request_hash == hash {
        Ok(receipt)
    } else {
        Err(IdentityError::Conflict)
    }
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for AssignmentUnit<'_> {
    type Output = SafeOperationReceipt;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        let digest = self
            .evidence
            .current_center_digest
            .as_ref()
            .ok_or(IdentityError::Unauthenticated)?;
        let actor = read_current_actor(tx, digest, now(self.service.clock.as_ref())?).await?;
        tx.authorize_identity_operation(&actor, self.mutation.operation())
            .await?;
        if let Some(receipt) = tx
            .operation_receipt(actor.account().user_id, self.mutation.kind(), self.key)
            .await?
        {
            return checked_receipt(receipt, self.hash);
        }
        let plan = AssignmentPlan::load(tx, self.mutation).await?;
        let actor =
            authenticate_current_actor(tx, digest, now(self.service.clock.as_ref())?, &plan.locks)
                .await?;
        tx.authorize_identity_operation(&actor, self.mutation.operation())
            .await?;
        if let Some(receipt) = tx
            .operation_receipt(actor.account().user_id, self.mutation.kind(), self.key)
            .await?
        {
            return checked_receipt(receipt, self.hash);
        }
        plan.recheck(tx, self.mutation).await?;
        let live = tx.live_rp_ids(&plan.rp_ids).await?;
        let (mut summary, resource, changed) = plan
            .apply(
                tx,
                self.mutation,
                actor.account().user_id,
                self.new_group_id,
            )
            .await?;
        if changed {
            for (environment_id, user_id) in &plan.pairs {
                tx.apply(IdentityWrite::RecomputeGeneration {
                    environment_id: *environment_id,
                    user_id: *user_id,
                })
                .await?;
            }
        }
        let remaining = tx.live_rp_ids(&live).await?;
        let revoked: Vec<_> = live
            .into_iter()
            .filter(|id| !remaining.contains(id))
            .collect();
        summary["affected_count"] = serde_json::json!(revoked.len());
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: self.operation_id,
            actor_id: actor.account().user_id,
            operation_kind: self.mutation.kind(),
            idempotency_key: self.key.into(),
            request_hash: self.hash.into(),
            resource_id: resource.clone(),
            summary: summary.clone(),
        }))
        .await?;
        let event_id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id: event_id,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: self.mutation.kind(),
            object_kind: if matches!(
                self.mutation,
                AssignmentMutation::SetAssignment { .. } | AssignmentMutation::SetTestUser { .. }
            ) {
                "application"
            } else {
                "group"
            },
            object_id: resource,
            result: AuditResult::Success,
            reason: None,
            change_summary: summary,
            operation_id: Some(self.operation_id),
            commit_marker: Some(event_id),
        })
        .await?;
        tx.enqueue_logout(&LogoutEvent {
            audit_event_id: event_id,
            rp_session_ids: revoked,
        })
        .await?;
        for application_id in plan.application_ids() {
            tx.associate_audit_application(event_id, application_id, None)
                .await?;
        }
        checked_receipt(
            tx.operation_receipt(actor.account().user_id, self.mutation.kind(), self.key)
                .await?
                .ok_or(IdentityError::DependencyUnavailable)?,
            self.hash,
        )
    }
}

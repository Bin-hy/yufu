//! Lock planning, policy rechecks and atomic application changes.
use super::*;

pub(super) async fn authorize(
    tx: &mut dyn IdentityTransaction,
    actor: &VerifiedActor,
    workspace: &ApplicationWorkspace,
    mutation: &ApplicationMutation,
    administrator: bool,
) -> Result<(), IdentityError> {
    let admin_operation = match mutation {
        ApplicationMutation::ActivateProduction { .. } => {
            Some(IdentityOperation::ApplicationActivate)
        }
        ApplicationMutation::UpdateMetadata { .. }
            if administrator || workspace.management_mode == ManagementMode::AdminConfigured =>
        {
            Some(IdentityOperation::ApplicationMetadata)
        }
        ApplicationMutation::ResetSecret { .. }
            if administrator || workspace.management_mode == ManagementMode::AdminConfigured =>
        {
            Some(IdentityOperation::ApplicationSecret)
        }
        ApplicationMutation::ReviewProduction { .. } => Some(IdentityOperation::ApplicationReview),
        ApplicationMutation::SetLifecycle { .. } => Some(IdentityOperation::ApplicationLifecycle),
        ApplicationMutation::TransferOwner { .. } if administrator => {
            Some(IdentityOperation::ApplicationOwner)
        }
        _ if administrator || workspace.management_mode == ManagementMode::AdminConfigured => {
            Some(IdentityOperation::ApplicationConfigure)
        }
        _ => None,
    };
    if let Some(operation) = admin_operation {
        if !actor.account().administrator {
            return Err(IdentityError::Forbidden);
        }
        tx.authorize_identity_operation(actor, operation).await?;
        if matches!(
            mutation,
            ApplicationMutation::UpdateConfig { .. }
                | ApplicationMutation::ResetSecret { .. }
                | ApplicationMutation::SetMember { .. }
        ) && workspace.management_mode != ManagementMode::AdminConfigured
            && administrator
        {
            return Err(IdentityError::Forbidden);
        }
    } else {
        require_developer(actor)?;
        require_membership(workspace, actor)?;
        if matches!(
            mutation,
            ApplicationMutation::SetMember { .. } | ApplicationMutation::TransferOwner { .. }
        ) && workspace.owner_user_id != actor.account().user_id
        {
            return Err(IdentityError::Forbidden);
        }
    }
    Ok(())
}
pub(super) struct MutationUnit<'a> {
    pub(super) service: &'a ApplicationService,
    pub(super) evidence: &'a BrowserWriteEvidence,
    pub(super) application_id: i32,
    pub(super) mutation: &'a ApplicationMutation,
    pub(super) key: &'a str,
    pub(super) request_id: Uuid,
    pub(super) hash: &'a str,
    pub(super) prepared: &'a PreparedMutation,
    pub(super) administrator: bool,
}
pub(super) struct MutationOutcome {
    pub(super) receipt: SafeOperationReceipt,
    pub(super) changed: bool,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for MutationUnit<'_> {
    type Output = MutationOutcome;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        let digest = self
            .evidence
            .current_center_digest
            .as_ref()
            .ok_or(IdentityError::Unauthenticated)?;
        let initial_actor =
            read_current_actor(tx, digest, now(self.service.clock.as_ref())?).await?;
        let initial = tx.application_workspace(self.application_id).await?;
        authorize(
            tx,
            &initial_actor,
            &initial,
            self.mutation,
            self.administrator,
        )
        .await?;
        let scopes = revocation_scopes(&initial, self.mutation, tx).await?;
        let mut keys = initial.locks.keys().to_vec();
        if let ApplicationMutation::SetMember { user_id, .. }
        | ApplicationMutation::TransferOwner { user_id, .. } = self.mutation
        {
            keys.push(IdentityLockKey::User(*user_id));
        }
        let mut plans = Vec::new();
        for scope in &scopes {
            let plan = tx.revocation_plan(scope).await?;
            keys.extend_from_slice(plan.locks.keys());
            plans.push(plan);
        }
        let actor = authenticate_current_actor(
            tx,
            digest,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(keys)?,
        )
        .await?;
        let workspace = tx.application_workspace(self.application_id).await?;
        authorize(tx, &actor, &workspace, self.mutation, self.administrator).await?;
        if let Some(receipt) = tx
            .operation_receipt(actor.account().user_id, self.mutation.kind(), self.key)
            .await?
        {
            check_receipt(&receipt, self.hash)?;
            return Ok(MutationOutcome {
                receipt,
                changed: false,
            });
        }
        if workspace.locks.keys() != initial.locks.keys() {
            return Err(IdentityError::Conflict);
        }
        let mut rp_ids = Vec::new();
        for (scope, plan) in scopes.iter().zip(&plans) {
            let current = tx.revocation_plan(scope).await?;
            if current.locks.keys() != plan.locks.keys()
                || current.rp_session_ids != plan.rp_session_ids
            {
                return Err(IdentityError::Conflict);
            }
            rp_ids.extend(current.rp_session_ids);
        }
        rp_ids.sort();
        rp_ids.dedup();
        let live = tx.live_rp_ids(&rp_ids).await?;
        let mut summary = self.apply_change(tx, &actor, &workspace).await?;
        let remaining = tx.live_rp_ids(&live).await?;
        let revoked: Vec<_> = live
            .into_iter()
            .filter(|id| !remaining.contains(id))
            .collect();
        summary["affected_count"] = serde_json::json!(revoked.len());
        let event_id = Uuid::new_v4();
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: self.prepared.operation_id,
            actor_id: actor.account().user_id,
            operation_kind: self.mutation.kind(),
            idempotency_key: self.key.into(),
            request_hash: self.hash.into(),
            resource_id: self.application_id.to_string(),
            summary: summary.clone(),
        }))
        .await?;
        tx.append_audit(&AuditEvent {
            id: event_id,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: self.mutation.kind(),
            object_kind: "application",
            object_id: self.application_id.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: summary,
            operation_id: Some(self.prepared.operation_id),
            commit_marker: Some(event_id),
        })
        .await?;
        tx.enqueue_logout(&LogoutEvent {
            audit_event_id: event_id,
            rp_session_ids: revoked,
        })
        .await?;
        let receipt = tx
            .operation_receipt(actor.account().user_id, self.mutation.kind(), self.key)
            .await?
            .ok_or(IdentityError::DependencyUnavailable)?;
        Ok(MutationOutcome {
            receipt,
            changed: true,
        })
    }
}
async fn revocation_scopes(
    workspace: &ApplicationWorkspace,
    mutation: &ApplicationMutation,
    tx: &mut dyn IdentityTransaction,
) -> Result<Vec<IdentityRevocationScope>, IdentityError> {
    let ids = match mutation {
        ApplicationMutation::UpdateConfig { environment_id, .. }
        | ApplicationMutation::ActivateProduction { environment_id, .. }
        | ApplicationMutation::ResetSecret { environment_id, .. } => vec![*environment_id],
        ApplicationMutation::ReviewProduction { approval_id, .. } => {
            let a = tx.approval_workspace(*approval_id).await?;
            if a.application_id != workspace.application_id {
                return Err(IdentityError::NotFound);
            }
            vec![a.environment_id]
        }
        ApplicationMutation::SetMember { user_id, .. } => {
            return Ok(workspace
                .environments
                .iter()
                .filter(|e| e.kind == AppEnvironmentKind::Development)
                .map(|e| IdentityRevocationScope::EnvironmentUser {
                    environment_id: e.environment_id,
                    user_id: *user_id,
                })
                .collect());
        }
        ApplicationMutation::SetLifecycle { .. } => workspace
            .environments
            .iter()
            .map(|e| e.environment_id)
            .collect(),
        _ => vec![],
    };
    Ok(ids
        .into_iter()
        .map(|environment_id| IdentityRevocationScope::Environment { environment_id })
        .collect())
}
impl MutationUnit<'_> {
    async fn apply_change(
        &self,
        tx: &mut dyn IdentityTransaction,
        actor: &VerifiedActor,
        workspace: &ApplicationWorkspace,
    ) -> Result<serde_json::Value, IdentityError> {
        let app = workspace.application_id;
        let expected = self.mutation.expected_revision();
        let mut summary = serde_json::json!({"application_id":app});
        match self.mutation {
            ApplicationMutation::UpdateMetadata {
                name, description, ..
            } => {
                if workspace.revision != expected {
                    return Err(IdentityError::Conflict);
                }
                tx.apply(IdentityWrite::UpdateApplicationMetadata {
                    application_id: app,
                    name: name.clone(),
                    description: description.clone(),
                    expected_revision: expected,
                })
                .await?;
                summary["metadata_changed"] = serde_json::json!(true);
                summary["revision"] = serde_json::json!(expected + 1);
            }
            ApplicationMutation::ActivateProduction {
                environment_id,
                version_id,
                accept_logout_limitation,
                ..
            } => {
                let env = environment(workspace, *environment_id)?;
                if !workspace.active
                    || workspace.management_mode != ManagementMode::AdminConfigured
                    || env.kind != AppEnvironmentKind::Production
                {
                    return Err(IdentityError::Forbidden);
                }
                if env.revision != expected || env.latest.version_id != *version_id {
                    return Err(IdentityError::Conflict);
                }
                let owner = tx
                    .load_account(workspace.owner_user_id)
                    .await?
                    .ok_or(IdentityError::OwnerRequired)?;
                if !owner_is_qualified(&owner, workspace.management_mode) {
                    return Err(IdentityError::OwnerRequired);
                }
                env.latest.config.clone().validate(env.kind, true)?;
                let snapshot = tx
                    .approval_snapshot(*environment_id, *version_id, *accept_logout_limitation)
                    .await?;
                tx.apply(IdentityWrite::RecordAdminActivation {
                    snapshot,
                    actor_id: actor.account().user_id,
                })
                .await?;
                tx.apply(IdentityWrite::ActivateVersion {
                    environment_id: *environment_id,
                    version_id: *version_id,
                    expected_revision: expected,
                })
                .await?;
                summary["environment_id"] = serde_json::json!(environment_id);
                summary["version_id"] = serde_json::json!(version_id);
                summary["revision"] = serde_json::json!(expected + 1);
                summary["logout_limitation_accepted"] = serde_json::json!(accept_logout_limitation);
            }
            ApplicationMutation::UpdateConfig {
                environment_id,
                config,
                ..
            } => {
                let env = environment(workspace, *environment_id)?;
                if env.revision != expected {
                    return Err(IdentityError::Conflict);
                }
                let config = config
                    .clone()
                    .validate(env.kind, env.kind == AppEnvironmentKind::Development)?;
                let candidate = if env.primary_client_kind != config.client_kind {
                    let IdentityWriteResult::Client {
                        oauth_client_id, ..
                    } = tx
                        .apply(IdentityWrite::CreateCandidateClient {
                            environment_id: *environment_id,
                            client_id: self.prepared.candidate_client_id.clone(),
                            client_secret_hash: self.prepared.hash_copy()?,
                            config: config.clone(),
                            actor_id: actor.account().user_id,
                        })
                        .await?
                    else {
                        return Err(IdentityError::DependencyUnavailable);
                    };
                    Some(oauth_client_id)
                } else {
                    None
                };
                tx.apply(IdentityWrite::InsertVersion(Box::new(VersionWrite {
                    id: self.prepared.version_id,
                    environment_id: *environment_id,
                    expected_revision: expected,
                    config,
                    candidate_client_id: candidate,
                    actor_id: actor.account().user_id,
                })))
                .await?;
                let revision = if env.kind == AppEnvironmentKind::Development {
                    tx.apply(IdentityWrite::ActivateVersion {
                        environment_id: *environment_id,
                        version_id: self.prepared.version_id,
                        expected_revision: expected + 1,
                    })
                    .await?;
                    expected + 2
                } else {
                    expected + 1
                };
                summary["environment_id"] = serde_json::json!(environment_id);
                summary["version_id"] = serde_json::json!(self.prepared.version_id);
                summary["revision"] = serde_json::json!(revision);
                summary["client_id"] = serde_json::json!(if candidate.is_some() {
                    &self.prepared.candidate_client_id
                } else {
                    &env.client_id
                });
            }
            ApplicationMutation::SubmitProduction {
                environment_id,
                version_id,
                ..
            } => {
                let env = environment(workspace, *environment_id)?;
                if !workspace.active
                    || env.kind != AppEnvironmentKind::Production
                    || workspace.management_mode != ManagementMode::DeveloperManaged
                {
                    return Err(IdentityError::Forbidden);
                }
                if env.revision != expected || env.latest.version_id != *version_id {
                    return Err(IdentityError::Conflict);
                }
                if env.pending_approval.is_some() {
                    return Err(IdentityError::Conflict);
                }
                env.latest.config.clone().validate(env.kind, true)?;
                let snapshot = tx
                    .approval_snapshot(*environment_id, *version_id, false)
                    .await?;
                tx.apply(IdentityWrite::SubmitApproval(Box::new(ApprovalWrite {
                    id: self.prepared.approval_id,
                    snapshot,
                    requested_by: actor.account().user_id,
                })))
                .await?;
                summary["approval_id"] = serde_json::json!(self.prepared.approval_id);
                summary["environment_id"] = serde_json::json!(environment_id);
                summary["version_id"] = serde_json::json!(version_id);
                summary["revision"] = serde_json::json!(expected + 1);
                summary["status"] = serde_json::json!("pending");
            }
            ApplicationMutation::ReviewProduction {
                approval_id,
                approve,
                reason,
                accept_logout_limitation,
                ..
            } => {
                let approval = tx.approval_workspace(*approval_id).await?;
                let env = environment(workspace, approval.environment_id)?;
                if workspace.management_mode != ManagementMode::DeveloperManaged
                    || !workspace.active
                {
                    return Err(IdentityError::Forbidden);
                }
                if approval.status != "pending" || env.latest.version_id != approval.version_id {
                    return Err(IdentityError::ApprovalObsolete);
                }
                if env.revision != expected {
                    return Err(IdentityError::Conflict);
                }
                tx.apply(IdentityWrite::ReviewApproval {
                    approval_id: *approval_id,
                    reviewer_id: actor.account().user_id,
                    approve: *approve,
                    reason: reason.clone(),
                    accept_logout_limitation: *accept_logout_limitation,
                })
                .await?;
                if *approve {
                    tx.apply(IdentityWrite::ActivateVersion {
                        environment_id: approval.environment_id,
                        version_id: approval.version_id,
                        expected_revision: expected,
                    })
                    .await?;
                }
                summary["approval_id"] = serde_json::json!(approval_id);
                summary["environment_id"] = serde_json::json!(env.environment_id);
                summary["version_id"] = serde_json::json!(approval.version_id);
                summary["revision"] = serde_json::json!(expected + u64::from(*approve));
                summary["status"] =
                    serde_json::json!(if *approve { "approved" } else { "rejected" });
                summary["logout_limitation_accepted"] =
                    serde_json::json!(*approve && *accept_logout_limitation);
            }
            ApplicationMutation::ResetSecret { environment_id, .. } => {
                let env = environment(workspace, *environment_id)?;
                if env.revision != expected {
                    return Err(IdentityError::Conflict);
                }
                if !env
                    .primary_client_kind
                    .eq(&crate::domain::identity::application::ClientKind::ServerWeb)
                {
                    return Err(IdentityError::InvalidInput);
                }
                tx.apply(IdentityWrite::ResetEnvironmentSecret {
                    environment_id: *environment_id,
                    expected_revision: expected,
                    secret_hash: self
                        .prepared
                        .hash_copy()?
                        .ok_or(IdentityError::InvalidInput)?,
                })
                .await?;
                summary["environment_id"] = serde_json::json!(environment_id);
                summary["client_id"] = serde_json::json!(env.client_id);
                summary["revision"] = serde_json::json!(expected + 1);
                summary["secret_revision"] = serde_json::json!(
                    env.secret_revision
                        .checked_add(1)
                        .ok_or(IdentityError::CapacityExceeded)?
                );
            }
            ApplicationMutation::SetMember { user_id, .. }
            | ApplicationMutation::TransferOwner { user_id, .. } => {
                if workspace.revision != expected {
                    return Err(IdentityError::Conflict);
                }
                let target = tx
                    .load_account(*user_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                let transfer = matches!(self.mutation, ApplicationMutation::TransferOwner { .. });
                if transfer
                    || matches!(
                        self.mutation,
                        ApplicationMutation::SetMember { present: true, .. }
                    )
                {
                    if !owner_is_qualified(&target, workspace.management_mode) {
                        return Err(IdentityError::OwnerRequired);
                    }
                    if !self.administrator
                        && workspace.management_mode == ManagementMode::DeveloperManaged
                        && !tx
                            .collaboration_visible(actor.account().user_id, *user_id)
                            .await?
                    {
                        return Err(IdentityError::Forbidden);
                    }
                }
                if transfer {
                    tx.apply(IdentityWrite::TransferOwner {
                        application_id: app,
                        owner_user_id: *user_id,
                        expected_revision: expected,
                    })
                    .await?;
                    summary["owner_user_id"] = serde_json::json!(user_id);
                    summary["previous_owner_user_id"] = serde_json::json!(workspace.owner_user_id);
                } else if let ApplicationMutation::SetMember { present, .. } = self.mutation {
                    if *user_id == workspace.owner_user_id {
                        return Err(IdentityError::OwnerRequired);
                    }
                    tx.apply(IdentityWrite::SetMember {
                        application_id: app,
                        user_id: *user_id,
                        present: *present,
                        actor_id: actor.account().user_id,
                        expected_revision: expected,
                    })
                    .await?;
                    summary["present"] = serde_json::json!(present);
                    summary["user_id"] = serde_json::json!(user_id);
                }
                for env in &workspace.environments {
                    if env.kind == AppEnvironmentKind::Development {
                        tx.apply(IdentityWrite::RecomputeGeneration {
                            environment_id: env.environment_id,
                            user_id: *user_id,
                        })
                        .await?;
                    }
                }
                summary["revision"] = serde_json::json!(expected + 1);
            }
            ApplicationMutation::SetLifecycle { active, .. } => {
                if workspace.revision != expected {
                    return Err(IdentityError::Conflict);
                }
                if *active {
                    let owner = tx
                        .load_account(workspace.owner_user_id)
                        .await?
                        .ok_or(IdentityError::OwnerRequired)?;
                    if !owner_is_qualified(&owner, workspace.management_mode) {
                        return Err(IdentityError::OwnerRequired);
                    }
                }
                tx.apply(IdentityWrite::SetApplicationLifecycle {
                    application_id: app,
                    active: *active,
                    expected_revision: expected,
                })
                .await?;
                summary["active"] = serde_json::json!(active);
                summary["revision"] = serde_json::json!(expected + 1);
            }
        }
        Ok(summary)
    }
}

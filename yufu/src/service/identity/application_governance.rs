use super::*;
#[path = "application_change.rs"]
mod change;
use crate::domain::identity::{
    account::{SecretDigest, owner_is_qualified},
    application::{ConsentPolicy, EnvironmentConfigInput},
    operation::IdentityOperation,
    session::IdentityRevocationScope,
};
use change::{MutationUnit, authorize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// These actions are chosen by authenticated routes, never by an OAuth request.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationMutation {
    UpdateMetadata {
        name: String,
        description: Option<String>,
        expected_revision: u64,
    },
    ActivateProduction {
        environment_id: Uuid,
        version_id: Uuid,
        expected_revision: u64,
        accept_logout_limitation: bool,
    },
    UpdateConfig {
        environment_id: Uuid,
        expected_revision: u64,
        config: EnvironmentConfigInput,
    },
    SubmitProduction {
        environment_id: Uuid,
        expected_revision: u64,
        version_id: Uuid,
    },
    ReviewProduction {
        approval_id: Uuid,
        expected_revision: u64,
        approve: bool,
        reason: Option<String>,
        accept_logout_limitation: bool,
    },
    ResetSecret {
        environment_id: Uuid,
        expected_revision: u64,
    },
    SetMember {
        user_id: i32,
        present: bool,
        expected_revision: u64,
    },
    TransferOwner {
        user_id: i32,
        expected_revision: u64,
    },
    SetLifecycle {
        active: bool,
        expected_revision: u64,
    },
}
impl ApplicationMutation {
    fn kind(&self) -> &'static str {
        match self {
            Self::UpdateMetadata { .. } => "application.metadata.update",
            Self::ActivateProduction { .. } => "application.production.activate",
            Self::UpdateConfig { .. } => "application.config.update",
            Self::SubmitProduction { .. } => "application.production.submit",
            Self::ReviewProduction { .. } => "application.production.review",
            Self::ResetSecret { .. } => "application.secret.reset",
            Self::SetMember { .. } => "application.member.change",
            Self::TransferOwner { .. } => "application.owner.transfer",
            Self::SetLifecycle { .. } => "application.lifecycle.change",
        }
    }
    fn expected_revision(&self) -> u64 {
        match self {
            Self::UpdateMetadata {
                expected_revision, ..
            }
            | Self::ActivateProduction {
                expected_revision, ..
            }
            | Self::UpdateConfig {
                expected_revision, ..
            }
            | Self::SubmitProduction {
                expected_revision, ..
            }
            | Self::ReviewProduction {
                expected_revision, ..
            }
            | Self::ResetSecret {
                expected_revision, ..
            }
            | Self::SetMember {
                expected_revision, ..
            }
            | Self::TransferOwner {
                expected_revision, ..
            }
            | Self::SetLifecycle {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    fn normalize(&mut self, workspace: &ApplicationWorkspace) -> Result<(), IdentityError> {
        if self.expected_revision() == u64::MAX {
            return Err(IdentityError::InvalidInput);
        }
        match self {
            Self::UpdateMetadata {
                name, description, ..
            } => {
                *name = name.trim().to_owned();
                *description = description
                    .take()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty());
                if name.is_empty()
                    || name.len() > 200
                    || name.chars().any(char::is_control)
                    || description
                        .as_ref()
                        .is_some_and(|s| s.len() > 2000 || s.chars().any(char::is_control))
                {
                    return Err(IdentityError::InvalidInput);
                }
            }
            Self::ActivateProduction { environment_id, .. } => {
                environment(workspace, *environment_id)?;
            }
            Self::UpdateConfig {
                environment_id,
                config,
                ..
            } => {
                let env = environment(workspace, *environment_id)?;
                let validated = config
                    .clone()
                    .validate(env.kind, env.kind == AppEnvironmentKind::Development)?;
                if workspace.management_mode == ManagementMode::DeveloperManaged
                    && validated.consent_policy == ConsentPolicy::EnterprisePreconsent
                {
                    return Err(IdentityError::Forbidden);
                }
                *config = validated.to_input();
            }
            Self::SubmitProduction { environment_id, .. }
            | Self::ResetSecret { environment_id, .. } => {
                environment(workspace, *environment_id)?;
            }
            Self::ReviewProduction {
                reason,
                approve,
                accept_logout_limitation,
                ..
            } => {
                if !*approve && *accept_logout_limitation {
                    return Err(IdentityError::InvalidInput);
                }
                if let Some(value) = reason {
                    *value = value.trim().to_owned();
                    if value.is_empty() || value.len() > 1000 || value.chars().any(char::is_control)
                    {
                        return Err(IdentityError::InvalidInput);
                    }
                }
            }
            Self::SetMember { user_id, .. } | Self::TransferOwner { user_id, .. }
                if *user_id <= 0 =>
            {
                return Err(IdentityError::InvalidInput);
            }
            _ => {}
        }
        Ok(())
    }
}
pub struct ApplicationChangeReceipt {
    pub receipt: SafeOperationReceipt,
    pub newly_issued_secret: Option<IssuedSecret>,
}
struct PreparedMutation {
    secret: Option<IssuedSecret>,
    hash: Option<SecretToken>,
    candidate_client_id: String,
    version_id: Uuid,
    approval_id: Uuid,
    operation_id: Uuid,
}
impl PreparedMutation {
    async fn new(needs_secret: bool) -> Result<Self, IdentityError> {
        let entropy = needs_secret.then(rand::random);
        let hash = match entropy {
            Some(value) => {
                Some(hash_secret(&IssuedSecret::from_entropy(value).into_response()).await?)
            }
            None => None,
        };
        Ok(Self {
            secret: entropy.map(IssuedSecret::from_entropy),
            hash,
            candidate_client_id: format!("app_{}", Uuid::new_v4()),
            version_id: Uuid::new_v4(),
            approval_id: Uuid::new_v4(),
            operation_id: Uuid::new_v4(),
        })
    }
    fn hash_copy(&self) -> Result<Option<SecretToken>, IdentityError> {
        self.hash
            .as_ref()
            .map(|h| SecretToken::new(h.expose_secret().to_owned()).map_err(Into::into))
            .transpose()
    }
}
impl ApplicationService {
    pub async fn approval(
        &self,
        secret: &str,
        app: i32,
        id: Uuid,
        administrator: bool,
        budget: &SecurityBudget,
    ) -> Result<ApprovalWorkspace, IdentityError> {
        super::super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            if administrator {
                tx.authorize_identity_operation(&actor, IdentityOperation::ApplicationRead)
                    .await?;
            } else {
                let app = tx.application_workspace(app).await?;
                require_membership(&app, &actor)?;
            }
            let approval = tx.approval_workspace(id).await?;
            if approval.application_id != app {
                return Err(IdentityError::NotFound);
            }
            Ok(approval)
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn detail_administrator(
        &self,
        secret: &str,
        app: i32,
        budget: &SecurityBudget,
    ) -> Result<ApplicationWorkspace, IdentityError> {
        super::super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            tx.authorize_identity_operation(&actor, IdentityOperation::ApplicationRead)
                .await?;
            tx.application_workspace(app).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn list_administrator(
        &self,
        secret: &str,
        after: i32,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<ApplicationPage, IdentityError> {
        if after < 0 || !(1..=50).contains(&limit) {
            return Err(IdentityError::InvalidInput);
        }
        super::super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            tx.authorize_identity_operation(&actor, IdentityOperation::ApplicationRead)
                .await?;
            let mut ids = tx.administrator_application_ids(after, limit + 1).await?;
            let next = if ids.len() > limit as usize {
                ids.truncate(limit as usize);
                ids.last().copied()
            } else {
                None
            };
            let mut items = Vec::with_capacity(ids.len());
            for id in ids {
                items.push(tx.application_workspace(id).await?);
            }
            Ok(ApplicationPage { items, next })
        }
        .await;
        finish_read(tx, result, budget).await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn set_candidate(
        &self,
        evidence: &BrowserWriteEvidence,
        requester: i32,
        candidate: i32,
        allowed: bool,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        if requester <= 0 || candidate <= 0 || requester == candidate {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &CandidateChange {
                service: self,
                evidence,
                requester,
                candidate,
                allowed,
                request_id,
            },
        )
        .await
    }
    pub async fn mutate(
        &self,
        evidence: &BrowserWriteEvidence,
        application_id: i32,
        mutation: ApplicationMutation,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<ApplicationChangeReceipt, IdentityError> {
        self.mutate_scoped(
            evidence,
            application_id,
            mutation,
            key,
            request_id,
            budget,
            false,
        )
        .await
    }
    pub async fn mutate_administrator(
        &self,
        evidence: &BrowserWriteEvidence,
        application_id: i32,
        mutation: ApplicationMutation,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<ApplicationChangeReceipt, IdentityError> {
        self.mutate_scoped(
            evidence,
            application_id,
            mutation,
            key,
            request_id,
            budget,
            true,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn mutate_scoped(
        &self,
        evidence: &BrowserWriteEvidence,
        application_id: i32,
        mut mutation: ApplicationMutation,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
        administrator: bool,
    ) -> Result<ApplicationChangeReceipt, IdentityError> {
        validate_idempotency_key(key)?;
        if application_id <= 0 {
            return Err(IdentityError::InvalidInput);
        }
        let digest = evidence
            .current_center_digest
            .as_ref()
            .ok_or(IdentityError::Unauthenticated)?;
        let mut tx = self.repository.begin(budget).await?;
        let preflight = async {
            let actor = read_current_actor(tx.as_mut(), digest, now(self.clock.as_ref())?).await?;
            let workspace = tx.application_workspace(application_id).await?;
            authorize(tx.as_mut(), &actor, &workspace, &mutation, administrator).await?;
            mutation.normalize(&workspace)?;
            let hash = request_hash(application_id, &mutation)?;
            let existing = tx
                .operation_receipt(actor.account().user_id, mutation.kind(), key)
                .await?;
            let needs_secret = match &mutation {
                _ if existing.is_some() => false,
                ApplicationMutation::ResetSecret { environment_id, .. } => {
                    if environment(&workspace, *environment_id)?.primary_client_kind
                        != crate::domain::identity::application::ClientKind::ServerWeb
                    {
                        return Err(IdentityError::InvalidInput);
                    }
                    true
                }
                ApplicationMutation::UpdateConfig {
                    environment_id,
                    config,
                    ..
                } => {
                    environment(&workspace, *environment_id)?.primary_client_kind
                        != config.client_kind
                        && config.client_kind
                            == crate::domain::identity::application::ClientKind::ServerWeb
                }
                _ => false,
            };
            Ok((hash, existing, needs_secret))
        }
        .await;
        let (hash, existing, needs_secret) = finish_read(tx, preflight, budget).await?;
        if let Some(receipt) = existing {
            check_receipt(&receipt, &hash)?;
            return Ok(ApplicationChangeReceipt {
                receipt,
                newly_issued_secret: None,
            });
        }
        let prepared = PreparedMutation::new(needs_secret).await?;
        let write_budget = budget.renewed()?;
        let output = execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &MutationUnit {
                service: self,
                evidence,
                application_id,
                mutation: &mutation,
                key,
                request_id,
                hash: &hash,
                prepared: &prepared,
                administrator,
            },
        )
        .await?;
        Ok(ApplicationChangeReceipt {
            receipt: output.receipt,
            newly_issued_secret: if output.changed {
                prepared.secret
            } else {
                None
            },
        })
    }
    pub async fn candidates(
        &self,
        secret: &str,
        after: i32,
        limit: u32,
        search: Option<&str>,
        budget: &SecurityBudget,
    ) -> Result<CandidatePage, IdentityError> {
        if after < 0 || !(1..=50).contains(&limit) {
            return Err(IdentityError::InvalidInput);
        }
        let search = super::super::search::normalize_query(search)?;
        super::super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            require_developer(&actor)?;
            let mut items = tx
                .collaboration_candidates(
                    actor.account().user_id,
                    after,
                    limit + 1,
                    search.as_deref(),
                )
                .await?;
            let next = if items.len() > limit as usize {
                items.truncate(limit as usize);
                items.last().map(|u| u.user_id)
            } else {
                None
            };
            Ok(CandidatePage { items, next })
        }
        .await;
        finish_read(tx, result, budget).await
    }
}
struct CandidateChange<'a> {
    service: &'a ApplicationService,
    evidence: &'a BrowserWriteEvidence,
    requester: i32,
    candidate: i32,
    allowed: bool,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for CandidateChange<'_> {
    type Output = ();
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<(), IdentityError> {
        let digest = self
            .evidence
            .current_center_digest
            .as_ref()
            .ok_or(IdentityError::Unauthenticated)?;
        let initial = read_current_actor(tx, digest, now(self.service.clock.as_ref())?).await?;
        tx.authorize_identity_operation(&initial, IdentityOperation::CollaborationCandidate)
            .await?;
        let actor = authenticate_current_actor(
            tx,
            digest,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(vec![
                IdentityLockKey::User(self.requester),
                IdentityLockKey::User(self.candidate),
            ])?,
        )
        .await?;
        tx.authorize_identity_operation(&actor, IdentityOperation::CollaborationCandidate)
            .await?;
        if self.allowed {
            for user in [self.requester, self.candidate] {
                let user = tx
                    .load_account(user)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                if !user.enabled || !user.developer_enabled {
                    return Err(IdentityError::Forbidden);
                }
            }
        }
        tx.apply(IdentityWrite::SetCandidate {
            requester_user_id: self.requester,
            candidate_user_id: self.candidate,
            allowed: self.allowed,
            actor_id: actor.account().user_id,
        })
        .await?;
        let id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {id,request_id:self.request_id,actor_id:Some(actor.account().user_id),action:"collaboration.candidate.change",object_kind:"account",object_id:self.requester.to_string(),result:AuditResult::Success,reason:None,change_summary:serde_json::json!({"user_id":self.requester,"target_user_id":self.candidate,"present":self.allowed}),operation_id:None,commit_marker:Some(id)}).await
    }
}
#[derive(Serialize)]
pub struct CandidatePage {
    pub items: Vec<CollaborationCandidate>,
    pub next: Option<i32>,
}
fn request_hash(app: i32, mutation: &ApplicationMutation) -> Result<String, IdentityError> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(&(app, mutation)).map_err(|_| IdentityError::InvalidInput)?,
    )))
}
fn check_receipt(receipt: &SafeOperationReceipt, hash: &str) -> Result<(), IdentityError> {
    if receipt.request_hash == hash {
        Ok(())
    } else {
        Err(IdentityError::Conflict)
    }
}
fn environment(
    workspace: &ApplicationWorkspace,
    id: Uuid,
) -> Result<&EnvironmentWorkspace, IdentityError> {
    workspace
        .environments
        .iter()
        .find(|e| e.environment_id == id)
        .ok_or(IdentityError::NotFound)
}

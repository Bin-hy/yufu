//! Authenticated self-service application operations share the identity transaction.
use super::{
    account::{authenticate_current_actor, read_current_actor},
    browser::BrowserWriteEvidence,
    reader::hash_secret,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::{
    domain::identity::{
        account::{IssuedSecret, ManagementMode, VerifiedActor},
        application::{
            AppEnvironmentKind, EnvironmentConfig, EnvironmentSummary, RegistrationReceipt,
        },
        error::IdentityError,
        registration::{CreateApplication, ValidatedRegistration, validate_idempotency_key},
    },
    service::security::repository::now,
};
use auth::security::{
    budget::{Clock, SecurityBudget},
    identity::SecretToken,
};
use std::sync::Arc;
use uuid::Uuid;

#[path = "application_admin.rs"]
mod administrator;
#[path = "application_migration.rs"]
mod migration;
pub use migration::MigrationPage;
#[path = "application_governance.rs"]
mod governance;
pub use governance::{ApplicationChangeReceipt, ApplicationMutation, CandidatePage};

pub struct ApplicationService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}
impl ApplicationService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn detail(
        &self,
        center_secret: &str,
        application_id: i32,
        budget: &SecurityBudget,
    ) -> Result<ApplicationWorkspace, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = self.read_center_actor(tx.as_mut(), center_secret).await?;
            let workspace = tx.application_workspace(application_id).await?;
            require_membership(&workspace, &actor)?;
            Ok(workspace)
        }
        .await;
        finish_read(tx, result, budget).await
    }

    pub async fn list(
        &self,
        center_secret: &str,
        after: i32,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<ApplicationPage, IdentityError> {
        if after < 0 || limit == 0 || limit > 50 {
            return Err(IdentityError::InvalidInput);
        }
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = self.read_center_actor(tx.as_mut(), center_secret).await?;
            let mut ids = tx
                .developer_application_ids(actor.account().user_id, after, limit + 1)
                .await?;
            let next = if ids.len() > limit as usize {
                ids.truncate(limit as usize);
                ids.last().copied()
            } else {
                None
            };
            let mut items = Vec::with_capacity(ids.len());
            for id in ids {
                let item = tx.application_workspace(id).await?;
                require_membership(&item, &actor)?;
                items.push(item);
            }
            Ok(ApplicationPage { items, next })
        }
        .await;
        finish_read(tx, result, budget).await
    }

    pub async fn operation(
        &self,
        center_secret: &str,
        operation_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<SafeOperationReceipt, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = self.read_center_actor(tx.as_mut(), center_secret).await?;
            tx.operation_receipt_by_id(actor.account().user_id, operation_id)
                .await?
                .ok_or(IdentityError::NotFound)
        }
        .await;
        finish_read(tx, result, budget).await
    }

    pub async fn register(
        &self,
        evidence: &BrowserWriteEvidence,
        request: CreateApplication,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<RegistrationReceipt, IdentityError> {
        validate_idempotency_key(key)?;
        let request = request.validate()?;
        // Authenticate and inspect an existing receipt before doing password work.
        // This read is a preflight only; the write locks and checks the actor again.
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = self.read_developer(tx.as_mut(), evidence).await?;
            tx.operation_receipt(actor.account().user_id, "application.register", key)
                .await
        }
        .await;
        let existing = match result {
            Ok(receipt) => {
                tx.commit().await?;
                budget.remaining()?;
                receipt
            }
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(error);
            }
        };
        if let Some(receipt) = existing {
            return registration_from_receipt(receipt, &request.request_hash);
        }
        let (development, production) = tokio::try_join!(
            PreparedEnvironment::new(&request.development),
            PreparedEnvironment::new(&request.production)
        )?;
        let operation_id = Uuid::new_v4();
        let write_budget = budget.renewed()?;
        let outcome = execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &Register {
                service: self,
                evidence,
                request: &request,
                key,
                request_id,
                operation_id,
                development: &development,
                production: &production,
                administrator: None,
            },
        )
        .await?;
        let mut receipt = registration_from_receipt(outcome.receipt, &request.request_hash)?;
        if outcome.created {
            receipt.newly_issued_secrets = [development, production]
                .into_iter()
                .filter_map(|environment| environment.secret.map(|secret| (environment.id, secret)))
                .collect();
        }
        Ok(receipt)
    }

    async fn read_developer(
        &self,
        tx: &mut dyn IdentityTransaction,
        evidence: &BrowserWriteEvidence,
    ) -> Result<VerifiedActor, IdentityError> {
        let actor = self.read_actor(tx, evidence).await?;
        require_developer(&actor)?;
        Ok(actor)
    }
    async fn read_actor(
        &self,
        tx: &mut dyn IdentityTransaction,
        evidence: &BrowserWriteEvidence,
    ) -> Result<VerifiedActor, IdentityError> {
        let actor = read_current_actor(
            tx,
            evidence
                .current_center_digest
                .as_ref()
                .ok_or(IdentityError::Unauthenticated)?,
            now(self.clock.as_ref())?,
        )
        .await?;
        if !actor.account().developer_enabled && !actor.account().administrator {
            return Err(IdentityError::Forbidden);
        }
        Ok(actor)
    }
    async fn read_center_actor(
        &self,
        tx: &mut dyn IdentityTransaction,
        secret: &str,
    ) -> Result<VerifiedActor, IdentityError> {
        super::sso::validate_opaque(secret)?;
        let actor = read_current_actor(
            tx,
            &crate::domain::identity::account::SecretDigest::of(secret),
            now(self.clock.as_ref())?,
        )
        .await?;
        if !actor.account().developer_enabled && !actor.account().administrator {
            return Err(IdentityError::Forbidden);
        }
        Ok(actor)
    }
}
#[derive(serde::Serialize)]
pub struct ApplicationPage {
    pub items: Vec<ApplicationWorkspace>,
    pub next: Option<i32>,
}
pub(crate) fn require_membership(
    workspace: &ApplicationWorkspace,
    actor: &VerifiedActor,
) -> Result<(), IdentityError> {
    if workspace
        .members
        .iter()
        .any(|m| m.user_id == actor.account().user_id)
        && (actor.account().developer_enabled || actor.account().administrator)
    {
        Ok(())
    } else {
        Err(IdentityError::Forbidden)
    }
}
pub(crate) async fn finish_read<T>(
    tx: Box<dyn IdentityTransaction>,
    result: Result<T, IdentityError>,
    budget: &SecurityBudget,
) -> Result<T, IdentityError> {
    match result {
        Ok(value) => {
            tx.commit().await?;
            budget.remaining()?;
            Ok(value)
        }
        Err(error) => {
            let _ = tx.rollback().await;
            Err(error)
        }
    }
}
fn require_developer(actor: &VerifiedActor) -> Result<(), IdentityError> {
    if actor.account().developer_enabled {
        Ok(())
    } else {
        Err(IdentityError::Forbidden)
    }
}
struct PreparedEnvironment {
    id: Uuid,
    client_id: String,
    version_id: Uuid,
    secret: Option<IssuedSecret>,
    hash: Option<SecretToken>,
}
impl PreparedEnvironment {
    async fn new(config: &EnvironmentConfig) -> Result<Self, IdentityError> {
        let id = Uuid::new_v4();
        let entropy = config.permits_client_secret().then(rand::random);
        let secret = entropy.map(IssuedSecret::from_entropy);
        let hash = match entropy {
            Some(value) => {
                Some(hash_secret(&IssuedSecret::from_entropy(value).into_response()).await?)
            }
            None => None,
        };
        Ok(Self {
            id,
            client_id: format!("app_{id}"),
            version_id: Uuid::new_v4(),
            secret,
            hash,
        })
    }
    fn write(&self, config: &EnvironmentConfig) -> Result<NewEnvironmentWrite, IdentityError> {
        Ok(NewEnvironmentWrite {
            id: self.id,
            client_id: self.client_id.clone(),
            client_secret_hash: self
                .hash
                .as_ref()
                .map(|h| SecretToken::new(h.expose_secret().to_owned()))
                .transpose()?,
            initial_version_id: self.version_id,
            config: config.clone(),
        })
    }
}
struct Register<'a> {
    service: &'a ApplicationService,
    evidence: &'a BrowserWriteEvidence,
    request: &'a ValidatedRegistration,
    key: &'a str,
    request_id: Uuid,
    operation_id: Uuid,
    development: &'a PreparedEnvironment,
    production: &'a PreparedEnvironment,
    administrator: Option<&'a crate::domain::identity::registration::ValidatedAdminRegistration>,
}
impl Register<'_> {
    fn kind(&self) -> &'static str {
        if self.administrator.is_some() {
            "application.admin.register"
        } else {
            "application.register"
        }
    }
}
struct RegistrationOutcome {
    receipt: SafeOperationReceipt,
    created: bool,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Register<'_> {
    type Output = RegistrationOutcome;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        let mut keys = Vec::new();
        if let Some(admin) = self.administrator {
            keys.push(IdentityLockKey::User(admin.owner_user_id));
            for target in &admin.assignments {
                match target {
                    AssignmentTarget::User(id) => keys.push(IdentityLockKey::User(*id)),
                    AssignmentTarget::Group(id) => {
                        keys.push(IdentityLockKey::Group(*id));
                        let group = tx.group_workspace(*id).await?;
                        keys.extend(group.members.into_iter().map(IdentityLockKey::User));
                    }
                }
            }
        }
        let actor = authenticate_current_actor(
            tx,
            self.evidence
                .current_center_digest
                .as_ref()
                .ok_or(IdentityError::Unauthenticated)?,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(keys)?,
        )
        .await?;
        if self.administrator.is_some() {
            tx.authorize_identity_operation(
                &actor,
                crate::domain::identity::operation::IdentityOperation::ApplicationRegister,
            )
            .await?;
            if self
                .administrator
                .is_some_and(|admin| !admin.assignments.is_empty())
            {
                tx.authorize_identity_operation(
                    &actor,
                    crate::domain::identity::operation::IdentityOperation::ApplicationAssignment,
                )
                .await?;
            }
        } else {
            require_developer(&actor)?;
        }
        if let Some(receipt) = tx
            .operation_receipt(actor.account().user_id, self.kind(), self.key)
            .await?
        {
            registration_from_receipt(receipt.clone(), &self.request.request_hash)?;
            return Ok(RegistrationOutcome {
                receipt,
                created: false,
            });
        }
        let IdentityWriteResult::Application {
            application_id,
            development_client_id,
            production_client_id,
        } = tx
            .apply(IdentityWrite::RegisterApplication(Box::new(
                RegistrationWrite {
                    app_key: self.request.app_key.clone(),
                    name: self.request.name.clone(),
                    description: self.request.description.clone(),
                    management_mode: if self.administrator.is_some() {
                        ManagementMode::AdminConfigured
                    } else {
                        ManagementMode::DeveloperManaged
                    },
                    third_party: self.request.third_party,
                    owner_user_id: self
                        .administrator
                        .map_or(actor.account().user_id, |admin| admin.owner_user_id),
                    development: self.development.write(&self.request.development)?,
                    production: self.production.write(&self.request.production)?,
                },
            )))
            .await?
        else {
            return Err(IdentityError::DependencyUnavailable);
        };
        if let Some(admin) = self.administrator {
            for target in &admin.assignments {
                if let AssignmentTarget::User(user) = target
                    && tx
                        .load_account(*user)
                        .await?
                        .is_none_or(|account| !account.enabled)
                {
                    return Err(IdentityError::Forbidden);
                }
                tx.apply(IdentityWrite::SetAssignment {
                    application_id,
                    target: target.clone(),
                    present: true,
                    actor_id: actor.account().user_id,
                })
                .await?;
            }
            let snapshot = tx
                .approval_snapshot(
                    self.production.id,
                    self.production.version_id,
                    admin.accept_logout_limitation,
                )
                .await?;
            tx.apply(IdentityWrite::RecordAdminActivation {
                snapshot,
                actor_id: actor.account().user_id,
            })
            .await?;
            tx.apply(IdentityWrite::ActivateVersion {
                environment_id: self.production.id,
                version_id: self.production.version_id,
                expected_revision: 0,
            })
            .await?;
        }
        let mut summary = serde_json::json!({
            "application_id": application_id, "management_mode": if self.administrator.is_some() {ManagementMode::AdminConfigured}else {ManagementMode::DeveloperManaged},
            "environment_ids": [self.development.id.to_string(), self.production.id.to_string()],
            "development_client_id": development_client_id, "production_client_id": production_client_id,
            "version_id": self.development.version_id, "created": true,
        });
        if let Some(admin) = self.administrator {
            summary["production_version_id"] = serde_json::json!(self.production.version_id);
            summary["logout_limitation_accepted"] =
                serde_json::json!(admin.accept_logout_limitation);
        }
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: self.operation_id,
            actor_id: actor.account().user_id,
            operation_kind: self.kind(),
            idempotency_key: self.key.into(),
            request_hash: self.request.request_hash.clone(),
            resource_id: application_id.to_string(),
            summary: summary.clone(),
        }))
        .await?;
        let event_id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id: event_id,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: self.kind(),
            object_kind: "application",
            object_id: application_id.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: summary,
            operation_id: Some(self.operation_id),
            commit_marker: Some(event_id),
        })
        .await?;
        let receipt = tx
            .operation_receipt(actor.account().user_id, self.kind(), self.key)
            .await?
            .ok_or(IdentityError::DependencyUnavailable)?;
        Ok(RegistrationOutcome {
            receipt,
            created: true,
        })
    }
}
fn registration_from_receipt(
    receipt: SafeOperationReceipt,
    expected_hash: &str,
) -> Result<RegistrationReceipt, IdentityError> {
    if receipt.request_hash != expected_hash {
        return Err(IdentityError::Conflict);
    }
    #[derive(serde::Deserialize)]
    struct Summary {
        application_id: i32,
        management_mode: ManagementMode,
        environment_ids: [Uuid; 2],
        development_client_id: String,
        production_client_id: String,
        version_id: Uuid,
        #[serde(default)]
        production_version_id: Option<Uuid>,
    }
    let summary: Summary = serde_json::from_value(receipt.summary)
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    Ok(RegistrationReceipt {
        operation_id: receipt.operation_id,
        application_id: summary.application_id,
        management_mode: summary.management_mode,
        environments: vec![
            EnvironmentSummary {
                environment_id: summary.environment_ids[0],
                kind: AppEnvironmentKind::Development,
                client_id: summary.development_client_id,
                active_version_id: Some(summary.version_id),
            },
            EnvironmentSummary {
                environment_id: summary.environment_ids[1],
                kind: AppEnvironmentKind::Production,
                client_id: summary.production_client_id,
                active_version_id: summary.production_version_id,
            },
        ],
        newly_issued_secrets: vec![],
    })
}

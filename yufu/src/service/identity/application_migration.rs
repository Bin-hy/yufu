use super::*;
use crate::domain::identity::{
    migration::{AdoptLegacyApplication, LegacyClientSummary, ValidatedAdoption},
    operation::IdentityOperation,
};

#[derive(serde::Serialize)]
pub struct MigrationPage {
    pub items: Vec<LegacyClientSummary>,
    pub next: Option<i32>,
}
impl ApplicationService {
    pub async fn legacy_clients(
        &self,
        center_secret: &str,
        after: i32,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<MigrationPage, IdentityError> {
        validate_page(after, limit)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = self.read_center_actor(tx.as_mut(), center_secret).await?;
            tx.authorize_identity_operation(&actor, IdentityOperation::MigrationRead)
                .await?;
            legacy_page(tx.as_mut(), after, limit).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn scan_legacy(
        &self,
        evidence: &BrowserWriteEvidence,
        after: i32,
        limit: u32,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<MigrationPage, IdentityError> {
        validate_page(after, limit)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &ScanLegacy {
                service: self,
                evidence,
                after,
                limit,
                request_id,
            },
        )
        .await
    }
    pub async fn adopt_legacy(
        &self,
        evidence: &BrowserWriteEvidence,
        request: AdoptLegacyApplication,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<RegistrationReceipt, IdentityError> {
        validate_idempotency_key(key)?;
        let request = request.validate()?;
        // Retry responses must not regenerate secrets or depend on old client
        // usability; only the original actor's committed receipt is replayed.
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                evidence
                    .current_center_digest
                    .as_ref()
                    .ok_or(IdentityError::Unauthenticated)?,
                now(self.clock.as_ref())?,
            )
            .await?;
            authorize_adoption(tx.as_mut(), &actor, &request).await?;
            tx.operation_receipt(actor.account().user_id, "application.legacy.adopt", key)
                .await
        }
        .await;
        if let Some(receipt) = finish_read(tx, result, budget).await? {
            return registration_from_receipt(receipt, &request.request_hash);
        }
        let other_config = match request.retained_environment {
            AppEnvironmentKind::Development => &request.registration.application.production,
            AppEnvironmentKind::Production => &request.registration.application.development,
        };
        let other = PreparedEnvironment::new(other_config).await?;
        let retained_id = Uuid::new_v4();
        let retained_version = Uuid::new_v4();
        let write_budget = budget.renewed()?;
        let output = execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &AdoptLegacy {
                service: self,
                evidence,
                request: &request,
                key,
                request_id,
                operation_id: Uuid::new_v4(),
                other: &other,
                retained_id,
                retained_version,
            },
        )
        .await?;
        let mut receipt = registration_from_receipt(output.receipt, &request.request_hash)?;
        if output.created
            && let Some(secret) = other.secret
        {
            receipt.newly_issued_secrets.push((other.id, secret));
        }
        Ok(receipt)
    }
}
fn validate_page(after: i32, limit: u32) -> Result<(), IdentityError> {
    if after < 0 || !(1..=50).contains(&limit) {
        Err(IdentityError::InvalidInput)
    } else {
        Ok(())
    }
}
async fn legacy_page(
    tx: &mut dyn IdentityTransaction,
    after: i32,
    limit: u32,
) -> Result<MigrationPage, IdentityError> {
    let mut ids = tx.legacy_client_ids(after, limit + 1).await?;
    let next = if ids.len() > limit as usize {
        ids.truncate(limit as usize);
        ids.last().copied()
    } else {
        None
    };
    let mut items = Vec::new();
    for id in ids {
        items.push(tx.legacy_workspace(id).await?.summary);
    }
    Ok(MigrationPage { items, next })
}
async fn authorize_adoption(
    tx: &mut dyn IdentityTransaction,
    actor: &VerifiedActor,
    request: &ValidatedAdoption,
) -> Result<(), IdentityError> {
    tx.authorize_identity_operation(actor, IdentityOperation::MigrationAdopt)
        .await?;
    tx.authorize_identity_operation(actor, IdentityOperation::ApplicationConfigure)
        .await?;
    tx.authorize_identity_operation(actor, IdentityOperation::ApplicationActivate)
        .await?;
    if !request.registration.assignments.is_empty() {
        tx.authorize_identity_operation(actor, IdentityOperation::ApplicationAssignment)
            .await?;
    }
    Ok(())
}
struct ScanLegacy<'a> {
    service: &'a ApplicationService,
    evidence: &'a BrowserWriteEvidence,
    after: i32,
    limit: u32,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for ScanLegacy<'_> {
    type Output = MigrationPage;
    async fn apply(
        &self,
        tx: &mut dyn IdentityTransaction,
    ) -> Result<MigrationPage, IdentityError> {
        let actor = authenticate_current_actor(
            tx,
            self.evidence
                .current_center_digest
                .as_ref()
                .ok_or(IdentityError::Unauthenticated)?,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(vec![IdentityLockKey::Policy])?,
        )
        .await?;
        tx.authorize_identity_operation(&actor, IdentityOperation::MigrationScan)
            .await?;
        let page = legacy_page(tx, self.after, self.limit).await?;
        for item in &page.items {
            tx.apply(IdentityWrite::RecordLegacyScan {
                oauth_client_id: item.oauth_client_id,
            })
            .await?;
        }
        let event = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id: event,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: "application.legacy.scan",
            object_kind: "migration",
            object_id: self.after.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: serde_json::json!({"affected_count":page.items.len()}),
            operation_id: None,
            commit_marker: Some(event),
        })
        .await?;
        Ok(page)
    }
}
struct AdoptLegacy<'a> {
    service: &'a ApplicationService,
    evidence: &'a BrowserWriteEvidence,
    request: &'a ValidatedAdoption,
    key: &'a str,
    request_id: Uuid,
    operation_id: Uuid,
    other: &'a PreparedEnvironment,
    retained_id: Uuid,
    retained_version: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for AdoptLegacy<'_> {
    type Output = RegistrationOutcome;
    async fn apply(
        &self,
        tx: &mut dyn IdentityTransaction,
    ) -> Result<RegistrationOutcome, IdentityError> {
        tx.lock_identity_scope(&IdentityLockScope::new(vec![IdentityLockKey::Policy])?)
            .await?;
        let initial = tx.legacy_workspace(self.request.oauth_client_id).await?;
        let mut keys = initial.locks.keys().to_vec();
        if let Some(id) = self.request.existing_application_id {
            keys.push(IdentityLockKey::Application(id));
        }
        keys.push(IdentityLockKey::User(
            self.request.registration.owner_user_id,
        ));
        for target in &self.request.registration.assignments {
            match target {
                AssignmentTarget::User(id) => keys.push(IdentityLockKey::User(*id)),
                AssignmentTarget::Group(id) => {
                    keys.push(IdentityLockKey::Group(*id));
                    keys.extend(
                        tx.group_workspace(*id)
                            .await?
                            .members
                            .into_iter()
                            .map(IdentityLockKey::User),
                    );
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
        authorize_adoption(tx, &actor, self.request).await?;
        if let Some(receipt) = tx
            .operation_receipt(
                actor.account().user_id,
                "application.legacy.adopt",
                self.key,
            )
            .await?
        {
            registration_from_receipt(receipt.clone(), &self.request.request_hash)?;
            return Ok(RegistrationOutcome {
                receipt,
                created: false,
            });
        }
        let current = tx.legacy_workspace(self.request.oauth_client_id).await?;
        if current.summary.snapshot != initial.summary.snapshot
            || current.locks.keys() != initial.locks.keys()
        {
            return Err(IdentityError::Conflict);
        }
        let registration = &self.request.registration;
        let input = &registration.application;
        let retained_config = match self.request.retained_environment {
            AppEnvironmentKind::Development => &input.development,
            AppEnvironmentKind::Production => &input.production,
        };
        let retained = NewEnvironmentWrite {
            id: self.retained_id,
            client_id: current.summary.client_id.clone(),
            client_secret_hash: None,
            initial_version_id: self.retained_version,
            config: retained_config.clone(),
        };
        let (development, production) = match self.request.retained_environment {
            AppEnvironmentKind::Development => (retained, self.other.write(&input.production)?),
            AppEnvironmentKind::Production => (self.other.write(&input.development)?, retained),
        };
        let dev_id = development.id;
        let prod_id = production.id;
        let dev_version = development.initial_version_id;
        let prod_version = production.initial_version_id;
        let IdentityWriteResult::Application {
            application_id,
            development_client_id,
            production_client_id,
        } = tx
            .apply(IdentityWrite::AdoptLegacyApplication(Box::new(
                LegacyAdoptionWrite {
                    oauth_client_id: self.request.oauth_client_id,
                    expected_snapshot: self.request.expected_snapshot.clone(),
                    existing_application_id: self.request.existing_application_id,
                    retained_environment: self.request.retained_environment,
                    actor_id: actor.account().user_id,
                    registration: RegistrationWrite {
                        app_key: input.app_key.clone(),
                        name: input.name.clone(),
                        description: input.description.clone(),
                        management_mode: ManagementMode::AdminConfigured,
                        third_party: input.third_party,
                        owner_user_id: registration.owner_user_id,
                        development,
                        production,
                    },
                },
            )))
            .await?
        else {
            return Err(IdentityError::DependencyUnavailable);
        };
        for target in &registration.assignments {
            if let AssignmentTarget::User(id) = target
                && tx.load_account(*id).await?.is_none_or(|a| !a.enabled)
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
            .approval_snapshot(prod_id, prod_version, registration.accept_logout_limitation)
            .await?;
        tx.apply(IdentityWrite::RecordAdminActivation {
            snapshot,
            actor_id: actor.account().user_id,
        })
        .await?;
        tx.apply(IdentityWrite::ActivateVersion {
            environment_id: prod_id,
            version_id: prod_version,
            expected_revision: 0,
        })
        .await?;
        let summary = serde_json::json!({"application_id":application_id,"management_mode":ManagementMode::AdminConfigured,"environment_ids":[dev_id.to_string(),prod_id.to_string()],"development_client_id":development_client_id,"production_client_id":production_client_id,"version_id":dev_version,"production_version_id":prod_version,"logout_limitation_accepted":registration.accept_logout_limitation,"created":true});
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: self.operation_id,
            actor_id: actor.account().user_id,
            operation_kind: "application.legacy.adopt",
            idempotency_key: self.key.into(),
            request_hash: self.request.request_hash.clone(),
            resource_id: application_id.to_string(),
            summary: summary.clone(),
        }))
        .await?;
        let event = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id: event,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: "application.legacy.adopt",
            object_kind: "application",
            object_id: application_id.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: summary,
            operation_id: Some(self.operation_id),
            commit_marker: Some(event),
        })
        .await?;
        let receipt = tx
            .operation_receipt(
                actor.account().user_id,
                "application.legacy.adopt",
                self.key,
            )
            .await?
            .ok_or(IdentityError::DependencyUnavailable)?;
        Ok(RegistrationOutcome {
            receipt,
            created: true,
        })
    }
}

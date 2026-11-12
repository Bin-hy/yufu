//! Independent server credentials for querying a public client's token status.
use super::{
    account::{authenticate_current_actor, read_current_actor},
    application::{finish_read, require_membership},
    browser::BrowserWriteEvidence,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::{
    domain::identity::{account::IssuedSecret, error::IdentityError},
    service::security::repository::now,
};
use auth::security::{
    budget::{Clock, SecurityBudget},
    identity::SecretToken,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReaderManagement {
    Developer,
    Administrator,
}
pub struct ReaderService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
    management: ReaderManagement,
}
pub struct IssuedReader {
    pub id: Uuid,
    pub reader_id: String,
    pub secret: Option<IssuedSecret>,
    pub operation_id: Uuid,
    pub revision: u64,
}
pub struct ReaderChange {
    pub operation_id: Uuid,
    pub revision: u64,
    pub secret: Option<IssuedSecret>,
}
#[derive(Clone, Copy)]
pub struct ReaderLocation {
    pub application_id: i32,
    pub environment_id: Uuid,
}
#[derive(serde::Serialize)]
pub struct ReaderPage {
    pub items: Vec<ReaderSummary>,
    pub next: Option<Uuid>,
}
impl ReaderService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            repository,
            clock,
            management: ReaderManagement::Developer,
        }
    }
    pub fn with_management(mut self, management: ReaderManagement) -> Self {
        self.management = management;
        self
    }
    pub async fn list(
        &self,
        center_secret: &str,
        location: ReaderLocation,
        after: Option<Uuid>,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<ReaderPage, IdentityError> {
        if location.application_id <= 0 || limit == 0 || limit > 50 {
            return Err(IdentityError::InvalidInput);
        }
        super::sso::validate_opaque(center_secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &crate::domain::identity::account::SecretDigest::of(center_secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            let workspace = tx.application_workspace(location.application_id).await?;
            match self.management {
                ReaderManagement::Developer => require_membership(&workspace, &actor)?,
                ReaderManagement::Administrator => {
                    tx.authorize_identity_operation(
                        &actor,
                        crate::domain::identity::operation::IdentityOperation::ReaderRead,
                    )
                    .await?
                }
            }
            if !workspace
                .environments
                .iter()
                .any(|e| e.environment_id == location.environment_id)
            {
                return Err(IdentityError::Forbidden);
            }
            let mut items = tx
                .reader_summaries(location.environment_id, after, limit + 1)
                .await?;
            let next = if items.len() > limit as usize {
                items.truncate(limit as usize);
                items.last().map(|r| r.id)
            } else {
                None
            };
            Ok(ReaderPage { items, next })
        }
        .await;
        finish_read(tx, result, budget).await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        evidence: &BrowserWriteEvidence,
        location: ReaderLocation,
        audience: &str,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedReader, IdentityError> {
        if location.application_id <= 0 || audience.is_empty() || audience.len() > 512 {
            return Err(IdentityError::InvalidInput);
        }
        crate::domain::identity::registration::validate_idempotency_key(key)?;
        let id = Uuid::new_v4();
        let reader_id = format!("reader_{id}");
        let entropy = rand::random();
        let secret = IssuedSecret::from_entropy(entropy);
        let plaintext = IssuedSecret::from_entropy(entropy).into_response();
        let hash = hash_secret(&plaintext).await?;
        let write_budget = budget.renewed()?;
        let outcome = execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &ReaderMutation {
                service: self,
                evidence,
                environment_id: location.environment_id,
                application_id: location.application_id,
                request_id,
                key,
                operation_id: Uuid::new_v4(),
                reader_id: &reader_id,
                change: Change::Create {
                    id,
                    audience,
                    hash: &hash,
                },
            },
        )
        .await?;
        let summary = &outcome.receipt.summary;
        Ok(IssuedReader {
            id: summary["reader_resource_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or(IdentityError::DependencyUnavailable)?,
            reader_id: summary["reader_id"]
                .as_str()
                .ok_or(IdentityError::DependencyUnavailable)?
                .into(),
            secret: outcome.changed.then_some(secret),
            revision: summary["revision"]
                .as_u64()
                .ok_or(IdentityError::DependencyUnavailable)?,
            operation_id: outcome.receipt.operation_id,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn change(
        &self,
        evidence: &BrowserWriteEvidence,
        reader_id: &str,
        location: ReaderLocation,
        expected_revision: u64,
        enabled: bool,
        reset_secret: bool,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<ReaderChange, IdentityError> {
        if location.application_id <= 0 || reader_id.is_empty() || reader_id.len() > 128 {
            return Err(IdentityError::InvalidInput);
        }
        crate::domain::identity::registration::validate_idempotency_key(key)?;
        let entropy = reset_secret.then(rand::random);
        let secret = entropy.map(IssuedSecret::from_entropy);
        let plaintext = entropy.map(|v| IssuedSecret::from_entropy(v).into_response());
        let hash = match plaintext.as_deref() {
            Some(value) => Some(hash_secret(value).await?),
            None => None,
        };
        let write_budget = budget.renewed()?;
        let outcome = execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &ReaderMutation {
                service: self,
                evidence,
                environment_id: location.environment_id,
                application_id: location.application_id,
                request_id,
                key,
                operation_id: Uuid::new_v4(),
                reader_id,
                change: Change::Update {
                    expected_revision,
                    enabled,
                    hash: hash.as_ref(),
                },
            },
        )
        .await?;
        Ok(ReaderChange {
            revision: outcome.receipt.summary["revision"]
                .as_u64()
                .ok_or(IdentityError::DependencyUnavailable)?,
            secret: if outcome.changed { secret } else { None },
            operation_id: outcome.receipt.operation_id,
        })
    }
}
pub(crate) async fn hash_secret(plaintext: &str) -> Result<SecretToken, IdentityError> {
    let plaintext = plaintext.to_owned();
    let hash = super::password::hash(plaintext).await?;
    Ok(SecretToken::new(hash)?)
}
enum Change<'a> {
    Create {
        id: Uuid,
        audience: &'a str,
        hash: &'a SecretToken,
    },
    Update {
        expected_revision: u64,
        enabled: bool,
        hash: Option<&'a SecretToken>,
    },
}
struct ReaderMutation<'a> {
    service: &'a ReaderService,
    evidence: &'a BrowserWriteEvidence,
    environment_id: Uuid,
    application_id: i32,
    request_id: Uuid,
    key: &'a str,
    operation_id: Uuid,
    reader_id: &'a str,
    change: Change<'a>,
}
struct ReaderMutationOutcome {
    receipt: SafeOperationReceipt,
    changed: bool,
}
impl ReaderMutation<'_> {
    fn action(&self) -> &'static str {
        match self.change {
            Change::Create { .. } => "reader.create",
            Change::Update { hash: Some(_), .. } => "reader.secret.reset",
            Change::Update { .. } => "reader.enabled.change",
        }
    }
    fn request_hash(&self) -> Result<String, IdentityError> {
        use sha2::{Digest, Sha256};
        let values = match self.change {
            Change::Create { audience, .. } => serde_json::json!({"audience":audience}),
            Change::Update {
                expected_revision,
                enabled,
                hash,
            } => {
                serde_json::json!({"reader_id":self.reader_id,"revision":expected_revision,"enabled":enabled,"reset_secret":hash.is_some()})
            }
        };
        Ok(hex::encode(Sha256::digest(
            serde_json::to_vec(&(
                self.application_id,
                self.environment_id,
                self.service.management == ReaderManagement::Administrator,
                values,
            ))
            .map_err(|_| IdentityError::InvalidInput)?,
        )))
    }
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for ReaderMutation<'_> {
    type Output = ReaderMutationOutcome;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        let client = tx
            .environment_client(self.environment_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let initial = tx.authorization_target(&client.client_id, None).await?;
        if initial.application_id != self.application_id {
            return Err(IdentityError::Forbidden);
        }
        let reader = match self.change {
            Change::Create { .. } => None,
            Change::Update { .. } => Some(
                tx.introspection_reader(self.reader_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?,
            ),
        };
        if reader
            .as_ref()
            .is_some_and(|r| r.environment_id != self.environment_id)
        {
            return Err(IdentityError::Forbidden);
        }
        let mut keys = initial.locks.keys().to_vec();
        if let Some(reader) = &reader {
            keys.push(IdentityLockKey::Reader(reader.id));
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
        let current = tx.authorization_target(&client.client_id, None).await?;
        if current.locks.keys() != initial.locks.keys()
            || current.config_version_id != initial.config_version_id
            || current.application_id != self.application_id
        {
            return Err(IdentityError::Conflict);
        }
        match self.service.management {
            ReaderManagement::Developer => {
                if !tx
                    .application_maintainer(current.application_id, actor.account().user_id)
                    .await?
                    || !(actor.account().developer_enabled || actor.account().administrator)
                {
                    return Err(IdentityError::Forbidden);
                }
            }
            ReaderManagement::Administrator => {
                tx.authorize_identity_operation(
                    &actor,
                    match self.change {
                        Change::Create { .. } => {
                            crate::domain::identity::operation::IdentityOperation::ReaderCreate
                        }
                        Change::Update { .. } => {
                            crate::domain::identity::operation::IdentityOperation::ReaderChange
                        }
                    },
                )
                .await?
            }
        }
        let request_hash = self.request_hash()?;
        if let Some(receipt) = tx
            .operation_receipt(actor.account().user_id, self.action(), self.key)
            .await?
        {
            if receipt.request_hash != request_hash {
                return Err(IdentityError::Conflict);
            }
            return Ok(ReaderMutationOutcome {
                receipt,
                changed: false,
            });
        }
        let (write, action, id, revision) = match self.change {
            Change::Create { id, audience, hash } => {
                if !current.enabled
                    || !current.config.client_kind.permits_introspection_reader()
                    || current.config.access_audience.as_str() != audience
                {
                    return Err(IdentityError::AccessDenied);
                }
                (
                    IdentityWrite::CreateIntrospectionReader {
                        id,
                        reader_id: self.reader_id.into(),
                        secret_hash: SecretToken::new(hash.expose_secret().into())?,
                        environment_id: self.environment_id,
                        approved_audience: audience.into(),
                        actor_id: actor.account().user_id,
                    },
                    "reader.create",
                    id,
                    1,
                )
            }
            Change::Update {
                expected_revision,
                enabled,
                hash,
            } => {
                let reader = tx
                    .introspection_reader(self.reader_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                if reader.environment_id != self.environment_id
                    || u64::try_from(reader.secret_revision).ok() != Some(expected_revision)
                {
                    return Err(IdentityError::Conflict);
                }
                if enabled
                    && (!current.enabled
                        || !current.config.client_kind.permits_introspection_reader()
                        || current.config.access_audience.as_str() != reader.approved_audience)
                {
                    return Err(IdentityError::AccessDenied);
                }
                (
                    IdentityWrite::UpdateIntrospectionReader {
                        id: reader.id,
                        environment_id: self.environment_id,
                        expected_revision,
                        enabled,
                        secret_hash: hash
                            .map(|v| SecretToken::new(v.expose_secret().into()))
                            .transpose()?,
                    },
                    if hash.is_some() {
                        "reader.secret.reset"
                    } else {
                        "reader.enabled.change"
                    },
                    reader.id,
                    expected_revision
                        .checked_add(1)
                        .ok_or(IdentityError::InvalidInput)?,
                )
            }
        };
        tx.apply(write).await?;
        let summary = serde_json::json!({"reader_resource_id":id,"reader_id":self.reader_id,"application_id":self.application_id,"environment_id":self.environment_id,"revision":revision});
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: self.operation_id,
            actor_id: actor.account().user_id,
            operation_kind: action,
            idempotency_key: self.key.into(),
            request_hash,
            resource_id: id.to_string(),
            summary: summary.clone(),
        }))
        .await?;
        let event_id = Uuid::new_v4();
        tx.append_audit(&AuditEvent{id:event_id,request_id:self.request_id,actor_id:Some(actor.account().user_id),action,
            object_kind:"introspection_reader",object_id:id.to_string(),result:AuditResult::Success,reason:None,
            change_summary:serde_json::json!({"environment_id":self.environment_id,"revision":revision}),operation_id:Some(self.operation_id),commit_marker:Some(event_id)}).await?;
        let receipt = tx
            .operation_receipt(actor.account().user_id, action, self.key)
            .await?
            .ok_or(IdentityError::DependencyUnavailable)?;
        Ok(ReaderMutationOutcome {
            receipt,
            changed: true,
        })
    }
}

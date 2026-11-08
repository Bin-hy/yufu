//! Primary-only, deadline-bounded security transactions. No replica or cache fallback.
use std::{future::Future, sync::Arc};

use auth::security::{
    budget::{Clock, SecurityBudget},
    credential::CredentialRef,
    error::SecurityError,
    identity::{IdentityBinding, PrincipalKind, PrincipalRef, SecretToken, TokenUse},
    message::SignedDocument,
    policy::{CredentialBounds, OperationId, ResourceRef, ResourceTarget, ScopedOperation},
};
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    DbErr, EntityTrait, IsolationLevel, QueryFilter, Statement, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entity::{
    security_binding::{self, device, gateway_key},
    security_credential as credential, security_grant as grant, security_permit as permit,
    security_refresh_family as family, security_session as session,
};

pub fn refresh_digest(secret: &SecretToken) -> [u8; 32] {
    Sha256::digest(secret.expose_secret().as_bytes()).into()
}
/// Trusted registration and reads share a canonical SPKI DER fingerprint.
pub fn gateway_key_fingerprint(pem: &str) -> Result<[u8; 32], SecurityError> {
    use rsa::{
        pkcs8::{DecodePublicKey, EncodePublicKey},
        traits::PublicKeyParts,
    };
    if pem.len() > 16_384 {
        return Err(SecurityError::InvalidCredential);
    }
    let public = rsa::RsaPublicKey::from_public_key_pem(pem)
        .map_err(|_| SecurityError::InvalidCredential)?;
    if !(2048..=8192).contains(&public.n().bits()) {
        return Err(SecurityError::InvalidCredential);
    }
    let der = public
        .to_public_key_der()
        .map_err(|_| SecurityError::InvalidCredential)?;
    Ok(Sha256::digest(der.as_bytes()).into())
}
fn unavailable(_: DbErr) -> SecurityError {
    SecurityError::DependencyUnavailable
}
pub(super) fn decode<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, SecurityError> {
    serde_json::from_value(value).map_err(|_| SecurityError::DependencyUnavailable)
}
pub(super) fn now(clock: &dyn Clock) -> Result<u64, SecurityError> {
    let reading = clock.now()?;
    if !reading.synchronized {
        return Err(SecurityError::DependencyUnavailable);
    }
    Ok(reading.unix_millis)
}
fn validate_application(application: &str) -> Result<(), SecurityError> {
    ResourceRef::new(application, ResourceTarget::Application).map(|_| ())
}
fn validate_binding(binding: &IdentityBinding, application: &str) -> Result<(), SecurityError> {
    let target = match binding {
        IdentityBinding::None => ResourceTarget::Application,
        IdentityBinding::Gateway { gateway_id } => ResourceTarget::Gateway {
            gateway_id: gateway_id.clone(),
        },
        IdentityBinding::Device {
            gateway_id,
            device_id,
        } => ResourceTarget::Device {
            gateway_id: gateway_id.clone(),
            device_id: device_id.clone(),
        },
    };
    ResourceRef::new(application, target)
        .map(|_| ())
        .map_err(|_| SecurityError::DependencyUnavailable)
}

#[derive(Clone)]
pub struct ServiceBindingKey {
    pub principal: PrincipalRef,
    pub token_use: TokenUse,
    pub application: String,
    pub audience: String,
}
#[derive(Debug, Clone)]
pub struct ServiceBinding {
    pub(super) identity: IdentityBinding,
    bounds: CredentialBounds,
}
impl ServiceBinding {
    pub fn bounds(&self) -> &CredentialBounds {
        &self.bounds
    }
    pub fn identity(&self) -> &IdentityBinding {
        &self.identity
    }
}

pub(super) struct RegisteredState {
    pub credential: credential::Model,
    pub grant: grant::Model,
    pub session: Option<session::Model>,
    pub family: Option<family::Model>,
    pub service_binding: Option<ServiceBinding>,
    pub device_binding: Option<device::Model>,
}
/// Trusted storage facts only, not an authentication or policy decision.
pub struct CredentialSnapshot {
    reference: CredentialRef,
    pub(super) checked_at: u64,
    pub(super) current: RegisteredState,
    pub(super) parent: Option<RegisteredState>,
}
impl CredentialSnapshot {
    pub fn reference(&self) -> &CredentialRef {
        &self.reference
    }
    pub fn checked_at(&self) -> u64 {
        self.checked_at
    }
}
impl std::fmt::Debug for CredentialSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialSnapshot")
            .field("reference", &self.reference)
            .field("checked_at", &self.checked_at)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
pub trait SecurityRepository: Send + Sync {
    async fn begin(
        &self,
        budget: &SecurityBudget,
    ) -> Result<Box<dyn SecurityTransaction>, SecurityError>;
}
/// A single consistent primary snapshot shared by lifecycle and permission reads.
/// Writes/conditional consumption are added by their lifecycle tasks, using the
/// same transaction boundary. Queries retain consumed refresh rows for replay detection.
#[async_trait::async_trait]
pub trait SecurityTransaction: Send {
    fn checked_at(&self) -> u64;
    async fn credential(
        &mut self,
        reference: &CredentialRef,
    ) -> Result<Option<CredentialSnapshot>, SecurityError>;
    async fn refresh(
        &mut self,
        issuer: &str,
        secret: &SecretToken,
    ) -> Result<Option<CredentialSnapshot>, SecurityError>;
    async fn service_binding(
        &mut self,
        key: &ServiceBindingKey,
    ) -> Result<Option<ServiceBinding>, SecurityError>;
    async fn device_binding(
        &mut self,
        principal: &PrincipalRef,
        application: &str,
    ) -> Result<Option<device::Model>, SecurityError>;
    async fn gateway_key(
        &mut self,
        issuer: &str,
        application: &str,
        gateway_id: &str,
        kid: &str,
    ) -> Result<Option<gateway_key::Model>, SecurityError>;
    async fn permit(
        &mut self,
        application: &str,
        request_id: Uuid,
    ) -> Result<Option<permit::Model>, SecurityError>;
    async fn commit(self: Box<Self>) -> Result<(), SecurityError>;
    async fn rollback(self: Box<Self>) -> Result<(), SecurityError>;
}

pub struct PgSecurityRepository {
    primary: DatabaseConnection,
    clock: Arc<dyn Clock>,
}
impl PgSecurityRepository {
    pub fn new(primary: DatabaseConnection, clock: Arc<dyn Clock>) -> Self {
        Self { primary, clock }
    }
}
struct PgSecurityTransaction {
    transaction: DatabaseTransaction,
    budget: SecurityBudget,
    clock: Arc<dyn Clock>,
    checked_at: u64,
}
#[async_trait::async_trait]
impl SecurityRepository for PgSecurityRepository {
    async fn begin(
        &self,
        budget: &SecurityBudget,
    ) -> Result<Box<dyn SecurityTransaction>, SecurityError> {
        let checked_at = now(self.clock.as_ref())?;
        if self.primary.get_database_backend() != DbBackend::Postgres {
            return Err(SecurityError::DependencyUnavailable);
        }
        let txn = budget
            .run(self.primary.begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadWrite),
            ))
            .await?
            .map_err(unavailable)?;
        let row=budget.run(txn.query_one_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT pg_is_in_recovery() AS standby, current_setting('transaction_read_only')::boolean AS readonly".to_owned())))
            .await?.map_err(unavailable)?.ok_or(SecurityError::DependencyUnavailable)?;
        if row.try_get::<bool>("", "standby").map_err(unavailable)?
            || row.try_get::<bool>("", "readonly").map_err(unavailable)?
        {
            return Err(SecurityError::DependencyUnavailable);
        }
        let timeout = budget.remaining()?.as_millis().max(1).to_string();
        budget
            .run(txn.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('statement_timeout', $1, true)",
                [timeout.into()],
            )))
            .await?
            .map_err(unavailable)?;
        Ok(Box::new(PgSecurityTransaction {
            transaction: txn,
            budget: budget.clone(),
            clock: self.clock.clone(),
            checked_at,
        }))
    }
}
impl PgSecurityTransaction {
    async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, DbErr>>,
    ) -> Result<T, SecurityError> {
        self.budget.run(future).await?.map_err(unavailable)
    }
    async fn registered(
        &mut self,
        row: credential::Model,
    ) -> Result<RegisteredState, SecurityError> {
        let grant = self
            .run(grant::Entity::find_by_id(row.grant_id).one(&self.transaction))
            .await?
            .ok_or(SecurityError::DependencyUnavailable)?;
        let session = match row.session_id {
            Some(id) => {
                self.run(session::Entity::find_by_id(id).one(&self.transaction))
                    .await?
            }
            None => None,
        };
        let family = match row.family_id {
            Some(id) => {
                self.run(family::Entity::find_by_id(id).one(&self.transaction))
                    .await?
            }
            None => None,
        };
        let kind: PrincipalKind = decode(row.principal_kind.clone().into())?;
        let token_use: TokenUse = decode(row.token_use.clone().into())?;
        let principal = PrincipalRef::new(&row.issuer, kind, &row.subject)?;
        let service_binding =
            if kind == PrincipalKind::Machine && token_use != TokenUse::GatewayOperation {
                self.service_binding(&ServiceBindingKey {
                    principal: principal.clone(),
                    token_use,
                    application: row.application.clone(),
                    audience: row.audience.clone(),
                })
                .await?
            } else {
                None
            };
        let device_binding = if kind == PrincipalKind::Device {
            self.device_binding(&principal, &row.application).await?
        } else {
            None
        };
        Ok(RegisteredState {
            credential: row,
            grant,
            session,
            family,
            service_binding,
            device_binding,
        })
    }
    async fn snapshot(
        &mut self,
        row: credential::Model,
    ) -> Result<CredentialSnapshot, SecurityError> {
        let reference = CredentialRef::new(&row.issuer, &row.jti)?;
        let parent = match &row.revocation_parent_jti {
            Some(jti) => match self
                .run(
                    credential::Entity::find_by_id((row.issuer.clone(), jti.clone()))
                        .one(&self.transaction),
                )
                .await?
            {
                Some(parent) => Some(self.registered(parent).await?),
                None => None,
            },
            None => None,
        };
        let current = self.registered(row).await?;
        Ok(CredentialSnapshot {
            reference,
            checked_at: self.checked_at,
            current,
            parent,
        })
    }
}
#[async_trait::async_trait]
impl SecurityTransaction for PgSecurityTransaction {
    fn checked_at(&self) -> u64 {
        self.checked_at
    }
    async fn credential(
        &mut self,
        reference: &CredentialRef,
    ) -> Result<Option<CredentialSnapshot>, SecurityError> {
        match self
            .run(
                credential::Entity::find_by_id((
                    reference.issuer().to_owned(),
                    reference.jti().to_owned(),
                ))
                .one(&self.transaction),
            )
            .await?
        {
            Some(row) => self.snapshot(row).await.map(Some),
            None => Ok(None),
        }
    }
    async fn refresh(
        &mut self,
        issuer: &str,
        secret: &SecretToken,
    ) -> Result<Option<CredentialSnapshot>, SecurityError> {
        PrincipalRef::new(issuer, PrincipalKind::User, "refresh-lookup")?;
        if secret.expose_secret().len() > 65_536 {
            return Err(SecurityError::InvalidCredential);
        }
        let row = self
            .run(
                credential::Entity::find()
                    .filter(credential::Column::Issuer.eq(issuer))
                    .filter(credential::Column::CredentialKind.eq("refresh"))
                    .filter(credential::Column::SecretDigest.eq(refresh_digest(secret).to_vec()))
                    .one(&self.transaction),
            )
            .await?;
        match row {
            Some(row) => self.snapshot(row).await.map(Some),
            None => Ok(None),
        }
    }
    async fn service_binding(
        &mut self,
        key: &ServiceBindingKey,
    ) -> Result<Option<ServiceBinding>, SecurityError> {
        if key.principal.kind() != PrincipalKind::Machine
            || !matches!(
                key.token_use,
                TokenUse::SecurityService | TokenUse::PermissionSync
            )
        {
            return Ok(None);
        }
        validate_application(&key.application)?;
        let token_use = match key.token_use {
            TokenUse::SecurityService => "security_service",
            _ => "permission_sync",
        };
        let row = self
            .run(
                security_binding::Entity::find()
                    .filter(security_binding::Column::Issuer.eq(key.principal.issuer()))
                    .filter(security_binding::Column::ClientId.eq(key.principal.subject()))
                    .filter(security_binding::Column::TokenUse.eq(token_use))
                    .filter(security_binding::Column::Application.eq(&key.application))
                    .filter(security_binding::Column::Audience.eq(&key.audience))
                    .filter(security_binding::Column::Enabled.eq(true))
                    .one(&self.transaction),
            )
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let grants: Vec<ScopedOperation> = decode(row.allowed_operations)?;
        if grants
            .iter()
            .flat_map(|g| g.resources())
            .any(|s| s.application() != row.application)
        {
            return Err(SecurityError::DependencyUnavailable);
        }
        let identity: IdentityBinding = decode(row.identity_binding)?;
        validate_binding(&identity, &row.application)?;
        if matches!(identity, IdentityBinding::Device { .. }) {
            return Err(SecurityError::DependencyUnavailable);
        }
        Ok(Some(ServiceBinding {
            identity,
            bounds: CredentialBounds::new(grants),
        }))
    }
    async fn device_binding(
        &mut self,
        principal: &PrincipalRef,
        application: &str,
    ) -> Result<Option<device::Model>, SecurityError> {
        if principal.kind() != PrincipalKind::Device {
            return Ok(None);
        }
        validate_application(application)?;
        let row = self
            .run(
                device::Entity::find()
                    .filter(device::Column::Issuer.eq(principal.issuer()))
                    .filter(device::Column::Subject.eq(principal.subject()))
                    .filter(device::Column::Application.eq(application))
                    .filter(device::Column::Enabled.eq(true))
                    .one(&self.transaction),
            )
            .await?;
        if let Some(row) = &row {
            ResourceRef::new(
                application,
                ResourceTarget::Device {
                    gateway_id: row.gateway_id.clone(),
                    device_id: row.device_id.clone(),
                },
            )?;
            if row.binding_revision.trim().is_empty() {
                return Err(SecurityError::DependencyUnavailable);
            }
        }
        Ok(row)
    }
    async fn gateway_key(
        &mut self,
        issuer: &str,
        application: &str,
        gateway_id: &str,
        kid: &str,
    ) -> Result<Option<gateway_key::Model>, SecurityError> {
        PrincipalRef::new(issuer, PrincipalKind::Machine, gateway_id)?;
        ResourceRef::new(
            application,
            ResourceTarget::GatewayKey {
                gateway_id: gateway_id.into(),
                kid: kid.into(),
            },
        )?;
        let row = self
            .run(
                gateway_key::Entity::find()
                    .filter(gateway_key::Column::Issuer.eq(issuer))
                    .filter(gateway_key::Column::Application.eq(application))
                    .filter(gateway_key::Column::GatewayId.eq(gateway_id))
                    .filter(gateway_key::Column::Kid.eq(kid))
                    .filter(gateway_key::Column::Enabled.eq(true))
                    .one(&self.transaction),
            )
            .await?;
        if let Some(row) = &row {
            let now = now(self.clock.as_ref())?;
            if u64::try_from(row.valid_from).map_err(|_| SecurityError::DependencyUnavailable)?
                > now.saturating_sub(1000)
                || u64::try_from(row.valid_until)
                    .map_err(|_| SecurityError::DependencyUnavailable)?
                    <= now.saturating_add(1000)
            {
                return Ok(None);
            }
            let fingerprint = gateway_key_fingerprint(&row.public_key)
                .map_err(|_| SecurityError::DependencyUnavailable)?;
            if fingerprint.as_slice() != row.public_key_fingerprint.as_slice() {
                return Err(SecurityError::DependencyUnavailable);
            }
        }
        Ok(row)
    }
    async fn permit(
        &mut self,
        application: &str,
        request_id: Uuid,
    ) -> Result<Option<permit::Model>, SecurityError> {
        validate_application(application)?;
        let row = self
            .run(
                permit::Entity::find()
                    .filter(permit::Column::Application.eq(application))
                    .filter(permit::Column::RequestId.eq(request_id))
                    .one(&self.transaction),
            )
            .await?;
        if let Some(row) = &row {
            let _: PrincipalRef = decode(row.actor.clone())?;
            let _: PrincipalRef = decode(row.presenter.clone())?;
            let resource: ResourceRef = decode(row.resource.clone())?;
            OperationId::new(&row.operation)?;
            CredentialRef::new(&row.issuer, &row.jti)?;
            SignedDocument::new(row.signed_permit.expose_secret().to_owned())?;
            if resource.application() != row.application {
                return Err(SecurityError::DependencyUnavailable);
            }
        }
        Ok(row)
    }
    async fn commit(self: Box<Self>) -> Result<(), SecurityError> {
        self.budget
            .run(self.transaction.commit())
            .await?
            .map_err(unavailable)
    }
    async fn rollback(self: Box<Self>) -> Result<(), SecurityError> {
        self.budget
            .run(self.transaction.rollback())
            .await?
            .map_err(unavailable)
    }
}

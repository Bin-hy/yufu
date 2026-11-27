//! Admin-side provider configuration and login statistics.
use super::secret::SecretCipher;
use super::service::append_audit;
use crate::domain::identity::{
    account::SecretDigest,
    error::IdentityError,
    external::{
        ExternalProviderAdminView, ExternalProviderKey, ExternalProviderStats,
        RotateExternalProviderSecret, UpdateExternalProvider,
    },
    operation::IdentityOperation,
};
use crate::service::identity::{
    account::{authenticate_current_actor, read_current_actor},
    repository::{AuditResult, IdentityLockScope, IdentityRepository, IdentityTransaction},
    sso::validate_opaque,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::service::security::repository::now;
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsWindow {
    H24,
    D7,
}

impl StatsWindow {
    pub fn parse(value: Option<&str>) -> Result<Self, IdentityError> {
        match value.unwrap_or("24h") {
            "24h" => Ok(Self::H24),
            "7d" => Ok(Self::D7),
            _ => Err(IdentityError::InvalidInput),
        }
    }

    fn range(self, now_millis: u64) -> (u64, u64) {
        let span = match self {
            Self::H24 => 24 * 3_600_000,
            Self::D7 => 7 * 24 * 3_600_000,
        };
        (now_millis.saturating_sub(span), now_millis)
    }
}

pub struct ExternalProviderAdminService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
    cipher: Option<Arc<SecretCipher>>,
}

impl ExternalProviderAdminService {
    pub fn new(
        repository: Arc<dyn IdentityRepository>,
        clock: Arc<dyn Clock>,
        cipher: Option<Arc<SecretCipher>>,
    ) -> Self {
        Self {
            repository,
            clock,
            cipher,
        }
    }

    pub async fn list(
        &self,
        user_secret: &str,
        budget: &SecurityBudget,
    ) -> Result<Vec<ExternalProviderAdminView>, IdentityError> {
        validate_opaque(user_secret)?;
        let mut transaction = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                transaction.as_mut(),
                &SecretDigest::of(user_secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            transaction
                .authorize_identity_operation(&actor, IdentityOperation::ExternalProviderRead)
                .await?;
            transaction.external_provider_admin_rows().await
        }
        .await;
        transaction.rollback().await?;
        result
    }

    pub async fn update(
        &self,
        user_secret: &str,
        provider_key: ExternalProviderKey,
        update: UpdateExternalProvider,
        request_id: uuid::Uuid,
        budget: &SecurityBudget,
    ) -> Result<ExternalProviderAdminView, IdentityError> {
        validate_opaque(user_secret)?;
        update.validate(provider_key)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &UpdateProvider {
                service: self,
                user_secret: user_secret.to_owned(),
                provider_key,
                update,
                request_id,
            },
        )
        .await
    }

    pub async fn rotate_secret(
        &self,
        user_secret: &str,
        provider_key: ExternalProviderKey,
        rotate: RotateExternalProviderSecret,
        request_id: uuid::Uuid,
        budget: &SecurityBudget,
    ) -> Result<ExternalProviderAdminView, IdentityError> {
        validate_opaque(user_secret)?;
        rotate.validate()?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &RotateSecret {
                service: self,
                user_secret: user_secret.to_owned(),
                provider_key,
                rotate,
                request_id,
            },
        )
        .await
    }

    pub async fn stats(
        &self,
        user_secret: &str,
        window: StatsWindow,
        budget: &SecurityBudget,
    ) -> Result<ExternalProviderStats, IdentityError> {
        validate_opaque(user_secret)?;
        let now_millis = now(self.clock.as_ref())?;
        let (from, to) = window.range(now_millis);
        let mut transaction = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                transaction.as_mut(),
                &SecretDigest::of(user_secret),
                now_millis,
            )
            .await?;
            transaction
                .authorize_identity_operation(&actor, IdentityOperation::ExternalProviderRead)
                .await?;
            transaction.external_login_stats(from, to).await
        }
        .await;
        transaction.rollback().await?;
        Ok(ExternalProviderStats {
            from: from / 1000,
            to: to / 1000,
            providers: result?,
        })
    }

    fn supported(provider_key: ExternalProviderKey) -> bool {
        matches!(
            provider_key,
            ExternalProviderKey::GitHub | ExternalProviderKey::Qq | ExternalProviderKey::WechatQr
        )
    }

    fn encrypt_secret(
        &self,
        provider_key: ExternalProviderKey,
        secret: &str,
    ) -> Result<String, IdentityError> {
        self.cipher
            .as_deref()
            .ok_or(IdentityError::DependencyUnavailable)?
            .encrypt(provider_key.as_str(), secret)
    }

    fn secret_key_version(&self) -> u32 {
        self.cipher
            .as_deref()
            .map(SecretCipher::key_version)
            .unwrap_or(1)
    }
}

struct UpdateProvider<'a> {
    service: &'a ExternalProviderAdminService,
    user_secret: String,
    provider_key: ExternalProviderKey,
    update: UpdateExternalProvider,
    request_id: uuid::Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for UpdateProvider<'_> {
    type Output = ExternalProviderAdminView;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        if !ExternalProviderAdminService::supported(self.provider_key) {
            return Err(IdentityError::InvalidInput);
        }
        let at = now(self.service.clock.as_ref())?;
        let actor = authenticate_current_actor(
            transaction,
            &SecretDigest::of(&self.user_secret),
            at,
            &IdentityLockScope::new(Vec::new())?,
        )
        .await?;
        transaction
            .authorize_identity_operation(&actor, IdentityOperation::ExternalProviderConfigure)
            .await?;
        let rows = transaction.external_provider_admin_rows().await?;
        let existing = rows
            .iter()
            .find(|row| row.provider_key == self.provider_key.as_str())
            .ok_or(IdentityError::NotFound)?;
        let secret_ciphertext = match &self.update.secret {
            Some(secret) => Some(self.service.encrypt_secret(self.provider_key, secret)?),
            None => None,
        };
        if self.update.enabled && !existing.secret_configured && secret_ciphertext.is_none() {
            return Err(IdentityError::InvalidInput);
        }
        let view = transaction
            .update_external_provider(
                self.provider_key,
                self.update.expected_revision,
                &self.update,
                secret_ciphertext,
                actor.account().user_id,
            )
            .await?;
        append_audit(
            transaction,
            self.request_id,
            Some(actor.account().user_id),
            "external_provider.update",
            "external_provider",
            self.provider_key.as_str().to_owned(),
            AuditResult::Success,
            None,
            serde_json::json!({
                "provider": self.provider_key.as_str(),
                "enabled": view.enabled,
                "changed_fields": ["enabled", "client_id", "secret"],
                "revision": view.revision
            }),
        )
        .await?;
        Ok(view)
    }
}

struct RotateSecret<'a> {
    service: &'a ExternalProviderAdminService,
    user_secret: String,
    provider_key: ExternalProviderKey,
    rotate: RotateExternalProviderSecret,
    request_id: uuid::Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for RotateSecret<'_> {
    type Output = ExternalProviderAdminView;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        if !ExternalProviderAdminService::supported(self.provider_key) {
            return Err(IdentityError::InvalidInput);
        }
        let at = now(self.service.clock.as_ref())?;
        let actor = authenticate_current_actor(
            transaction,
            &SecretDigest::of(&self.user_secret),
            at,
            &IdentityLockScope::new(Vec::new())?,
        )
        .await?;
        transaction
            .authorize_identity_operation(&actor, IdentityOperation::ExternalProviderConfigure)
            .await?;
        let ciphertext = self
            .service
            .encrypt_secret(self.provider_key, &self.rotate.secret)?;
        let view = transaction
            .rotate_external_provider_secret(
                self.provider_key,
                self.rotate.expected_revision,
                ciphertext,
                self.service.secret_key_version(),
                actor.account().user_id,
            )
            .await?;
        append_audit(
            transaction,
            self.request_id,
            Some(actor.account().user_id),
            "external_provider.secret",
            "external_provider",
            self.provider_key.as_str().to_owned(),
            AuditResult::Success,
            None,
            serde_json::json!({
                "provider": self.provider_key.as_str(),
                "revision": view.revision
            }),
        )
        .await?;
        Ok(view)
    }
}

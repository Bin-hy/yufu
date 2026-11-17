//! Presentation changes retain the existing current-actor/transaction boundary.
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
        site_branding::*,
    },
    service::security::repository::now,
};
use auth::security::budget::{Clock, SecurityBudget};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub struct SiteSettingsService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}
impl SiteSettingsService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn public_branding(&self, budget: &SecurityBudget) -> Result<Value, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let settings = tx.site_settings(false).await?;
            Ok(public_branding(&settings))
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn preflight(
        &self,
        secret: &str,
        operation: IdentityOperation,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            authorize(tx.as_mut(), &actor, operation).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn admin_settings(
        &self,
        secret: &str,
        budget: &SecurityBudget,
    ) -> Result<SiteSettings, IdentityError> {
        super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            authorize(tx.as_mut(), &actor, IdentityOperation::SiteSettingsRead).await?;
            tx.site_settings(false).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
    pub async fn save(
        &self,
        evidence: &BrowserWriteEvidence,
        input: SaveSiteSettings,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<SiteSettings, IdentityError> {
        validate_settings(&input.document)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &SettingsUnit {
                service: self,
                evidence,
                input: &input,
                request_id,
                reset: false,
            },
        )
        .await
    }
    pub async fn reset(
        &self,
        evidence: &BrowserWriteEvidence,
        revision: u64,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<SiteSettings, IdentityError> {
        let input = SaveSiteSettings {
            expected_revision: revision,
            document: SiteSettingsDocument::defaults()?,
        };
        execute_identity(
            self.repository.as_ref(),
            budget,
            &SettingsUnit {
                service: self,
                evidence,
                input: &input,
                request_id,
                reset: true,
            },
        )
        .await
    }
    pub async fn upload(
        &self,
        evidence: &BrowserWriteEvidence,
        kind: SiteAssetKind,
        bytes: Vec<u8>,
        budget: &SecurityBudget,
    ) -> Result<SiteAssetMetadata, IdentityError> {
        let (metadata, bytes) = super::site_asset::validate_upload(bytes, kind).await?;
        // Upload I/O and decoding have separate bounds; recheck current actor in a fresh DB budget.
        let write_budget = budget.renewed()?;
        execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &UploadUnit {
                service: self,
                evidence,
                metadata,
                bytes,
            },
        )
        .await
    }
    pub async fn asset(
        &self,
        secret: Option<&str>,
        id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<StoredSiteAsset, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            if let Some(secret) = secret {
                super::sso::validate_opaque(secret)?;
                let actor = read_current_actor(
                    tx.as_mut(),
                    &SecretDigest::of(secret),
                    now(self.clock.as_ref())?,
                )
                .await?;
                authorize(tx.as_mut(), &actor, IdentityOperation::SiteAssetPreview).await?;
            }
            tx.site_asset_content(id, secret.is_none())
                .await?
                .ok_or(IdentityError::NotFound)
        }
        .await;
        finish_read(tx, result, budget).await
    }
}
async fn authorize(
    tx: &mut dyn IdentityTransaction,
    actor: &VerifiedActor,
    operation: IdentityOperation,
) -> Result<(), IdentityError> {
    if !actor.account().super_administrator {
        return Err(IdentityError::Forbidden);
    }
    tx.authorize_identity_operation(actor, operation).await
}
async fn write_actor(
    tx: &mut dyn IdentityTransaction,
    evidence: &BrowserWriteEvidence,
    clock: &dyn Clock,
    operation: IdentityOperation,
) -> Result<VerifiedActor, IdentityError> {
    let digest = evidence
        .current_center_digest
        .as_ref()
        .ok_or(IdentityError::Unauthenticated)?;
    let actor = authenticate_current_actor(
        tx,
        digest,
        now(clock)?,
        &IdentityLockScope::new(vec![IdentityLockKey::Policy])?,
    )
    .await?;
    authorize(tx, &actor, operation).await?;
    Ok(actor)
}
struct SettingsUnit<'a> {
    service: &'a SiteSettingsService,
    evidence: &'a BrowserWriteEvidence,
    input: &'a SaveSiteSettings,
    request_id: Uuid,
    reset: bool,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for SettingsUnit<'_> {
    type Output = SiteSettings;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        let op = if self.reset {
            IdentityOperation::SiteSettingsReset
        } else {
            IdentityOperation::SiteSettingsSave
        };
        let actor = write_actor(tx, self.evidence, self.service.clock.as_ref(), op).await?;
        let before = tx.site_settings(true).await?;
        let result = tx
            .replace_site_settings(self.input, actor.account().user_id)
            .await?;
        let id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {id,request_id:self.request_id,actor_id:Some(actor.account().user_id),action:if self.reset {"site_settings.reset"}else{"site_settings.saved"},object_kind:"site_settings",object_id:"1".into(),result:AuditResult::Success,reason:None,change_summary:json!({"revision":result.revision,"changed_fields":changed_fields(&before.document,&result.document)}),operation_id:None,commit_marker:Some(id)}).await?;
        Ok(result)
    }
}
struct UploadUnit<'a> {
    service: &'a SiteSettingsService,
    evidence: &'a BrowserWriteEvidence,
    metadata: SiteAssetMetadata,
    bytes: Vec<u8>,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for UploadUnit<'_> {
    type Output = SiteAssetMetadata;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        let actor = write_actor(
            tx,
            self.evidence,
            self.service.clock.as_ref(),
            IdentityOperation::SiteAssetUpload,
        )
        .await?;
        tx.insert_site_asset(&NewSiteAsset {
            metadata: self.metadata.clone(),
            bytes: self.bytes.clone(),
            uploaded_by: actor.account().user_id,
        })
        .await?;
        Ok(self.metadata.clone())
    }
}
pub fn admin_response(settings: SiteSettings) -> Value {
    json!({"settings":settings,"limits":{"logo_bytes":2097152,"favicon_bytes":2097152,"cover_bytes":10485760,"max_dimension":8192,"staged_hours":24,"site_name":80,"site_description":300,"welcome_title":120,"welcome_description":1000,"copyright_text":300,"filing_number":100,"links":10,"link_label":80,"link_url":2048}})
}
pub fn public_branding(settings: &SiteSettings) -> Value {
    let resolved = resolve_branding(&settings.document);
    let b = &resolved.branding;
    let asset = |id: Option<Uuid>| id.map(|id| format!("/api/v1/site/assets/{id}"));
    let auth = |value: &ResolvedAuthBranding| json!({"cover_url":asset(value.cover_asset_id),"welcome_title":value.welcome_title,"welcome_description":value.welcome_description});
    json!({"revision":settings.revision,"branding":{"site_name":b.site_name,"site_description":b.site_description,"logo_url":asset(b.logo_asset_id),"favicon_url":asset(b.favicon_asset_id),"footer":b.footer},"login":auth(&resolved.login),"oidc":auth(&resolved.oidc)})
}

//! Branding storage shares the authenticated identity transaction.
use super::repository::pg::{get, signed, unsigned};
use crate::{
    domain::identity::{error::IdentityError, site_branding::*},
    service::security::repository::{PgSecurityTransaction, now},
};
use uuid::Uuid;

fn metadata(row: &sea_orm::QueryResult) -> Result<SiteAssetMetadata, IdentityError> {
    let id: Uuid = get(row, "id")?;
    let kind = match get::<String>(row, "kind")?.as_str() {
        "logo" => SiteAssetKind::Logo,
        "favicon" => SiteAssetKind::Favicon,
        "cover" => SiteAssetKind::Cover,
        _ => return Err(IdentityError::DependencyUnavailable),
    };
    Ok(SiteAssetMetadata {
        id,
        kind,
        mime: get(row, "mime")?,
        width: u32::try_from(get::<i32>(row, "width")?)
            .map_err(|_| IdentityError::DependencyUnavailable)?,
        height: u32::try_from(get::<i32>(row, "height")?)
            .map_err(|_| IdentityError::DependencyUnavailable)?,
        byte_size: unsigned(get(row, "byte_size")?)?,
        sha256: get(row, "sha256")?,
        preview_url: format!("/api/v1/admin/site-assets/{id}"),
    })
}
impl PgSecurityTransaction {
    pub(crate) async fn branding_settings(
        &self,
        for_update: bool,
    ) -> Result<SiteSettings, IdentityError> {
        let sql = if for_update {
            "SELECT * FROM site_settings WHERE id=1 FOR UPDATE"
        } else {
            "SELECT * FROM site_settings WHERE id=1"
        };
        let row = self.identity_one(sql, vec![]).await?;
        if get::<i32>(&row, "schema_version")? != 1 {
            return Err(IdentityError::DependencyUnavailable);
        }
        let document: SiteSettingsDocument = serde_json::from_value(get(&row, "document")?)
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        validate_settings(&document).map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(SiteSettings {
            document,
            revision: unsigned(get(&row, "revision")?)?,
            updated_by: get(&row, "updated_by")?,
            updated_at: get::<Option<i64>>(&row, "updated_at")?
                .map(|at| unsigned(at).map(|v| v / 1000))
                .transpose()?,
        })
    }
    pub(crate) async fn branding_metadata(
        &self,
        id: Uuid,
    ) -> Result<Option<SiteAssetMetadata>, IdentityError> {
        self.identity_rows(
            "SELECT id,kind,mime,width,height,byte_size,sha256 FROM site_asset WHERE id=$1",
            vec![id.into()],
        )
        .await?
        .first()
        .map(metadata)
        .transpose()
    }
    pub(crate) async fn branding_content(
        &self,
        id: Uuid,
        published: bool,
    ) -> Result<Option<StoredSiteAsset>, IdentityError> {
        let rows=self.identity_rows("SELECT mime,sha256,bytes FROM site_asset a WHERE a.id=$1 AND (NOT $2 OR EXISTS(SELECT 1 FROM site_settings_asset_ref r WHERE r.asset_id=a.id))",vec![id.into(),published.into()]).await?;
        rows.first()
            .map(|r| {
                Ok(StoredSiteAsset {
                    mime: get(r, "mime")?,
                    sha256: get(r, "sha256")?,
                    bytes: get(r, "bytes")?,
                })
            })
            .transpose()
    }
    pub(crate) async fn branding_insert(&self, asset: &NewSiteAsset) -> Result<(), IdentityError> {
        let m = &asset.metadata;
        let at = signed(now(self.clock.as_ref())?)?;
        self.identity_execute("INSERT INTO site_asset(id,kind,mime,width,height,byte_size,sha256,bytes,uploaded_by,created_at,unreferenced_since) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)",vec![m.id.into(),m.kind.as_str().into(),m.mime.clone().into(),i64::from(m.width).into(),i64::from(m.height).into(),signed(m.byte_size)?.into(),m.sha256.clone().into(),asset.bytes.clone().into(),asset.uploaded_by.into(),at.into()]).await?;
        Ok(())
    }
    pub(crate) async fn branding_replace(
        &mut self,
        input: &SaveSiteSettings,
        actor_id: i32,
    ) -> Result<SiteSettings, IdentityError> {
        let current = self.branding_settings(true).await?;
        if input.expected_revision != current.revision || current.revision >= MAX_REVISION {
            return Err(IdentityError::Conflict);
        }
        validate_settings(&input.document)?;
        let refs = referenced_assets(&input.document);
        for r in &refs {
            let asset = self
                .branding_metadata(r.asset_id)
                .await?
                .ok_or(IdentityError::NotFound)?;
            if asset.kind != r.kind {
                return Err(IdentityError::InvalidInput);
            }
        }
        let at = signed(now(self.clock.as_ref())?)?;
        let document =
            serde_json::to_value(&input.document).map_err(|_| IdentityError::InvalidInput)?;
        let affected=self.identity_execute("UPDATE site_settings SET document=$1,revision=revision+1,updated_by=$2,updated_at=$3 WHERE id=1 AND revision=$4",vec![document.into(),actor_id.into(),at.into(),signed(input.expected_revision)?.into()]).await?;
        if affected != 1 {
            return Err(IdentityError::Conflict);
        }
        self.identity_execute(
            "DELETE FROM site_settings_asset_ref WHERE settings_id=1",
            vec![],
        )
        .await?;
        for r in refs {
            self.identity_execute(
                "INSERT INTO site_settings_asset_ref(slot,settings_id,asset_id) VALUES($1,1,$2)",
                vec![r.slot.into(), r.asset_id.into()],
            )
            .await?;
        }
        self.identity_execute("UPDATE site_asset a SET unreferenced_since=CASE WHEN EXISTS(SELECT 1 FROM site_settings_asset_ref r WHERE r.asset_id=a.id) THEN NULL ELSE COALESCE(a.unreferenced_since,$1) END WHERE a.unreferenced_since IS NULL OR EXISTS(SELECT 1 FROM site_settings_asset_ref r WHERE r.asset_id=a.id)",vec![at.into()]).await?;
        self.branding_settings(false).await
    }
    pub(crate) async fn branding_cleanup(
        &self,
        before: u64,
        limit: u32,
    ) -> Result<u64, IdentityError> {
        if limit == 0 || limit > 100 {
            return Err(IdentityError::InvalidInput);
        }
        self.branding_settings(true).await?;
        self.identity_execute("WITH due AS (SELECT a.id FROM site_asset a WHERE a.unreferenced_since<$1 AND NOT EXISTS(SELECT 1 FROM site_settings_asset_ref r WHERE r.asset_id=a.id) ORDER BY a.unreferenced_since,a.id LIMIT $2 FOR UPDATE) DELETE FROM site_asset a USING due WHERE a.id=due.id",vec![signed(before)?.into(),i64::from(limit).into()]).await
    }
}

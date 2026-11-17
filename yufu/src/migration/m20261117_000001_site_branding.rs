use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
CREATE TABLE site_settings (
 id SMALLINT PRIMARY KEY CHECK(id=1), schema_version INTEGER NOT NULL DEFAULT 1 CHECK(schema_version=1),
 document JSONB NOT NULL, revision BIGINT NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 updated_by INTEGER REFERENCES "user"(id) ON DELETE SET NULL, updated_at BIGINT
);
CREATE TABLE site_asset (
 id UUID PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('logo','favicon','cover')),
 mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/webp','image/x-icon')),
 width INTEGER NOT NULL CHECK(width BETWEEN 1 AND 8192), height INTEGER NOT NULL CHECK(height BETWEEN 1 AND 8192),
 byte_size BIGINT NOT NULL CHECK(byte_size>0), sha256 TEXT NOT NULL CHECK(length(sha256)=64),
 bytes BYTEA NOT NULL, uploaded_by INTEGER REFERENCES "user"(id) ON DELETE SET NULL,
 created_at BIGINT NOT NULL, unreferenced_since BIGINT,
 CHECK(byte_size=octet_length(bytes)), CHECK(byte_size<=CASE WHEN kind='cover' THEN 10485760 ELSE 2097152 END),
 CHECK(mime<>'image/x-icon' OR kind='favicon')
);
CREATE INDEX site_asset_cleanup ON site_asset(unreferenced_since,id) WHERE unreferenced_since IS NOT NULL;
CREATE TABLE site_settings_asset_ref (
 slot TEXT PRIMARY KEY CHECK(slot IN ('logo','favicon','cover','oidc_cover')),
 settings_id SMALLINT NOT NULL REFERENCES site_settings(id) CHECK(settings_id=1),
 asset_id UUID NOT NULL REFERENCES site_asset(id) ON DELETE RESTRICT
);
CREATE INDEX site_asset_reference ON site_settings_asset_ref(asset_id);
INSERT INTO site_settings(id,document,revision) VALUES(1,'{
  "branding": {
    "site_name": "yufu",
    "site_description": "企业统一身份认证",
    "logo_asset_id": null,
    "favicon_asset_id": null,
    "cover_asset_id": null,
    "welcome_title": "一个账户，进入每一个工作场景。",
    "welcome_description": "统一管理企业账户、应用接入与授权边界。清晰的身份，是每一次安全协作的起点。",
    "footer": { "copyright_text": "yufu · 身份与访问管理 · 受企业安全策略保护", "filing_number": "", "filing_url": null, "links": [] }
  },
  "oidc": { "cover_asset_id": null, "welcome_title": null, "welcome_description": null }
}'::jsonb,1);
"#).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DROP TABLE site_settings_asset_ref; DROP TABLE site_asset; DROP TABLE site_settings;").await?;
        Ok(())
    }
}

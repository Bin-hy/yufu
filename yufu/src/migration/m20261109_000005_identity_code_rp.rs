//! A managed code identifies exactly the RP session created by its authorization.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
            ALTER TABLE oauth_authorization_code ADD COLUMN oidc_rp_session_id UUID;
            -- Old managed codes cannot be associated by guessing from nonce or root.
            UPDATE oauth_authorization_code SET used=TRUE WHERE code_digest IS NOT NULL;
            ALTER TABLE oauth_authorization_code
                ADD CONSTRAINT identity_code_rp FOREIGN KEY
                    (oidc_rp_session_id,sso_session_id,environment_id,config_version_id)
                    REFERENCES identity_oidc_rp_session(id,sso_session_id,environment_id,config_version_id)
                    ON DELETE RESTRICT,
                ADD CONSTRAINT identity_code_rp_required CHECK (
                    (code_digest IS NULL AND oidc_rp_session_id IS NULL) OR
                    (code_digest IS NOT NULL AND (used OR oidc_rp_session_id IS NOT NULL)));
            CREATE INDEX identity_code_rp ON oauth_authorization_code(oidc_rp_session_id);
        "#).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
            ALTER TABLE oauth_authorization_code DROP CONSTRAINT identity_code_rp_required,
                DROP CONSTRAINT identity_code_rp, DROP COLUMN oidc_rp_session_id;
        "#,
            )
            .await?;
        Ok(())
    }
}

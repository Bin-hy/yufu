//! Client secret versions are independent of configuration and lifecycle versions.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE oauth_client ADD COLUMN secret_revision BIGINT NOT NULL DEFAULT 0 CHECK (secret_revision >= 0);"
        ).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE oauth_client DROP COLUMN secret_revision;")
            .await?;
        Ok(())
    }
}

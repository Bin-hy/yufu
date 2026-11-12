//! Broaden the registered audience catalog without deleting historical business data.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE security_session_client DROP CONSTRAINT IF EXISTS security_session_client_audience_check;
             ALTER TABLE security_session_client ADD CONSTRAINT security_session_client_audience_check CHECK (audience <> '');"
        ).await?;
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // A narrowed audience list would invalidate administrator-registered applications.
        Ok(())
    }
}

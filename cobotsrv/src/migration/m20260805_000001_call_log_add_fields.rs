use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DO $$ BEGIN
                ALTER TABLE mcp_call_log ADD COLUMN IF NOT EXISTS input_params JSONB;
                ALTER TABLE mcp_call_log ADD COLUMN IF NOT EXISTS status_code INTEGER;
                ALTER TABLE mcp_call_log ADD COLUMN IF NOT EXISTS device_id VARCHAR;
                ALTER TABLE mcp_call_log ADD COLUMN IF NOT EXISTS cobotlet_alias VARCHAR;
                ALTER TABLE mcp_call_log ADD COLUMN IF NOT EXISTS source VARCHAR;
            END $$",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE mcp_call_log
                DROP COLUMN IF EXISTS source,
                DROP COLUMN IF EXISTS cobotlet_alias,
                DROP COLUMN IF EXISTS device_id,
                DROP COLUMN IF EXISTS status_code,
                DROP COLUMN IF EXISTS input_params",
        )
        .await?;
        Ok(())
    }
}

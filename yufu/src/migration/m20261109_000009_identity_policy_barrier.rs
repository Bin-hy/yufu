use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("CREATE TABLE identity_policy_barrier (id INTEGER PRIMARY KEY CHECK (id=1), revision BIGINT NOT NULL DEFAULT 0); INSERT INTO identity_policy_barrier (id) VALUES (1)").await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE identity_policy_barrier")
            .await?;
        Ok(())
    }
}

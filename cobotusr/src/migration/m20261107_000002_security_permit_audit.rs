use sea_orm_migration::{prelude::*, sea_orm::DbBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DbBackend::Postgres {
            return Err(DbErr::Migration(
                "security audit requires PostgreSQL".into(),
            ));
        }
        manager.get_connection().execute_unprepared(SCHEMA).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS security_permit; DROP TABLE security_audit; ALTER TABLE security_credential DROP CONSTRAINT IF EXISTS security_credential_permit_parent;")
            .await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE security_audit (
    id UUID PRIMARY KEY,
    event_type VARCHAR(128) NOT NULL CHECK (event_type <> ''),
    actor JSONB CHECK (jsonb_typeof(actor) = 'object'),
    operation VARCHAR(128),
    resource JSONB CHECK (jsonb_typeof(resource) = 'object'),
    credential_issuer TEXT,
    credential_jti VARCHAR(512),
    request_id UUID,
    task_id UUID,
    outcome TEXT NOT NULL CHECK (outcome IN ('success','denied','error')),
    reason_code VARCHAR(128) NOT NULL CHECK (reason_code <> ''),
    occurred_at BIGINT NOT NULL CHECK (occurred_at >= 0),
    CHECK ((credential_issuer IS NULL) = (credential_jti IS NULL))
);
CREATE INDEX security_audit_time ON security_audit(occurred_at);
CREATE INDEX security_audit_task ON security_audit(task_id);
"#;

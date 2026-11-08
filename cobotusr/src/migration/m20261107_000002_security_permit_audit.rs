use sea_orm_migration::{prelude::*, sea_orm::DbBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DbBackend::Postgres {
            return Err(DbErr::Migration(
                "security permit requires PostgreSQL".into(),
            ));
        }
        manager.get_connection().execute_unprepared(SCHEMA).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE security_audit; DROP TABLE security_permit;
             ALTER TABLE security_credential DROP CONSTRAINT security_credential_permit_parent;",
            )
            .await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
ALTER TABLE security_credential ADD CONSTRAINT security_credential_permit_parent
    UNIQUE (issuer,jti,grant_id,application);
CREATE TABLE security_permit (
    issuer TEXT NOT NULL,
    jti VARCHAR(512) NOT NULL CHECK (jti <> ''),
    parent_jti VARCHAR(512) NOT NULL,
    grant_id UUID NOT NULL,
    application VARCHAR(512) NOT NULL,
    request_id UUID NOT NULL CHECK (request_id <> '00000000-0000-0000-0000-000000000000'),
    task_id UUID CHECK (task_id <> '00000000-0000-0000-0000-000000000000'),
    actor JSONB NOT NULL CHECK (jsonb_typeof(actor) = 'object'),
    presenter JSONB NOT NULL CHECK (jsonb_typeof(presenter) = 'object'),
    operation VARCHAR(128) NOT NULL CHECK (operation <> ''),
    resource JSONB NOT NULL CHECK (jsonb_typeof(resource) = 'object'),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    capability_revision VARCHAR(512),
    target_binding_revision VARCHAR(512),
    signing_key_id VARCHAR(128) NOT NULL CHECK (signing_key_id <> ''),
    signed_permit TEXT NOT NULL CHECK (signed_permit <> ''),
    checked_at BIGINT NOT NULL CHECK (checked_at >= 0),
    valid_until BIGINT NOT NULL CHECK (valid_until > checked_at AND valid_until - checked_at <= 5000),
    issued_at BIGINT NOT NULL CHECK (issued_at >= 0 AND issued_at + 999 >= checked_at),
    expires_at BIGINT NOT NULL CHECK (expires_at > issued_at AND expires_at <= valid_until),
    PRIMARY KEY (issuer,jti),
    FOREIGN KEY (issuer,parent_jti,grant_id,application)
        REFERENCES security_credential(issuer,jti,grant_id,application) ON DELETE RESTRICT,
    UNIQUE (application,request_id),
    CHECK (operation <> 'task:execute' OR task_id IS NOT NULL)
);
-- NULL tasks are request-bound operations. Do not use a process lock to enforce
-- one authorization per task across instances or after a restart.
CREATE UNIQUE INDEX security_permit_one_task ON security_permit(application,task_id) WHERE task_id IS NOT NULL;
CREATE INDEX security_permit_parent ON security_permit(issuer,parent_jti);
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

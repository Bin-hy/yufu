use sea_orm_migration::{prelude::*, sea_orm::DbBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DbBackend::Postgres {
            return Err(DbErr::Migration(
                "security identity requires PostgreSQL".into(),
            ));
        }
        // Composite foreign keys pin every root to the exact issuer, principal
        // and application. No cascading deletion of security/revocation facts.
        manager
            .get_connection()
            .execute_unprepared(IDENTITY_SCHEMA)
            .await?;
        manager
            .get_connection()
            .execute_unprepared(BINDING_SCHEMA)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
            "DROP TABLE security_gateway_signing_key; DROP TABLE security_device_binding;
             DROP TABLE security_binding; DROP TABLE security_credential; DROP TABLE security_refresh_family;
             DROP TABLE security_session; DROP TABLE security_grant;",
            )
            .await?;
        Ok(())
    }
}

const IDENTITY_SCHEMA: &str = r#"
CREATE TABLE security_grant (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL CHECK (issuer <> ''),
    principal_kind TEXT NOT NULL CHECK (principal_kind IN ('user','machine','device')),
    subject VARCHAR(512) NOT NULL CHECK (subject <> ''),
    application VARCHAR(512) NOT NULL CHECK (application <> ''),
    grant_kind TEXT NOT NULL CHECK (grant_kind IN ('session','mcp','service','device')),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    revoked_at BIGINT CHECK (revoked_at >= created_at),
    UNIQUE (id,issuer,principal_kind,subject,application)
);
CREATE TABLE security_session (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL CHECK (issuer <> ''),
    principal_kind TEXT NOT NULL CHECK (principal_kind = 'user'),
    subject VARCHAR(512) NOT NULL CHECK (subject <> ''),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    revoked_at BIGINT CHECK (revoked_at >= created_at),
    UNIQUE (id,issuer,principal_kind,subject)
);
CREATE TABLE security_refresh_family (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL,
    principal_kind TEXT NOT NULL CHECK (principal_kind = 'user'),
    subject VARCHAR(512) NOT NULL,
    application VARCHAR(512) NOT NULL,
    session_id UUID NOT NULL,
    grant_id UUID NOT NULL,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    revoked_at BIGINT CHECK (revoked_at >= created_at),
    FOREIGN KEY (session_id,issuer,principal_kind,subject)
        REFERENCES security_session(id,issuer,principal_kind,subject) ON DELETE RESTRICT,
    FOREIGN KEY (grant_id,issuer,principal_kind,subject,application)
        REFERENCES security_grant(id,issuer,principal_kind,subject,application) ON DELETE RESTRICT,
    UNIQUE (id,session_id,grant_id)
);
CREATE TABLE security_credential (
    issuer TEXT NOT NULL,
    jti VARCHAR(512) NOT NULL CHECK (jti <> ''),
    principal_kind TEXT NOT NULL CHECK (principal_kind IN ('user','machine','device')),
    subject VARCHAR(512) NOT NULL CHECK (subject <> ''),
    credential_kind TEXT NOT NULL CHECK (credential_kind IN ('access','refresh')),
    token_use TEXT NOT NULL CHECK (token_use IN ('user_session','mcp_access','security_service',
        'permission_sync','device_access','gateway_operation','stream_connect')),
    audience VARCHAR(512) NOT NULL CHECK (audience <> ''),
    application VARCHAR(512) NOT NULL CHECK (application <> ''),
    bounds JSONB NOT NULL CHECK (jsonb_typeof(bounds) = 'object'),
    binding JSONB NOT NULL CHECK (jsonb_typeof(binding) = 'object'),
    grant_id UUID NOT NULL,
    session_id UUID,
    family_id UUID,
    revocation_parent_jti VARCHAR(512),
    minted_by JSONB CHECK (jsonb_typeof(minted_by) = 'object'),
    signing_key_id VARCHAR(128),
    secret_digest BYTEA UNIQUE CHECK (octet_length(secret_digest) = 32),
    issued_at BIGINT NOT NULL CHECK (issued_at >= 0),
    not_before BIGINT,
    expires_at BIGINT NOT NULL CHECK (expires_at > issued_at),
    revoked_at BIGINT CHECK (revoked_at >= issued_at),
    consumed_at BIGINT CHECK (consumed_at >= issued_at),
    replacement_jti VARCHAR(512),
    PRIMARY KEY (issuer,jti),
    FOREIGN KEY (grant_id,issuer,principal_kind,subject,application)
        REFERENCES security_grant(id,issuer,principal_kind,subject,application) ON DELETE RESTRICT,
    FOREIGN KEY (session_id,issuer,principal_kind,subject)
        REFERENCES security_session(id,issuer,principal_kind,subject) ON DELETE RESTRICT,
    FOREIGN KEY (family_id,session_id,grant_id)
        REFERENCES security_refresh_family(id,session_id,grant_id) ON DELETE RESTRICT,
    FOREIGN KEY (issuer,revocation_parent_jti)
        REFERENCES security_credential(issuer,jti) ON DELETE RESTRICT,
    FOREIGN KEY (issuer,replacement_jti)
        REFERENCES security_credential(issuer,jti) ON DELETE RESTRICT,
    CHECK (not_before IS NULL OR (not_before >= issued_at AND not_before < expires_at)),
    CHECK (token_use <> 'user_session' OR (session_id IS NOT NULL AND principal_kind = 'user')),
    CHECK (family_id IS NULL OR session_id IS NOT NULL),
    CHECK (credential_kind <> 'refresh' OR (family_id IS NOT NULL AND token_use = 'user_session')),
    CHECK ((credential_kind = 'refresh') = (secret_digest IS NOT NULL)),
    CHECK (credential_kind <> 'access' OR (signing_key_id IS NOT NULL AND signing_key_id <> '')),
    CHECK ((token_use = 'gateway_operation') = (revocation_parent_jti IS NOT NULL)),
    CHECK (revocation_parent_jti IS NULL OR revocation_parent_jti <> jti),
    CHECK (consumed_at IS NULL OR credential_kind = 'refresh'),
    CHECK (replacement_jti IS NULL OR (replacement_jti <> jti AND consumed_at IS NOT NULL))
);
CREATE INDEX security_credential_grant ON security_credential(grant_id);
CREATE INDEX security_credential_session ON security_credential(session_id);
CREATE INDEX security_credential_family ON security_credential(family_id);
CREATE INDEX security_credential_parent ON security_credential(issuer,revocation_parent_jti);
CREATE INDEX security_grant_principal_application ON security_grant(issuer,principal_kind,subject,application);
CREATE INDEX security_session_principal ON security_session(issuer,principal_kind,subject);
"#;

const BINDING_SCHEMA: &str = r#"
CREATE TABLE security_binding (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL CHECK (issuer <> ''),
    client_id VARCHAR(512) NOT NULL CHECK (client_id <> ''),
    token_use TEXT NOT NULL CHECK (token_use IN ('security_service','permission_sync')),
    application VARCHAR(512) NOT NULL CHECK (application <> ''),
    audience VARCHAR(512) NOT NULL CHECK (audience <> ''),
    allowed_operations JSONB NOT NULL CHECK (jsonb_typeof(allowed_operations) = 'array'),
    identity_binding JSONB NOT NULL CHECK (jsonb_typeof(identity_binding) = 'object'),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE (issuer,client_id,token_use,application,audience)
);
CREATE TABLE security_device_binding (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL CHECK (issuer <> ''),
    subject VARCHAR(512) NOT NULL CHECK (subject <> ''),
    application VARCHAR(512) NOT NULL CHECK (application <> ''),
    gateway_id VARCHAR(512) NOT NULL CHECK (gateway_id <> ''),
    device_id VARCHAR(512) NOT NULL CHECK (device_id <> ''),
    binding_revision VARCHAR(512) NOT NULL CHECK (binding_revision <> ''),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE (issuer,subject),
    UNIQUE (application,gateway_id,device_id)
);
CREATE TABLE security_gateway_signing_key (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL CHECK (issuer <> ''),
    application VARCHAR(512) NOT NULL CHECK (application <> ''),
    gateway_id VARCHAR(512) NOT NULL CHECK (gateway_id <> ''),
    kid VARCHAR(128) NOT NULL CHECK (kid <> ''),
    public_key TEXT NOT NULL CHECK (public_key <> ''),
    public_key_fingerprint BYTEA NOT NULL UNIQUE CHECK (octet_length(public_key_fingerprint) = 32),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    valid_from BIGINT NOT NULL CHECK (valid_from >= 0),
    valid_until BIGINT NOT NULL CHECK (valid_until > valid_from),
    UNIQUE (issuer,kid)
);
CREATE INDEX security_gateway_signing_key_gateway ON security_gateway_signing_key(application,gateway_id);
"#;

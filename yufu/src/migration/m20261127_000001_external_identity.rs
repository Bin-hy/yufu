//! External identity federation is additive: existing OIDC subjects and sessions are untouched.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(SCHEMA).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TABLE identity_handoff_ticket, identity_session_client,
    identity_external_flow, identity_external_identity, identity_external_provider;
"#,
            )
            .await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE identity_external_provider (
    provider_key TEXT PRIMARY KEY
        CHECK (provider_key IN ('github','qq','wechat_mini','wechat_web','wechat_qr')),
    family TEXT NOT NULL CHECK (family IN ('github','qq','wechat')),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    client_id TEXT NOT NULL DEFAULT '' CHECK (length(client_id) <= 256),
    secret_ciphertext TEXT,
    secret_key_version INTEGER NOT NULL DEFAULT 1 CHECK (secret_key_version > 0),
    config JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_by INTEGER REFERENCES "user"(id) ON DELETE SET NULL,
    updated_at BIGINT NOT NULL CHECK (updated_at >= 0),
    CHECK (NOT enabled OR (client_id <> '' AND secret_ciphertext IS NOT NULL))
);
INSERT INTO identity_external_provider
    (provider_key, family, enabled, client_id, config, revision, updated_at)
VALUES
    ('github', 'github', FALSE, '', '{}'::jsonb, 0, 0),
    ('qq', 'qq', FALSE, '', '{}'::jsonb, 0, 0),
    ('wechat_mini', 'wechat', FALSE, '', '{}'::jsonb, 0, 0),
    ('wechat_web', 'wechat', FALSE, '', '{}'::jsonb, 0, 0),
    ('wechat_qr', 'wechat', FALSE, '', '{}'::jsonb, 0, 0);

CREATE TABLE identity_external_identity (
    id UUID PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    provider_key TEXT NOT NULL REFERENCES identity_external_provider(provider_key) ON DELETE RESTRICT,
    provider_subject TEXT NOT NULL CHECK (length(provider_subject) BETWEEN 1 AND 255),
    provider_union_id TEXT CHECK (provider_union_id IS NULL OR length(provider_union_id) <= 255),
    display_name TEXT CHECK (display_name IS NULL OR length(display_name) <= 200),
    avatar_url TEXT CHECK (
        avatar_url IS NULL OR
        (length(avatar_url) <= 2048 AND avatar_url LIKE 'https://%')
    ),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    updated_at BIGINT NOT NULL CHECK (updated_at >= created_at),
    last_login_at BIGINT CHECK (last_login_at IS NULL OR last_login_at >= created_at),
    UNIQUE (provider_key, provider_subject),
    UNIQUE (user_id, provider_key)
);
CREATE INDEX identity_external_user ON identity_external_identity(user_id);
CREATE INDEX identity_external_union ON identity_external_identity(provider_key, provider_union_id)
    WHERE provider_union_id IS NOT NULL;

CREATE TABLE identity_external_flow (
    id UUID PRIMARY KEY,
    provider_key TEXT NOT NULL REFERENCES identity_external_provider(provider_key) ON DELETE RESTRICT,
    purpose TEXT NOT NULL CHECK (purpose IN ('login','bind')),
    state_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(state_digest) = 32),
    browser_binding_digest BYTEA NOT NULL CHECK (octet_length(browser_binding_digest) = 32),
    user_id INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    oidc_flow_id UUID REFERENCES identity_authorization_flow(id) ON DELETE SET NULL,
    nonce TEXT CHECK (nonce IS NULL OR length(nonce) <= 256),
    secret_ciphertext TEXT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > created_at),
    consumed_at BIGINT CHECK (
        consumed_at IS NULL OR (consumed_at >= created_at AND consumed_at < expires_at)
    ),
    CHECK (purpose <> 'bind' OR user_id IS NOT NULL)
);
CREATE INDEX identity_external_flow_pending
    ON identity_external_flow(browser_binding_digest, expires_at)
    WHERE consumed_at IS NULL;
CREATE INDEX identity_external_flow_oidc ON identity_external_flow(oidc_flow_id);

CREATE TABLE identity_session_client (
    sso_session_id UUID PRIMARY KEY REFERENCES identity_sso_session(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('browser','wechat_mini')),
    device_id TEXT CHECK (device_id IS NULL OR length(device_id) <= 128),
    device_label TEXT CHECK (device_label IS NULL OR length(device_label) <= 200),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    last_seen_at BIGINT CHECK (last_seen_at IS NULL OR last_seen_at >= created_at)
);

CREATE TABLE identity_handoff_ticket (
    id UUID PRIMARY KEY,
    secret_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(secret_digest) = 32),
    purpose TEXT NOT NULL CHECK (purpose IN ('session','flow_resume')),
    sso_session_id UUID NOT NULL REFERENCES identity_sso_session(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    auth_epoch BIGINT NOT NULL CHECK (auth_epoch >= 0),
    oidc_flow_id UUID REFERENCES identity_authorization_flow(id) ON DELETE CASCADE,
    application_id INTEGER REFERENCES application(id) ON DELETE RESTRICT,
    environment_id UUID REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    next_path TEXT CHECK (
        next_path IS NULL OR (length(next_path) <= 2048 AND next_path LIKE '/%')
    ),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > created_at),
    consumed_at BIGINT CHECK (
        consumed_at IS NULL OR (consumed_at >= created_at AND consumed_at < expires_at)
    ),
    CHECK (
        (purpose = 'session' AND oidc_flow_id IS NULL AND application_id IS NOT NULL AND environment_id IS NOT NULL) OR
        (purpose = 'flow_resume' AND oidc_flow_id IS NOT NULL AND application_id IS NULL AND environment_id IS NULL)
    )
);
CREATE INDEX identity_handoff_pending
    ON identity_handoff_ticket(expires_at)
    WHERE consumed_at IS NULL;
"#;

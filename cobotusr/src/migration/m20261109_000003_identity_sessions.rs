//! Center/RP links augment the existing credential graph, rather than replacing it.
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
        manager.get_connection().execute_unprepared(r#"
            DROP INDEX identity_consent_legacy_unique, identity_consent_version_unique;
            ALTER TABLE oauth_consent DROP CONSTRAINT identity_consent_version,
                DROP COLUMN environment_id, DROP COLUMN config_version_id;
            CREATE UNIQUE INDEX idx_oauth_consent_user_client ON oauth_consent(user_id,client_id);
            ALTER TABLE oauth_authorization_code DROP COLUMN code_digest, DROP COLUMN flow_id,
                DROP COLUMN sso_session_id, DROP COLUMN environment_id, DROP COLUMN config_version_id,
                DROP COLUMN auth_time, DROP COLUMN auth_epoch, DROP COLUMN lifecycle_epoch,
                DROP COLUMN access_generation;
            ALTER TABLE security_session DROP COLUMN sso_session_id, DROP COLUMN oidc_rp_session_id,
                DROP COLUMN environment_id, DROP COLUMN config_version_id, DROP COLUMN auth_epoch,
                DROP COLUMN lifecycle_epoch, DROP COLUMN access_generation, DROP COLUMN identity_scopes;
            DROP TABLE identity_logout_delivery, identity_audit_event, identity_oidc_rp_session,
                identity_action_flow, identity_authorization_flow, identity_browser_binding, identity_sso_session;
        "#).await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE identity_sso_session (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL REFERENCES identity_provider(issuer) ON DELETE RESTRICT,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    subject TEXT NOT NULL,
    secret_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(secret_digest) = 32),
    auth_epoch BIGINT NOT NULL CHECK (auth_epoch >= 0),
    authenticated_at BIGINT NOT NULL CHECK (authenticated_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > authenticated_at),
    revoked_at BIGINT CHECK (revoked_at >= authenticated_at),
    created_at BIGINT NOT NULL CHECK (created_at >= authenticated_at),
    last_seen_at BIGINT CHECK (last_seen_at >= created_at),
    CHECK (subject = user_id::text),
    FOREIGN KEY (issuer,subject) REFERENCES identity_subject(issuer,sub) ON DELETE RESTRICT,
    UNIQUE (id,issuer,subject),
    UNIQUE (id,issuer,subject,auth_epoch),
    UNIQUE (id,user_id,auth_epoch)
);
CREATE INDEX identity_sso_user ON identity_sso_session(user_id,expires_at);
CREATE TABLE identity_browser_binding (
    secret_digest BYTEA PRIMARY KEY CHECK (octet_length(secret_digest)=32),
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision>=0),
    touched_at BIGINT NOT NULL CHECK (touched_at>=0)
);
CREATE TABLE identity_authorization_flow (
    id UUID PRIMARY KEY,
    request_payload JSONB NOT NULL CHECK (jsonb_typeof(request_payload) = 'object'),
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    config_version_id UUID NOT NULL,
    browser_proof_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(browser_proof_digest) = 32),
    browser_binding_digest BYTEA NOT NULL CHECK (octet_length(browser_binding_digest) = 32),
    bound_sso_session_id UUID REFERENCES identity_sso_session(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > created_at),
    completed_at BIGINT CHECK (completed_at >= created_at AND completed_at < expires_at),
    reauth_completed_at BIGINT CHECK (reauth_completed_at >= created_at AND reauth_completed_at < expires_at),
    FOREIGN KEY (config_version_id,environment_id) REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT,
    CHECK (reauth_completed_at IS NULL OR bound_sso_session_id IS NOT NULL)
);
CREATE INDEX identity_flow_browser_pending ON identity_authorization_flow(browser_binding_digest,expires_at) WHERE completed_at IS NULL;
CREATE INDEX identity_flow_sso ON identity_authorization_flow(bound_sso_session_id);
CREATE TABLE identity_action_flow (
    id UUID PRIMARY KEY,
    action_kind TEXT NOT NULL CHECK (action_kind IN ('invitation','password_reset','logout_confirmation')),
    invitation_id UUID REFERENCES identity_invitation(id) ON DELETE RESTRICT,
    password_reset_id UUID REFERENCES identity_password_reset(id) ON DELETE RESTRICT,
    sso_session_id UUID REFERENCES identity_sso_session(id) ON DELETE RESTRICT,
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(request_payload) = 'object'),
    browser_proof_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(browser_proof_digest) = 32),
    browser_binding_digest BYTEA NOT NULL CHECK (octet_length(browser_binding_digest) = 32),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > created_at),
    consumed_at BIGINT CHECK (consumed_at >= created_at AND consumed_at < expires_at),
    CHECK ((action_kind = 'invitation' AND invitation_id IS NOT NULL AND password_reset_id IS NULL AND sso_session_id IS NULL) OR
        (action_kind = 'password_reset' AND password_reset_id IS NOT NULL AND invitation_id IS NULL AND sso_session_id IS NULL) OR
        (action_kind = 'logout_confirmation' AND sso_session_id IS NOT NULL AND invitation_id IS NULL AND password_reset_id IS NULL))
);
CREATE TABLE identity_oidc_rp_session (
    id UUID PRIMARY KEY,
    sso_session_id UUID NOT NULL,
    issuer TEXT NOT NULL,
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    config_version_id UUID NOT NULL,
    protocol_sid UUID NOT NULL UNIQUE,
    subject TEXT NOT NULL,
    auth_time BIGINT NOT NULL CHECK (auth_time >= 0),
    nonce TEXT,
    oauth_client_id INTEGER NOT NULL REFERENCES oauth_client(id) ON DELETE RESTRICT,
    identity_scopes TEXT NOT NULL CHECK (identity_scopes <> ''),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    revoked_at BIGINT CHECK (revoked_at >= created_at),
    FOREIGN KEY (sso_session_id,issuer,subject) REFERENCES identity_sso_session(id,issuer,subject) ON DELETE RESTRICT,
    FOREIGN KEY (config_version_id,environment_id) REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT,
    UNIQUE (id,sso_session_id,environment_id,config_version_id)
);
CREATE INDEX identity_rp_sso ON identity_oidc_rp_session(sso_session_id,environment_id);
ALTER TABLE security_session
    ADD COLUMN sso_session_id UUID,
    ADD COLUMN oidc_rp_session_id UUID,
    ADD COLUMN environment_id UUID REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    ADD COLUMN config_version_id UUID,
    ADD COLUMN auth_epoch BIGINT CHECK (auth_epoch >= 0),
    ADD COLUMN lifecycle_epoch BIGINT CHECK (lifecycle_epoch >= 0),
    ADD COLUMN access_generation BIGINT CHECK (access_generation >= 0),
    ADD COLUMN identity_scopes TEXT,
    ADD CONSTRAINT identity_security_root FOREIGN KEY (sso_session_id,issuer,subject,auth_epoch)
        REFERENCES identity_sso_session(id,issuer,subject,auth_epoch) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_security_rp FOREIGN KEY (oidc_rp_session_id,sso_session_id,environment_id,config_version_id)
        REFERENCES identity_oidc_rp_session(id,sso_session_id,environment_id,config_version_id) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_security_version FOREIGN KEY (config_version_id,environment_id)
        REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_security_complete_root CHECK (num_nonnulls(sso_session_id,auth_epoch) IN (0,2)),
    ADD CONSTRAINT identity_security_complete_managed CHECK (
        num_nonnulls(oidc_rp_session_id,environment_id,config_version_id,lifecycle_epoch,access_generation,identity_scopes) = 0 OR
        (num_nonnulls(oidc_rp_session_id,environment_id,config_version_id,lifecycle_epoch,access_generation,identity_scopes,sso_session_id,auth_epoch) = 8 AND identity_scopes <> ''));
CREATE INDEX identity_security_sso ON security_session(sso_session_id);
CREATE INDEX identity_security_environment ON security_session(environment_id,subject);
ALTER TABLE oauth_authorization_code
    ADD COLUMN code_digest BYTEA UNIQUE CHECK (octet_length(code_digest) = 32),
    ADD COLUMN flow_id UUID REFERENCES identity_authorization_flow(id) ON DELETE RESTRICT,
    ADD COLUMN sso_session_id UUID,
    ADD COLUMN environment_id UUID REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    ADD COLUMN config_version_id UUID,
    ADD COLUMN auth_time BIGINT CHECK (auth_time >= 0),
    ADD COLUMN auth_epoch BIGINT CHECK (auth_epoch >= 0),
    ADD COLUMN lifecycle_epoch BIGINT CHECK (lifecycle_epoch >= 0),
    ADD COLUMN access_generation BIGINT CHECK (access_generation >= 0),
    ADD CONSTRAINT identity_code_root FOREIGN KEY (sso_session_id,user_id,auth_epoch)
        REFERENCES identity_sso_session(id,user_id,auth_epoch) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_code_version FOREIGN KEY (config_version_id,environment_id)
        REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_code_complete CHECK (num_nonnulls(code_digest,flow_id,sso_session_id,environment_id,config_version_id,auth_time,auth_epoch,lifecycle_epoch,access_generation) IN (0,9)),
    ADD CONSTRAINT identity_code_no_plaintext CHECK (code_digest IS NULL OR code = 'sha256:' || encode(code_digest,'hex'));
CREATE INDEX identity_code_sso ON oauth_authorization_code(sso_session_id);
ALTER TABLE oauth_consent
    ADD COLUMN environment_id UUID REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    ADD COLUMN config_version_id UUID,
    ADD CONSTRAINT identity_consent_version FOREIGN KEY (config_version_id,environment_id)
        REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_consent_complete CHECK (num_nonnulls(environment_id,config_version_id) IN (0,2));
DROP INDEX IF EXISTS idx_oauth_consent_user_client;
CREATE UNIQUE INDEX identity_consent_legacy_unique ON oauth_consent(user_id,client_id) WHERE environment_id IS NULL;
CREATE UNIQUE INDEX identity_consent_version_unique ON oauth_consent(user_id,environment_id,config_version_id) WHERE environment_id IS NOT NULL;
CREATE TABLE identity_audit_event (
    id UUID PRIMARY KEY,
    request_id UUID NOT NULL,
    actor_id INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    action TEXT NOT NULL CHECK (action <> ''),
    object_kind TEXT NOT NULL CHECK (object_kind <> ''),
    object_id TEXT NOT NULL CHECK (object_id <> ''),
    happened_at BIGINT NOT NULL CHECK (happened_at >= 0),
    result TEXT NOT NULL CHECK (result IN ('success','denied','failure')),
    reason_code TEXT,
    change_summary JSONB NOT NULL CHECK (jsonb_typeof(change_summary) = 'object'),
    operation_id UUID,
    commit_marker UUID,
    CHECK (result <> 'success' OR commit_marker IS NOT NULL)
);
CREATE INDEX identity_audit_request ON identity_audit_event(request_id,happened_at);
CREATE INDEX identity_audit_object ON identity_audit_event(object_kind,object_id,happened_at);
CREATE TABLE identity_logout_delivery (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES identity_audit_event(id) ON DELETE RESTRICT,
    rp_session_id UUID NOT NULL REFERENCES identity_oidc_rp_session(id) ON DELETE RESTRICT,
    client_id TEXT NOT NULL CHECK (client_id <> ''),
    endpoint_snapshot TEXT NOT NULL CHECK (endpoint_snapshot <> ''),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','leased','delivered','permanent_failure','expired')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at BIGINT NOT NULL CHECK (next_attempt_at >= 0),
    lease_until BIGINT CHECK (lease_until >= 0),
    lease_owner UUID,
    token_jti UUID NOT NULL,
    last_error_code TEXT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    delivered_at BIGINT CHECK (delivered_at >= created_at),
    UNIQUE (event_id,rp_session_id),
    CHECK ((status = 'leased') = (lease_owner IS NOT NULL AND lease_until IS NOT NULL)),
    CHECK ((status = 'delivered') = (delivered_at IS NOT NULL))
);
CREATE INDEX identity_logout_due ON identity_logout_delivery(status,next_attempt_at,lease_until);
"#;

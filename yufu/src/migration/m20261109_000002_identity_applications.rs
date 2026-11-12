//! Governance is explicit, including for orphan historical OAuth clients.
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
            DROP TABLE identity_migration_conflict, identity_request_operation, identity_introspection_reader,
                identity_admin_activation, identity_app_approval, identity_access_generation,
                identity_app_test_user, identity_app_assignment, identity_app_member;
            ALTER TABLE identity_app_environment DROP CONSTRAINT identity_environment_active_version;
            ALTER TABLE oauth_client DROP CONSTRAINT identity_public_client_secret,
                DROP COLUMN management_state, DROP COLUMN environment_id;
            DROP TABLE identity_app_config_version, identity_app_environment;
            DROP FUNCTION identity_immutable_config();
            ALTER TABLE security_session_client ADD CONSTRAINT security_session_client_check CHECK (application=audience) NOT VALID;
            ALTER TABLE application DROP COLUMN management_state, DROP COLUMN application_kind,
                DROP COLUMN management_mode, DROP COLUMN lifecycle, DROP COLUMN identity_revision,
                DROP COLUMN owner_revision, DROP COLUMN access_revision, DROP COLUMN security_revision;
        "#).await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
ALTER TABLE security_session_client DROP CONSTRAINT IF EXISTS security_session_client_application_check;
ALTER TABLE security_session_client DROP CONSTRAINT IF EXISTS security_session_client_check;
ALTER TABLE application
    ADD COLUMN management_state TEXT NOT NULL DEFAULT 'legacy_unmanaged' CHECK (management_state IN ('legacy_unmanaged','managed')),
    ADD COLUMN application_kind TEXT NOT NULL DEFAULT 'internal' CHECK (application_kind IN ('internal','third_party')),
    ADD COLUMN management_mode TEXT NOT NULL DEFAULT 'developer_managed' CHECK (management_mode IN ('developer_managed','admin_configured')),
    ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active','suspended')),
    ADD COLUMN identity_revision BIGINT NOT NULL DEFAULT 0 CHECK (identity_revision >= 0),
    ADD COLUMN owner_revision BIGINT NOT NULL DEFAULT 0 CHECK (owner_revision >= 0),
    ADD COLUMN access_revision BIGINT NOT NULL DEFAULT 0 CHECK (access_revision >= 0),
    ADD COLUMN security_revision BIGINT NOT NULL DEFAULT 0 CHECK (security_revision >= 0);
ALTER TABLE oauth_client
    ADD COLUMN management_state TEXT NOT NULL DEFAULT 'legacy_unmanaged' CHECK (management_state IN ('legacy_unmanaged','managed')),
    ADD COLUMN environment_id UUID;
CREATE TABLE identity_app_environment (
    id UUID PRIMARY KEY,
    application_id INTEGER NOT NULL REFERENCES application(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('development','production')),
    oauth_client_id INTEGER NOT NULL UNIQUE REFERENCES oauth_client(id) ON DELETE RESTRICT,
    active_version_id UUID,
    lifecycle_epoch BIGINT NOT NULL DEFAULT 0 CHECK (lifecycle_epoch >= 0),
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    UNIQUE (application_id,kind),
    UNIQUE (id,application_id)
);
ALTER TABLE oauth_client
    ADD CONSTRAINT identity_client_environment FOREIGN KEY (environment_id) REFERENCES identity_app_environment(id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT identity_client_management CHECK ((management_state = 'managed') = (environment_id IS NOT NULL)),
    ADD CONSTRAINT identity_public_client_secret CHECK (NOT public_client OR client_secret_hash = '') NOT VALID;
CREATE INDEX identity_client_environment ON oauth_client(environment_id);
CREATE TABLE identity_app_config_version (
    id UUID PRIMARY KEY,
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    version BIGINT NOT NULL CHECK (version > 0),
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    payload_hash TEXT NOT NULL CHECK (length(payload_hash) = 64),
    candidate_oauth_client_id INTEGER REFERENCES oauth_client(id) ON DELETE RESTRICT,
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    UNIQUE (environment_id,version),
    UNIQUE (id,environment_id)
);
ALTER TABLE identity_app_environment ADD CONSTRAINT identity_environment_active_version
    FOREIGN KEY (active_version_id,id) REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED;
CREATE FUNCTION identity_immutable_config() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'identity configuration versions are immutable' USING ERRCODE = '23514'; END;
$$;
CREATE TRIGGER identity_config_immutable BEFORE UPDATE OR DELETE ON identity_app_config_version FOR EACH ROW EXECUTE FUNCTION identity_immutable_config();
CREATE TABLE identity_app_member (
    application_id INTEGER NOT NULL REFERENCES application(id) ON DELETE RESTRICT,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    member_role TEXT NOT NULL CHECK (member_role IN ('owner','maintainer')),
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    PRIMARY KEY (application_id,user_id)
);
CREATE UNIQUE INDEX identity_app_single_owner ON identity_app_member(application_id) WHERE member_role = 'owner';
CREATE INDEX identity_app_member_user ON identity_app_member(user_id,application_id);
CREATE TABLE identity_app_assignment (
    id UUID PRIMARY KEY,
    application_id INTEGER NOT NULL REFERENCES application(id) ON DELETE RESTRICT,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('user','group')),
    user_id INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    group_id UUID REFERENCES identity_user_group(id) ON DELETE RESTRICT,
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    CHECK ((target_kind = 'user' AND user_id IS NOT NULL AND group_id IS NULL) OR
        (target_kind = 'group' AND group_id IS NOT NULL AND user_id IS NULL))
);
CREATE UNIQUE INDEX identity_app_assignment_user ON identity_app_assignment(application_id,user_id) WHERE target_kind = 'user';
CREATE UNIQUE INDEX identity_app_assignment_group ON identity_app_assignment(application_id,group_id) WHERE target_kind = 'group';
CREATE INDEX identity_assignment_group ON identity_app_assignment(group_id,application_id) WHERE group_id IS NOT NULL;
CREATE TABLE identity_app_test_user (
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    PRIMARY KEY (environment_id,user_id)
);
CREATE TABLE identity_access_generation (
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    generation BIGINT NOT NULL DEFAULT 0 CHECK (generation >= 0),
    eligible BOOLEAN NOT NULL DEFAULT FALSE,
    changed_at BIGINT NOT NULL CHECK (changed_at >= 0),
    PRIMARY KEY (environment_id,user_id)
);
CREATE TABLE identity_app_approval (
    id UUID PRIMARY KEY,
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    version_id UUID NOT NULL,
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot) = 'object'),
    snapshot_hash TEXT NOT NULL CHECK (length(snapshot_hash) = 64),
    requested_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','obsolete')),
    reviewed_by INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    reason TEXT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    reviewed_at BIGINT CHECK (reviewed_at >= created_at),
    FOREIGN KEY (version_id,environment_id) REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT,
    CHECK (reviewed_by IS NULL OR reviewed_by <> requested_by),
    CHECK ((status IN ('approved','rejected')) = (reviewed_by IS NOT NULL)),
    CHECK ((reviewed_by IS NULL) = (reviewed_at IS NULL))
);
CREATE UNIQUE INDEX identity_environment_pending_approval ON identity_app_approval(environment_id) WHERE status = 'pending';
CREATE TABLE identity_admin_activation (
    id UUID PRIMARY KEY,
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    version_id UUID NOT NULL,
    actor_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot) = 'object'),
    logout_limitation_accepted BOOLEAN NOT NULL,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    FOREIGN KEY (version_id,environment_id) REFERENCES identity_app_config_version(id,environment_id) ON DELETE RESTRICT
);
CREATE TABLE identity_introspection_reader (
    id UUID PRIMARY KEY,
    reader_id TEXT NOT NULL UNIQUE CHECK (reader_id <> ''),
    secret_hash TEXT NOT NULL CHECK (secret_hash <> ''),
    environment_id UUID NOT NULL REFERENCES identity_app_environment(id) ON DELETE RESTRICT,
    approved_audience TEXT NOT NULL CHECK (approved_audience <> ''),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    secret_revision BIGINT NOT NULL DEFAULT 0 CHECK (secret_revision >= 0),
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    updated_at BIGINT NOT NULL CHECK (updated_at >= created_at)
);
CREATE TABLE identity_request_operation (
    id UUID PRIMARY KEY,
    actor_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    operation_kind TEXT NOT NULL CHECK (operation_kind <> ''),
    idempotency_key TEXT NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 128),
    request_hash TEXT NOT NULL CHECK (length(request_hash) = 64),
    resource_id TEXT NOT NULL CHECK (resource_id <> ''),
    result_summary JSONB NOT NULL CHECK (jsonb_typeof(result_summary) = 'object'),
    committed_at BIGINT NOT NULL CHECK (committed_at >= 0),
    UNIQUE (actor_id,operation_kind,idempotency_key)
);
CREATE TABLE identity_migration_conflict (
    id UUID PRIMARY KEY,
    oauth_client_id INTEGER REFERENCES oauth_client(id) ON DELETE RESTRICT,
    candidate_application_id INTEGER REFERENCES application(id) ON DELETE RESTRICT,
    candidate_owner_id INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    conflict_kind TEXT NOT NULL CHECK (conflict_kind <> ''),
    evidence_summary JSONB NOT NULL CHECK (jsonb_typeof(evidence_summary) = 'object'),
    resolved_by INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    resolution TEXT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    resolved_at BIGINT CHECK (resolved_at >= created_at),
    CHECK ((resolved_by IS NULL) = (resolved_at IS NULL))
);
"#;

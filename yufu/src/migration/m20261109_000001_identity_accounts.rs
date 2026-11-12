//! Additive identity onboarding. The issuer is initialized explicitly by the host.
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
            DROP TABLE identity_group_member, identity_user_group, identity_collaboration_candidate,
                identity_password_reset, identity_invitation, identity_subject, identity_provider;
            ALTER TABLE "user" DROP COLUMN enabled, DROP COLUMN person_kind,
                DROP COLUMN developer_enabled, DROP COLUMN auth_epoch, DROP COLUMN email,
                DROP COLUMN email_verified_at, DROP COLUMN identity_revision;
        "#,
            )
            .await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
ALTER TABLE "user"
    ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN person_kind TEXT NOT NULL DEFAULT 'employee' CHECK (person_kind IN ('employee','collaborator')),
    ADD COLUMN developer_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN auth_epoch BIGINT NOT NULL DEFAULT 0 CHECK (auth_epoch >= 0),
    ADD COLUMN identity_revision BIGINT NOT NULL DEFAULT 0 CHECK (identity_revision >= 0),
    ADD COLUMN email TEXT CHECK (email IS NULL OR (length(email) BETWEEN 3 AND 254)),
    ADD COLUMN email_verified_at BIGINT CHECK (email_verified_at >= 0),
    ADD CONSTRAINT identity_user_email_verification CHECK (email_verified_at IS NULL OR email IS NOT NULL);
CREATE TABLE identity_provider (
    id SMALLINT PRIMARY KEY CHECK (id = 1),
    issuer TEXT NOT NULL UNIQUE CHECK (issuer <> ''),
    initialized_at BIGINT NOT NULL CHECK (initialized_at >= 0),
    profile_version INTEGER NOT NULL DEFAULT 1 CHECK (profile_version > 0)
);
CREATE TABLE identity_subject (
    issuer TEXT NOT NULL REFERENCES identity_provider(issuer) ON DELETE RESTRICT,
    sub TEXT NOT NULL CHECK (sub <> ''),
    user_id INTEGER UNIQUE REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    tombstoned_at BIGINT CHECK (tombstoned_at >= created_at),
    PRIMARY KEY (issuer,sub),
    CHECK (user_id IS NULL OR sub = user_id::text),
    CHECK (user_id IS NOT NULL OR tombstoned_at IS NOT NULL)
);
CREATE TABLE identity_invitation (
    id UUID PRIMARY KEY,
    secret_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(secret_digest) = 32),
    purpose TEXT NOT NULL CHECK (purpose IN ('onboarding','qualification')),
    target_user_id INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    person_kind TEXT NOT NULL CHECK (person_kind IN ('employee','collaborator')),
    developer_grant BOOLEAN NOT NULL DEFAULT FALSE,
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > created_at),
    consumed_at BIGINT CHECK (consumed_at >= created_at),
    accepted_user_id INTEGER REFERENCES "user"(id) ON DELETE RESTRICT,
    revoked_at BIGINT CHECK (revoked_at >= created_at),
    CHECK (purpose <> 'qualification' OR target_user_id IS NOT NULL),
    CHECK ((consumed_at IS NULL) = (accepted_user_id IS NULL)),
    CHECK (consumed_at IS NULL OR consumed_at < expires_at),
    CHECK (target_user_id IS NULL OR accepted_user_id IS NULL OR target_user_id = accepted_user_id)
);
CREATE INDEX identity_invitation_expiry ON identity_invitation(expires_at) WHERE consumed_at IS NULL AND revoked_at IS NULL;
CREATE TABLE identity_password_reset (
    id UUID PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    secret_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(secret_digest) = 32),
    auth_epoch_at_issue BIGINT NOT NULL CHECK (auth_epoch_at_issue >= 0),
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > created_at),
    consumed_at BIGINT CHECK (consumed_at >= created_at AND consumed_at < expires_at),
    revoked_at BIGINT CHECK (revoked_at >= created_at)
);
CREATE INDEX identity_password_reset_user ON identity_password_reset(user_id);
CREATE TABLE identity_collaboration_candidate (
    requester_user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    candidate_user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    PRIMARY KEY (requester_user_id,candidate_user_id),
    CHECK (requester_user_id <> candidate_user_id)
);
CREATE TABLE identity_user_group (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 128),
    description TEXT,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    updated_at BIGINT NOT NULL CHECK (updated_at >= created_at)
);
CREATE TABLE identity_group_member (
    group_id UUID NOT NULL REFERENCES identity_user_group(id) ON DELETE RESTRICT,
    user_id INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_by INTEGER NOT NULL REFERENCES "user"(id) ON DELETE RESTRICT,
    created_at BIGINT NOT NULL CHECK (created_at >= 0),
    PRIMARY KEY (group_id,user_id)
);
CREATE INDEX identity_group_member_user ON identity_group_member(user_id,group_id);
"#;

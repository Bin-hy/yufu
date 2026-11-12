//! Reviewer decisions are separate from the immutable request snapshot.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
            ALTER TABLE identity_app_approval ADD COLUMN logout_limitation_accepted BOOLEAN NOT NULL DEFAULT FALSE;
            CREATE FUNCTION identity_immutable_approval() RETURNS TRIGGER LANGUAGE plpgsql AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id OR NEW.environment_id IS DISTINCT FROM OLD.environment_id
                    OR NEW.version_id IS DISTINCT FROM OLD.version_id OR NEW.snapshot IS DISTINCT FROM OLD.snapshot
                    OR NEW.snapshot_hash IS DISTINCT FROM OLD.snapshot_hash OR NEW.requested_by IS DISTINCT FROM OLD.requested_by
                    OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
                    RAISE EXCEPTION 'identity approval snapshots are immutable' USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$;
            CREATE TRIGGER identity_approval_immutable BEFORE UPDATE ON identity_app_approval
                FOR EACH ROW EXECUTE FUNCTION identity_immutable_approval();
        "#).await?;
        // Register only missing operations; preserve existing edited grants/denies.
        crate::seed::m20261109_000004_identity_operations::Migration
            .up(manager)
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DROP TRIGGER identity_approval_immutable ON identity_app_approval; DROP FUNCTION identity_immutable_approval(); ALTER TABLE identity_app_approval DROP COLUMN logout_limitation_accepted;").await?;
        Ok(())
    }
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
            CREATE TABLE identity_audit_application (
                event_id UUID NOT NULL REFERENCES identity_audit_event(id) ON DELETE CASCADE,
                application_id INTEGER NOT NULL REFERENCES application(id),
                environment_id UUID REFERENCES identity_app_environment(id),
                PRIMARY KEY(event_id,application_id)
            );
            CREATE INDEX product_audit_application ON identity_audit_application(application_id,event_id);
            CREATE INDEX product_audit_time ON identity_audit_event(happened_at DESC,id DESC);
            CREATE INDEX product_audit_actor ON identity_audit_event(actor_id,happened_at DESC,id DESC);
            CREATE INDEX product_audit_request ON identity_audit_event(request_id);
            CREATE INDEX product_audit_action ON identity_audit_event(action,happened_at DESC,id DESC);
            CREATE INDEX product_audit_object ON identity_audit_event(object_kind,object_id,happened_at DESC,id DESC);
        "#).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
            DROP TABLE identity_audit_application;
            DROP INDEX product_audit_time;
            DROP INDEX product_audit_actor;
            DROP INDEX product_audit_request;
            DROP INDEX product_audit_action;
            DROP INDEX product_audit_object;
        "#,
            )
            .await?;
        Ok(())
    }
}

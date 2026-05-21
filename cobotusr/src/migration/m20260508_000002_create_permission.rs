use sea_orm_migration::prelude::*;

/// Per-application permission rule. Independent from the existing `authority`
/// table (which encodes URI+METHOD-based RBAC for cobotsrv's own routes) —
/// permissions describe **resource-level** access ("user X may `submit` on
/// `task:cobotlet-0:lift`") and are evaluated by the `/api/v1/enforce` PDP.
///
/// `application_id` is required: every permission belongs to an application,
/// so admins of app A can't accidentally grant rights inside app B.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Permission::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Permission::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(Permission::ApplicationId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Permission::Name).string().not_null())
                    .col(ColumnDef::new(Permission::Description).string().null())
                    // Verb / capability, e.g. "read", "write", "submit".
                    .col(ColumnDef::new(Permission::Action).string().not_null())
                    // Object class, e.g. "device", "task". NULL = action-only (no object).
                    .col(ColumnDef::new(Permission::ResourceType).string().null())
                    // Glob pattern matched against the object id, e.g.
                    // "cobotlet-0:*". NULL = matches any id.
                    .col(ColumnDef::new(Permission::ResourcePattern).string().null())
                    .col(
                        ColumnDef::new(Permission::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Permission::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_permission_application")
                    .from(Permission::Table, Permission::ApplicationId)
                    .to(Application::Table, Application::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_permission_app_name_unique")
                    .table(Permission::Table)
                    .col(Permission::ApplicationId)
                    .col(Permission::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Permission::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Permission {
    Table,
    Id,
    ApplicationId,
    Name,
    Description,
    Action,
    ResourceType,
    ResourcePattern,
    CreateTime,
    UpdateTime,
}

#[derive(DeriveIden)]
enum Application {
    Table,
    Id,
}

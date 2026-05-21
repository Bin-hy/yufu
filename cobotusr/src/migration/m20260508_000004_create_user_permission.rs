use crate::entity::user;
use sea_orm_migration::prelude::*;

/// `(user_id, permission_id, allow)` — direct user-level permission with an
/// explicit allow/deny override (mirrors `user_authority`'s ABAC pattern).
/// Resolution order during `/enforce`:
///   1. user_permission row exists → use its `allow` (deny short-circuits)
///   2. otherwise check role_permission via the user's role(s)
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserPermission::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserPermission::UserId).integer().not_null())
                    .col(
                        ColumnDef::new(UserPermission::PermissionId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserPermission::Allow).boolean().not_null())
                    .col(
                        ColumnDef::new(UserPermission::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPermission::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserPermission::UserId)
                            .col(UserPermission::PermissionId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_permission_user")
                    .from(UserPermission::Table, UserPermission::UserId)
                    .to(user::Entity, user::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_permission_permission")
                    .from(UserPermission::Table, UserPermission::PermissionId)
                    .to(Permission::Table, Permission::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserPermission::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserPermission {
    Table,
    UserId,
    PermissionId,
    Allow,
    CreateTime,
    UpdateTime,
}

#[derive(DeriveIden)]
enum Permission {
    Table,
    Id,
}

use crate::entity::role;
use sea_orm_migration::prelude::*;

/// `(role_id, permission_id)` join — assigns a permission to all members of a role.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RolePermission::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(RolePermission::RoleId).integer().not_null())
                    .col(
                        ColumnDef::new(RolePermission::PermissionId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RolePermission::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(RolePermission::RoleId)
                            .col(RolePermission::PermissionId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_role_permission_role")
                    .from(RolePermission::Table, RolePermission::RoleId)
                    .to(role::Entity, role::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_role_permission_permission")
                    .from(RolePermission::Table, RolePermission::PermissionId)
                    .to(Permission::Table, Permission::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RolePermission::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RolePermission {
    Table,
    RoleId,
    PermissionId,
    CreateTime,
}

#[derive(DeriveIden)]
enum Permission {
    Table,
    Id,
}

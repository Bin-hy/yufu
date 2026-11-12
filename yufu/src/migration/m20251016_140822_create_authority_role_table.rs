use crate::entity::{authority, role};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(AuthorityRole::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(AuthorityRole::RoleId).integer().not_null())
                    .col(
                        ColumnDef::new(AuthorityRole::AuthorityId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthorityRole::CreateTime)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthorityRole::UpdateTime)
                            .date_time()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(AuthorityRole::AuthorityId)
                            .col(AuthorityRole::RoleId),
                    )
                    .to_owned(),
            )
            .await?;
        //  外键：role_id -> role.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_authority_role_role")
                    .from(AuthorityRole::Table, AuthorityRole::RoleId)
                    .to(role::Entity, role::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // 外键：authority_id -> authority.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_authority_role_authority")
                    .from(AuthorityRole::Table, AuthorityRole::AuthorityId)
                    .to(authority::Entity, authority::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(AuthorityRole::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuthorityRole {
    Table,
    RoleId,
    AuthorityId,
    CreateTime,
    UpdateTime,
}

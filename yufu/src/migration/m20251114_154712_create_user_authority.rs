use crate::entity::{authority, user};
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
                    .table(UserAuthority::Table)
                    .comment("用于覆盖 role 的权限直接绑定在user中实现 ABAC")
                    .if_not_exists()
                    .col(ColumnDef::new(UserAuthority::UserId).integer().not_null())
                    .col(
                        ColumnDef::new(UserAuthority::AuthorityId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserAuthority::Allow).boolean().not_null())
                    .col(
                        ColumnDef::new(UserAuthority::CreateTime)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserAuthority::UpdateTime)
                            .date_time()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserAuthority::AuthorityId)
                            .col(UserAuthority::UserId),
                    )
                    .to_owned(),
            )
            .await?;
        //  外键：user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_authority_user")
                    .from(UserAuthority::Table, UserAuthority::UserId)
                    .to(user::Entity, user::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // 外键：authority_id -> authority.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_authority_authority")
                    .from(UserAuthority::Table, UserAuthority::AuthorityId)
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
            .drop_table(Table::drop().table(UserAuthority::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserAuthority {
    Table,
    UserId,
    AuthorityId,
    Allow,
    CreateTime,
    UpdateTime,
}

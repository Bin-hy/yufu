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
                    .table(User::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(User::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(User::Username).string().unique_key())
                    .col(ColumnDef::new(User::Password).string())
                    .col(ColumnDef::new(User::Nickname).string())
                    .col(ColumnDef::new(User::GroupId).string())
                    .col(ColumnDef::new(User::RoleId).string())
                    .col(ColumnDef::new(User::Description).string())
                    .col(ColumnDef::new(User::CreateTime).date_time().not_null())
                    .col(ColumnDef::new(User::UpdateTime).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
    Password,
    Username,
    CreateTime,
    UpdateTime,
    Nickname,
    GroupId,
    RoleId,
    Description,
}

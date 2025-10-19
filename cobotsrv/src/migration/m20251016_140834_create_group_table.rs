use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(Group::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Group::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Group::Name).string())
                    .col(ColumnDef::new(Group::CreateTime).date_time().not_null())
                    .col(ColumnDef::new(Group::UpdateTime).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(Group::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Group {
    Table,
    Id,
    Name,
    CreateTime,
    UpdateTime,
}

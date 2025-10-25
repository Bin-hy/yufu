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
                    .table(Recording::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Recording::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Recording::Name).string())
                    .col(ColumnDef::new(Recording::UserId).string())
                    .col(ColumnDef::new(Recording::CreateTime).date_time())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(Recording::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Recording {
    Table,
    Id,
    Name,
    UserId,
    CreateTime,
}

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
                    .table(ORefreshToken::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ORefreshToken::Jti).string().not_null())
                    .col(
                        ColumnDef::new(ORefreshToken::UserId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ORefreshToken::CreateTime)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ORefreshToken::UpdateTime)
                            .date_time()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(ORefreshToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ORefreshToken {
    Table,
    Jti,
    UserId,
    CreateTime,
    UpdateTime,
}

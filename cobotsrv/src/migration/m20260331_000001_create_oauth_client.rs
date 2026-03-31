use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OAuthClient::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthClient::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(OAuthClient::ClientId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OAuthClient::Name).string().not_null())
                    .col(
                        ColumnDef::new(OAuthClient::RedirectUris)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClient::AllowedScopes)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClient::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClient::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OAuthClient::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OAuthClient {
    Table,
    Id,
    ClientId,
    Name,
    RedirectUris,
    AllowedScopes,
    CreateTime,
    UpdateTime,
}

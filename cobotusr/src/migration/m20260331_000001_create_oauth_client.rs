use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OauthClient::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OauthClient::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(OauthClient::ClientId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OauthClient::Name).string().not_null())
                    .col(
                        ColumnDef::new(OauthClient::RedirectUris)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthClient::AllowedScopes)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthClient::ClientSecretHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthClient::GrantTypes)
                            .string()
                            .not_null()
                            .default("authorization_code,refresh_token"),
                    )
                    .col(
                        ColumnDef::new(OauthClient::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthClient::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OauthClient::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OauthClient {
    Table,
    Id,
    ClientId,
    Name,
    RedirectUris,
    AllowedScopes,
    ClientSecretHash,
    GrantTypes,
    CreateTime,
    UpdateTime,
}

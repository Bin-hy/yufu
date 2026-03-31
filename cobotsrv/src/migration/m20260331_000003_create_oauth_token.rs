use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OAuthToken::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthToken::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(OAuthToken::UserId).integer().not_null())
                    .col(ColumnDef::new(OAuthToken::ClientId).string().not_null())
                    .col(ColumnDef::new(OAuthToken::AccessToken).string().not_null())
                    .col(ColumnDef::new(OAuthToken::IdToken).string().not_null())
                    .col(
                        ColumnDef::new(OAuthToken::RefreshToken)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OAuthToken::Scope).string().null())
                    .col(ColumnDef::new(OAuthToken::TokenType).string().not_null())
                    .col(
                        ColumnDef::new(OAuthToken::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthToken::AccessExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthToken::RefreshExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthToken::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthToken::Revoked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OAuthToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OAuthToken {
    Table,
    Id,
    UserId,
    ClientId,
    AccessToken,
    IdToken,
    RefreshToken,
    Scope,
    TokenType,
    CreateTime,
    AccessExpiresAt,
    RefreshExpiresAt,
    UpdateTime,
    Revoked,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OauthToken::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OauthToken::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(OauthToken::UserId).integer().not_null())
                    .col(ColumnDef::new(OauthToken::ClientId).string().not_null())
                    .col(ColumnDef::new(OauthToken::AccessToken).string().not_null())
                    .col(ColumnDef::new(OauthToken::IdToken).string().not_null())
                    .col(
                        ColumnDef::new(OauthToken::RefreshToken)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OauthToken::Scope).string().null())
                    .col(ColumnDef::new(OauthToken::TokenType).string().not_null())
                    .col(
                        ColumnDef::new(OauthToken::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthToken::AccessExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthToken::RefreshExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthToken::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthToken::Revoked)
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
            .drop_table(Table::drop().table(OauthToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OauthToken {
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

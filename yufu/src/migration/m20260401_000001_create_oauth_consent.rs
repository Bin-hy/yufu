use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OauthConsent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OauthConsent::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(OauthConsent::UserId).integer().not_null())
                    .col(ColumnDef::new(OauthConsent::ClientId).string().not_null())
                    .col(ColumnDef::new(OauthConsent::Scope).string().not_null())
                    .col(
                        ColumnDef::new(OauthConsent::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthConsent::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // UNIQUE(user_id, client_id)
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_consent_user_client")
                    .table(OauthConsent::Table)
                    .col(OauthConsent::UserId)
                    .col(OauthConsent::ClientId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OauthConsent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OauthConsent {
    Table,
    Id,
    UserId,
    ClientId,
    Scope,
    CreateTime,
    UpdateTime,
}

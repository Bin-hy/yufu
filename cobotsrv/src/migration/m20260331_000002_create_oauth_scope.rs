use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OauthScope::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OauthScope::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(OauthScope::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OauthScope::Description).string().null())
                    .col(
                        ColumnDef::new(OauthScope::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OauthScope::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OauthScope::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OauthScope {
    Table,
    Id,
    Name,
    Description,
    CreateTime,
    UpdateTime,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OAuthScope::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthScope::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(OAuthScope::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OAuthScope::Description).string().null())
                    .col(
                        ColumnDef::new(OAuthScope::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthScope::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OAuthScope::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OAuthScope {
    Table,
    Id,
    Name,
    Description,
    CreateTime,
    UpdateTime,
}

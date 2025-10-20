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
                    .table(UserAuthority::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserAuthority::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserAuthority::UserId).string())
                    .col(ColumnDef::new(UserAuthority::AuthorityId).string())
                    .col(ColumnDef::new(UserAuthority::Allow).boolean())
                    .col(
                        ColumnDef::new(UserAuthority::CreateTime)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserAuthority::UpdateTime)
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
            .drop_table(Table::drop().table(UserAuthority::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserAuthority {
    Table,
    Id,
    UserId,
    AuthorityId,
    Allow,
    CreateTime,
    UpdateTime,
}

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
                    .table(AuthorityRole::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(AuthorityRole::RoleId).string().not_null())
                    .col(
                        ColumnDef::new(AuthorityRole::AuthorityId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthorityRole::CreateTime)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthorityRole::UpdateTime)
                            .date_time()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(AuthorityRole::AuthorityId)
                            .col(AuthorityRole::RoleId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(AuthorityRole::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuthorityRole {
    Table,
    RoleId,
    AuthorityId,
    CreateTime,
    UpdateTime,
}

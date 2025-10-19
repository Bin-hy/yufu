use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Authority::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Authority::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Authority::Authority).string())
                    .col(ColumnDef::new(Authority::CreateTime).date_time().not_null())
                    .col(ColumnDef::new(Authority::UpdateTime).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(Authority::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Authority {
    Table,
    Id,
    Authority,
    CreateTime,
    UpdateTime,
}
// #[sea_orm(primary_key, auto_increment = false)]
// pub id: String,
// pub authority: Option<String>,
// pub create_time: DateTime,
// pub update_time: DateTime,

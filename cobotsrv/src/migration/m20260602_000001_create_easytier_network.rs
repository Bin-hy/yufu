use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EasytierNetwork::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EasytierNetwork::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EasytierNetwork::RobotSn)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(EasytierNetwork::VirtualIp)
                            .string()
                            .null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(EasytierNetwork::Status)
                            .string()
                            .not_null()
                            .default("ACTIVE"),
                    )
                    .col(
                        ColumnDef::new(EasytierNetwork::AllocatedAt)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EasytierNetwork::LastOnlineAt)
                            .date_time()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EasytierNetwork::LeaseExpiredAt)
                            .date_time()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EasytierNetwork::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum EasytierNetwork {
    Table,
    Id,
    RobotSn,
    VirtualIp,
    Status,
    AllocatedAt,
    LastOnlineAt,
    LeaseExpiredAt,
}

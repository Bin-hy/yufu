use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Device::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Device::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Device::CobotletAlias).string().not_null())
                    .col(ColumnDef::new(Device::DeviceId).string().not_null())
                    .col(ColumnDef::new(Device::ServiceUrl).string().not_null())
                    .col(ColumnDef::new(Device::Capabilities).json().not_null())
                    .col(
                        ColumnDef::new(Device::Online)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Device::LastHeartbeat).date_time().not_null())
                    .col(ColumnDef::new(Device::RegisteredAt).date_time().not_null())
                    .col(ColumnDef::new(Device::UpdateTime).date_time().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_device_alias_device_id")
                    .table(Device::Table)
                    .col(Device::CobotletAlias)
                    .col(Device::DeviceId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Device::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Device {
    Table,
    Id,
    CobotletAlias,
    DeviceId,
    ServiceUrl,
    Capabilities,
    Online,
    LastHeartbeat,
    RegisteredAt,
    UpdateTime,
}

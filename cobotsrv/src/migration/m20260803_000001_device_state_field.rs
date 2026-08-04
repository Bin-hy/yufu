use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Device::Table)
                    .add_column(
                        ColumnDef::new(Device::State)
                            .string()
                            .not_null()
                            .default("offline"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE device SET state = CASE WHEN online = true THEN 'ready' ELSE 'offline' END",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Device::Table)
                    .drop_column(Device::Online)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Device::Table)
                    .add_column(
                        ColumnDef::new(Device::Online)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared("UPDATE device SET online = (state = 'ready' OR state = 'busy')")
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Device::Table)
                    .drop_column(Device::State)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Device {
    Table,
    Online,
    State,
}

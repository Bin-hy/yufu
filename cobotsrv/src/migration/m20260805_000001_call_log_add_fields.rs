use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(McpCallLog::Table)
                    .add_column(ColumnDef::new(McpCallLog::InputParams).json_binary().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(McpCallLog::Table)
                    .add_column(ColumnDef::new(McpCallLog::StatusCode).integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(McpCallLog::Table)
                    .add_column(ColumnDef::new(McpCallLog::DeviceId).string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(McpCallLog::Table)
                    .add_column(ColumnDef::new(McpCallLog::CobotletAlias).string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(McpCallLog::Table)
                    .add_column(ColumnDef::new(McpCallLog::Source).string().null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            McpCallLog::Source,
            McpCallLog::CobotletAlias,
            McpCallLog::DeviceId,
            McpCallLog::StatusCode,
            McpCallLog::InputParams,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(McpCallLog::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum McpCallLog {
    Table,
    InputParams,
    StatusCode,
    DeviceId,
    CobotletAlias,
    Source,
}

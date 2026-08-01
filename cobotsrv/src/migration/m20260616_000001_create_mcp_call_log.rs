use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(McpCallLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(McpCallLog::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(McpCallLog::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(McpCallLog::Subject).string().not_null())
                    .col(ColumnDef::new(McpCallLog::Method).string().not_null())
                    .col(ColumnDef::new(McpCallLog::ToolName).string().null())
                    // Free-form tool/DB error text — must not be length-capped or
                    // overlong errors fail the insert and the record is lost.
                    .col(ColumnDef::new(McpCallLog::ErrorMsg).text().null())
                    .col(
                        ColumnDef::new(McpCallLog::DurationMs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(McpCallLog::RequestId).string().null())
                    .to_owned(),
            )
            .await?;

        // Index on created_at for time-ordered audit queries.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_call_log_created_at")
                    .table(McpCallLog::Table)
                    .col(McpCallLog::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Composite index for the common subject-filtered, time-ordered queries.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_call_log_subject_created_at")
                    .table(McpCallLog::Table)
                    .col(McpCallLog::Subject)
                    .col(McpCallLog::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(McpCallLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum McpCallLog {
    Table,
    Id,
    CreatedAt,
    Subject,
    Method,
    ToolName,
    ErrorMsg,
    DurationMs,
    RequestId,
}

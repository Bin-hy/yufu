use crate::entity::user;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(McpToken::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(McpToken::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(McpToken::UserId).integer().not_null())
                    .col(ColumnDef::new(McpToken::Name).string().not_null())
                    .col(
                        ColumnDef::new(McpToken::Jti)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(McpToken::Aud).string().not_null())
                    .col(ColumnDef::new(McpToken::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(McpToken::ExpiresAt).timestamp().not_null())
                    .col(
                        ColumnDef::new(McpToken::Revoked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // 外键：user_id -> user.id（删除用户时级联删除其 MCP 令牌记录）
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_mcp_token_user")
                    .from(McpToken::Table, McpToken::UserId)
                    .to(user::Entity, user::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Index on user_id for the owner's "list my tokens" query.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_token_user_id")
                    .table(McpToken::Table)
                    .col(McpToken::UserId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(McpToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum McpToken {
    Table,
    Id,
    UserId,
    Name,
    Jti,
    Aud,
    CreatedAt,
    ExpiresAt,
    Revoked,
}

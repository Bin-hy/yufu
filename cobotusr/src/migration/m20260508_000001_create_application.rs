use sea_orm_migration::prelude::*;

/// Per-application identity. An OIDC client_id is the public identifier
/// the SDK/browser uses; an `app_key` is the human/admin-facing handle for
/// permissions and routing (e.g. `cobotrix`). They're 1:1 in practice but
/// kept separate so apps without an OIDC client (M2M-only) can still exist.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Application::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Application::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(Application::AppKey)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Application::Name).string().not_null())
                    .col(ColumnDef::new(Application::Description).string().null())
                    .col(ColumnDef::new(Application::OidcClientId).integer().null())
                    .col(
                        ColumnDef::new(Application::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Application::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_application_oidc_client")
                    .from(Application::Table, Application::OidcClientId)
                    .to(OauthClient::Table, OauthClient::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Application::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Application {
    Table,
    Id,
    AppKey,
    Name,
    Description,
    OidcClientId,
    CreateTime,
    UpdateTime,
}

#[derive(DeriveIden)]
enum OauthClient {
    Table,
    Id,
}

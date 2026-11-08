use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE security_session_client (
                    issuer TEXT NOT NULL CHECK (issuer<>''),
                    client_id VARCHAR(512) NOT NULL REFERENCES oauth_client(client_id) ON DELETE RESTRICT,
                    audience VARCHAR(512) NOT NULL CHECK (audience IN ('cobotusr','cobotsrv')),
                    application VARCHAR(512) NOT NULL CHECK (application=audience),
                    enabled BOOLEAN NOT NULL DEFAULT FALSE,
                    PRIMARY KEY (issuer,client_id)
                 );
                 ALTER TABLE security_credential ADD COLUMN paired_access_jti VARCHAR(512),
             ADD COLUMN oidc_client_id VARCHAR(512),
             ADD CONSTRAINT security_session_client_ref FOREIGN KEY (issuer,oidc_client_id)
                REFERENCES security_session_client(issuer,client_id) ON DELETE RESTRICT,
             ADD CONSTRAINT security_refresh_access_pair FOREIGN KEY (issuer,paired_access_jti)
                REFERENCES security_credential(issuer,jti) ON DELETE RESTRICT,
             ADD CONSTRAINT security_refresh_access_pair_kind CHECK (paired_access_jti IS NULL OR
                (credential_kind='refresh' AND paired_access_jti<>jti));",
            )
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE security_credential DROP COLUMN paired_access_jti, DROP COLUMN oidc_client_id; DROP TABLE security_session_client")
            .await?;
        Ok(())
    }
}

use crate::entity::oauth_scope;
use sea_orm::{ActiveModelTrait, ActiveValue, Set};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const OIDC_SCOPES: &[(&str, &str)] = &[
    ("openid", "OpenID Connect base scope"),
    ("profile", "Access to user profile (name, nickname)"),
    ("email", "Access to user email address"),
    ("phone", "Access to user phone number"),
    ("address", "Access to user address"),
    ("offline_access", "Request refresh token for offline access"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let now = chrono::Utc::now().naive_utc();
        for (name, desc) in OIDC_SCOPES {
            oauth_scope::ActiveModel {
                id: ActiveValue::NotSet,
                name: Set((*name).to_owned()),
                description: Set(Some((*desc).to_owned())),
                create_time: Set(now),
                update_time: Set(now),
            }
            .insert(conn)
            .await?;
        }
        Ok(())
    }
}

//! Append only branding registrations; never recreate unrelated removed operations.
use crate::{domain::identity::operation::IdentityOperation, entity::authority};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let at = chrono::Utc::now().naive_utc();
        for operation in [
            IdentityOperation::SiteSettingsRead,
            IdentityOperation::SiteSettingsSave,
            IdentityOperation::SiteSettingsReset,
            IdentityOperation::SiteAssetUpload,
            IdentityOperation::SiteAssetPreview,
        ] {
            let (method, path) = operation.registration();
            if authority::Entity::find()
                .filter(authority::Column::Method.eq(method))
                .filter(authority::Column::AuthorityUri.eq(path))
                .one(db)
                .await?
                .is_some()
            {
                continue;
            }
            authority::ActiveModel {
                id: sea_orm::NotSet,
                name: Set(format!("identity.uri:{method}:{path}")),
                method: Set(method.into()),
                authority_uri: Set(path.into()),
                create_time: Set(at),
                update_time: Set(at),
            }
            .insert(db)
            .await?;
        }
        // Current Superadmin role is handled by the existing exact operation checker.
        // No enterprise role grants or user overrides are copied or replaced.
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

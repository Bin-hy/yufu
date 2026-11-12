//! Register enterprise operations without restoring deliberately revoked grants.
use crate::domain::identity::operation::IdentityOperation;
use crate::entity::{authority, authority_role, role, user_authority};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let at = chrono::Utc::now().naive_utc();
        for &operation in IdentityOperation::ALL {
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
            let row = authority::ActiveModel {
                id: sea_orm::NotSet,
                name: Set(format!("identity.uri:{method}:{path}")),
                method: Set(method.into()),
                authority_uri: Set(path.into()),
                create_time: Set(at),
                update_time: Set(at),
            }
            .insert(db)
            .await?;
            let (legacy_method, legacy_path) = operation.legacy_registration();
            let legacy = authority::Entity::find()
                .filter(authority::Column::Method.eq(legacy_method))
                .filter(authority::Column::AuthorityUri.eq(legacy_path))
                .one(db)
                .await?;
            let legacy_grants = match &legacy {
                Some(legacy) => {
                    authority_role::Entity::find()
                        .filter(authority_role::Column::AuthorityId.eq(legacy.id))
                        .all(db)
                        .await?
                }
                None => vec![],
            };
            for role in role::Entity::find().all(db).await? {
                if role.name == role::RoleEnum::Superadmin.get_name()
                    || legacy_grants.iter().any(|grant| grant.role_id == role.id)
                {
                    authority_role::ActiveModel {
                        role_id: Set(role.id),
                        authority_id: Set(row.id),
                        create_time: Set(at),
                        update_time: Set(at),
                    }
                    .insert(db)
                    .await?;
                }
            }
            if let Some(legacy) = legacy {
                // Carry both grants and explicit denials across the renamed
                // operation. A superadmin's legacy denial must not disappear.
                for entry in user_authority::Entity::find()
                    .filter(user_authority::Column::AuthorityId.eq(legacy.id))
                    .all(db)
                    .await?
                {
                    user_authority::ActiveModel {
                        user_id: Set(entry.user_id),
                        authority_id: Set(row.id),
                        allow: Set(entry.allow),
                        create_time: Set(at),
                        update_time: Set(at),
                    }
                    .insert(db)
                    .await?;
                }
            }
        }
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Registrations have no identity-table dependency; preserve administrator edits.
        Ok(())
    }
}

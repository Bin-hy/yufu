//! Versioned method-aware registration of authentication administration
//! and owner-facing account operations.
use crate::entity::{authority, authority_role, role};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
const MANAGEMENT: &[(&str, &str)] = &[
    ("POST", "/authority"),
    ("GET", "/authority"),
    ("PATCH", "/authority"),
    ("DELETE", "/authority"),
    ("POST", "/role"),
    ("GET", "/role"),
    ("PATCH", "/role"),
    ("DELETE", "/role"),
    ("POST", "/role_authority"),
    ("DELETE", "/role_authority"),
    ("GET", "/role_authority/{role_id}"),
    ("POST", "/role_authority/batch"),
    ("DELETE", "/role_authority/batch"),
    ("GET", "/role/modifiable"),
    ("GET", "/recordings"),
    ("GET", "/recordings/{user_id}"),
    ("GET", "/users"),
    ("POST", "/user"),
    ("PATCH", "/user"),
    ("PATCH", "/user/role"),
    ("DELETE", "/user/{user_id}"),
    ("POST", "/user_authority_override"),
    ("GET", "/user_authority_override"),
    ("PATCH", "/user_authority_override"),
    ("DELETE", "/user_authority_override"),
    ("POST", "/application"),
    ("GET", "/application"),
    ("PATCH", "/application"),
    ("DELETE", "/application"),
    ("POST", "/permission"),
    ("GET", "/permission"),
    ("PATCH", "/permission"),
    ("DELETE", "/permission"),
    ("POST", "/role_permission"),
    ("DELETE", "/role_permission"),
    ("GET", "/role_permission/{role_id}"),
    ("POST", "/role_permission/batch"),
    ("DELETE", "/role_permission/batch"),
    ("POST", "/user_permission_override"),
    ("GET", "/user_permission_override"),
    ("PATCH", "/user_permission_override"),
    ("DELETE", "/user_permission_override"),
    ("GET", "/oidc/clients"),
    ("POST", "/oidc/clients"),
    ("GET", "/oidc/client/{id}"),
    ("PATCH", "/oidc/client/{id}"),
    ("DELETE", "/oidc/client/{id}"),
    ("POST", "/oidc/client/{id}/secret"),
    ("GET", "/oidc/scope"),
    ("POST", "/oidc/scope"),
    ("POST", "/kickout"),
    ("POST", "/enforce"),
];
// These existing owner-facing handlers also check the authenticated owner or
// account ownership. Registering them preserves their existing purpose;
// it grants no role-writing or gateway/device execution permission.
const PERSONAL: &[(&str, &str)] = &[
    ("GET", "/user"),
    ("GET", "/oidc/userinfo"),
    ("GET", "/oidc/authorize/client-info"),
    ("POST", "/oidc/authorize/consent"),
    ("GET", "/oidc/authorized-apps"),
    ("DELETE", "/oidc/authorized-apps/{consent_id}"),
    ("POST", "/oidc/mcp-tokens"),
    ("GET", "/oidc/mcp-tokens"),
    ("PATCH", "/oidc/mcp-tokens/{id}"),
    ("DELETE", "/oidc/mcp-tokens/{id}"),
    ("POST", "/oidc/mcp-tokens/{id}/refresh"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let now = chrono::Utc::now().naive_utc();
        let mut created = std::collections::HashSet::new();
        for &(method, path) in MANAGEMENT.iter().chain(PERSONAL) {
            if authority::Entity::find()
                .filter(authority::Column::Method.eq(method))
                .filter(authority::Column::AuthorityUri.eq(path))
                .one(db)
                .await?
                .is_none()
            {
                let inserted = authority::ActiveModel {
                    id: sea_orm::NotSet,
                    name: Set(format!("security.uri:{method}:{path}")),
                    method: Set(method.into()),
                    authority_uri: Set(path.into()),
                    create_time: Set(now),
                    update_time: Set(now),
                }
                .insert(db)
                .await?;
                created.insert(inserted.id);
            }
        }
        let roles = role::Entity::find()
            .filter(role::Column::Name.is_in([
                role::RoleEnum::User.get_name(),
                role::RoleEnum::Admin.get_name(),
                role::RoleEnum::Superadmin.get_name(),
            ]))
            .all(db)
            .await?;
        for role in roles {
            for &(method, path) in PERSONAL {
                let row = authority::Entity::find()
                    .filter(authority::Column::Method.eq(method))
                    .filter(authority::Column::AuthorityUri.eq(path))
                    .one(db)
                    .await?
                    .ok_or_else(|| DbErr::Custom("registered personal authority missing".into()))?;
                // Existing catalog rows may have deliberately restricted grants.
                // Repairing omitted methods must never re-grant an administrator's
                // previously revoked role permission.
                if !created.contains(&row.id) {
                    continue;
                }
                if authority_role::Entity::find_by_id((role.id, row.id))
                    .one(db)
                    .await?
                    .is_none()
                {
                    authority_role::ActiveModel {
                        role_id: Set(role.id),
                        authority_id: Set(row.id),
                        create_time: Set(now),
                        update_time: Set(now),
                    }
                    .insert(db)
                    .await?;
                }
            }
        }
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Do not erase administrator-edited registrations or grants on schema
        // rollback. These rows reference only legacy tables; up is idempotent.
        Ok(())
    }
}

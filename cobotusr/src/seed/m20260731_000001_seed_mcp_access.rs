//! Seed `mcp:access` for cobotsrv and grant it to the default `User` role.
//!
//! `POST /api/v1/oidc/mcp-tokens` requires the caller to hold `mcp:access` for the
//! target audience (see route/mcp.rs). cobotsrv declares the permission at startup via
//! `/application/sync-permissions`, but nothing granted it to any role — so out of the
//! box only the superadmin (implicit all-permissions) could mint MCP tokens. This seed
//! upserts the application + permission rows (same shape as the sync endpoint, fully
//! idempotent) and grants the permission to the default `User` role so ordinary
//! authenticated users can mint MCP tokens for cobotsrv.

use crate::entity::role::RoleEnum;
use crate::entity::{application, permission, role, role_permission};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Audience / app_key of the MCP resource server (matches cobotsrv's OIDC client_id).
const MCP_APP_KEY: &str = "cobotsrv";
const MCP_PERMISSION: &str = "mcp:access";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let now = chrono::Utc::now().naive_utc();

        // Application row (identical to the OIDC-bootstrap / sync-permissions upsert;
        // both skip creation when the app_key already exists).
        let app = match application::Entity::find()
            .filter(application::Column::AppKey.eq(MCP_APP_KEY))
            .one(conn)
            .await?
        {
            Some(a) => a,
            None => {
                application::ActiveModel {
                    id: ActiveValue::NotSet,
                    app_key: Set(MCP_APP_KEY.to_string()),
                    name: Set("CoBotrix Device Management Server".to_string()),
                    description: Set(None),
                    oidc_client_id: Set(None),
                    create_time: Set(now),
                    update_time: Set(now),
                }
                .insert(conn)
                .await?
            }
        };

        // Permission row (identical to cobotsrv's [[permissions]] declaration).
        let perm = match permission::Entity::find()
            .filter(permission::Column::ApplicationId.eq(app.id))
            .filter(permission::Column::Name.eq(MCP_PERMISSION))
            .one(conn)
            .await?
        {
            Some(p) => p,
            None => {
                permission::ActiveModel {
                    id: ActiveValue::NotSet,
                    application_id: Set(app.id),
                    name: Set(MCP_PERMISSION.to_string()),
                    description: Set(None),
                    action: Set("access".to_string()),
                    resource_type: Set(Some("mcp".to_string())),
                    resource_pattern: Set(Some("*".to_string())),
                    create_time: Set(now),
                    update_time: Set(now),
                }
                .insert(conn)
                .await?
            }
        };

        // Grant to the default user role (superadmin holds every permission implicitly).
        let user_role = role::Entity::find()
            .filter(role::Column::Name.eq(RoleEnum::User.get_name()))
            .one(conn)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("default User role missing".to_string()))?;

        let granted = role_permission::Entity::find()
            .filter(role_permission::Column::RoleId.eq(user_role.id))
            .filter(role_permission::Column::PermissionId.eq(perm.id))
            .one(conn)
            .await?;
        if granted.is_none() {
            role_permission::ActiveModel {
                role_id: Set(user_role.id),
                permission_id: Set(perm.id),
                create_time: Set(now),
            }
            .insert(conn)
            .await?;
        }
        Ok(())
    }
}

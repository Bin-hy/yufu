use crate::IdType;
use crate::entity::authority::AuthorityVO;
use crate::entity::role::RoleEnum;
use crate::entity::{authority, authority_role, role};
use crate::error::AppError;
use crate::result::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;

pub struct Service;

/// URI operations are registered by the authority row, never inferred from an
/// unregistered request. Resource binding also includes the exact HTTP method.
pub fn uri_operation(
    authority_id: IdType,
) -> std::result::Result<auth::security::policy::OperationId, auth::security::error::SecurityError>
{
    if authority_id <= 0 {
        return Err(auth::security::error::SecurityError::InvalidResource);
    }
    auth::security::policy::OperationId::new(format!("idp.uri:{authority_id}"))
}
pub fn uri_resource(
    application: &str,
    method: &str,
    route: &str,
) -> std::result::Result<auth::security::policy::ResourceRef, auth::security::error::SecurityError>
{
    use auth::security::{
        error::SecurityError,
        policy::{ResourceRef, ResourceTarget},
    };
    let method = method.to_ascii_uppercase();
    http::Method::from_bytes(method.as_bytes()).map_err(|_| SecurityError::InvalidResource)?;
    if !route.starts_with('/') || route.contains(['?', '#']) {
        return Err(SecurityError::InvalidResource);
    }
    ResourceRef::new(
        application,
        ResourceTarget::IdpRoute {
            route_id: format!("{method} {route}"),
        },
    )
}

impl Service {
    // 查找 authority_id 的 authority_vo
    pub async fn get_authority_vo(
        db: &impl sea_orm::ConnectionTrait,
        authority_id: IdType,
    ) -> Result<AuthorityVO> {
        if let Some(db_authority) = authority::Entity::find_by_id(authority_id).one(db).await? {
            Ok(AuthorityVO::from(db_authority))
        } else {
            Err(AppError::NotFound("找不到指定权限".to_string()))
        }
    }

    // 给超级管理员添加权限
    pub async fn ensure_sadmin_authorities(db: &impl sea_orm::ConnectionTrait) -> Result<()> {
        // 1. 获取 超级管理员 角色 role
        let super_admin_name = RoleEnum::Superadmin.get_name();
        let role_db = match role::Entity::find()
            .filter(role::Column::Name.eq(super_admin_name))
            .one(db)
            .await?
        {
            Some(r) => r,
            None => {
                tracing::warn!("Superadmin role not found, skipping authority sync");
                return Ok(());
            }
        };
        // 2. 获取所有权限
        let authorities_db = authority::Entity::find().all(db).await?;
        let existing = authority_role::Entity::find()
            .filter(authority_role::Column::RoleId.eq(role_db.id))
            .all(db)
            .await?;
        // 3. 同步所有权限
        let existing_ids: HashSet<IdType> = existing.iter().map(|r| r.authority_id).collect();
        for auth in &authorities_db {
            if !existing_ids.contains(&auth.id) {
                authority_role::ActiveModel {
                    role_id: Set(role_db.id),
                    authority_id: Set(auth.id),
                    create_time: Set(Utc::now().naive_utc().to_owned()),
                    update_time: Set(Utc::now().naive_utc().to_owned()),
                }
                .insert(db)
                .await?;
            }
        }
        tracing::info!("sadmin synced with all authorities");
        Ok(())
    }
}

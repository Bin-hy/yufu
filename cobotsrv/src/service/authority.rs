use crate::IdType;
use crate::entity::authority::AuthorityVO;
use crate::entity::role::RoleEnum;
use crate::entity::{authority, authority_role, role};
use crate::error::AppError;
use crate::result::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;

pub struct Service;

impl Service {
    // 查找 authority_id 的 authority_vo
    pub async fn get_authority_vo(
        db: &DatabaseConnection,
        authority_id: IdType,
    ) -> Result<AuthorityVO> {
        if let Some(db_authority) = authority::Entity::find_by_id(authority_id).one(db).await? {
            Ok(AuthorityVO::from(db_authority))
        } else {
            Err(AppError::NotFound("找不到指定权限".to_string()))
        }
    }

    // 给超级管理员添加权限
    pub async fn ensure_sadmin_authorities(db: &DatabaseConnection) -> Result<()> {
        // 1. 获取 超级管理员 角色 role
        let super_admin_name = RoleEnum::Superadmin.get_name();
        let role_db = role::Entity::find()
            .filter(role::Column::Name.eq(super_admin_name))
            .one(db)
            .await?
            .unwrap();
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

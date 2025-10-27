use crate::entity::role;
use crate::entity::role::RoleVO;
use crate::error::AppError;
use crate::result::Result;
use sea_orm::{DatabaseConnection, EntityTrait};
pub struct Service;

impl Service {
    // 查找 role_id 的 role_vo
    pub async fn get_role_vo(db: &DatabaseConnection, authority_id: String) -> Result<RoleVO> {
        if let Some(db_role) = role::Entity::find_by_id(authority_id).one(db).await? {
            Ok(RoleVO::from(db_role))
        } else {
            Err(AppError::NotFound("找不到指定权限".to_string()))
        }
    }
}

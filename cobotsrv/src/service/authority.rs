use crate::entity::authority;
use crate::entity::authority::AuthorityVO;
use crate::error::AppError;
use crate::result::Result;
use sea_orm::{DatabaseConnection, EntityTrait};
pub struct Service;

impl Service {
    // 查找 authority_id 的 authority_vo
    pub async fn get_authority_vo(
        db: &DatabaseConnection,
        authority_id: String,
    ) -> Result<AuthorityVO> {
        if let Some(db_authority) = authority::Entity::find_by_id(authority_id).one(db).await? {
            Ok(AuthorityVO::from(db_authority))
        } else {
            Err(AppError::NotFound("找不到指定权限".to_string()))
        }
    }
}

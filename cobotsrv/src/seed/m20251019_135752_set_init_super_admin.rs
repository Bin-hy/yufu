use crate::entity::role::RoleEnum;
use crate::entity::user::ActiveModel as User;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert SuperAdmin
        let sadmin_username = "sadmin";
        User {
            id: ActiveValue::NotSet,
            username: Set(sadmin_username.to_string()),
            password: Set(Some("sadmin".to_string())),
            nickname: Set(Some("sadmin".to_string())),
            role_id: Set(RoleEnum::Superadmin.default_id()), // 绑定到 Superadmin 角色
            description: Set(Some("The System Admin.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }
}

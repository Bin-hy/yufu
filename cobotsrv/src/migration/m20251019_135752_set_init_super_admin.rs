use crate::entity::user::{self, ActiveModel as User};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert SuperAdmin
        let _ = User {
            id: Set("1".into()),
            username: Set(Some("sadmin".to_string())),
            password: Set(Some("sadmin".to_string())),
            nickname: Set(Some("sadmin".to_string())),
            role_id: Set(Some("1".to_string())),
            description: Set(Some("The System Admin.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
            .insert(conn)
            .await?;
        // insert Admin
        let _ = User {
            id: Set("2".into()),
            username: Set(Some("admin".to_string())),
            password: Set(Some("admin".to_string())),
            nickname: Set(Some("admin".to_string())),
            role_id: Set(Some("2".to_string())),
            description: Set(Some("The System Admin.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        // 插入 User 角色
        let _ = User {
            id: Set("3".into()),
            username: Set(Some("user".to_string())),
            password: Set(Some("user".to_string())),
            nickname: Set(Some("basic user".to_string())),
            role_id: Set(Some("3".to_string())),
            description: Set(Some("The normal user.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // delete 'sAdmin', 'admin' , 'User' by id
        user::Entity::delete_by_id("1".to_string())
            .exec(conn)
            .await?;

        user::Entity::delete_by_id("2".to_string())
            .exec(conn)
            .await?;

        user::Entity::delete_by_id("3".to_string())
            .exec(conn)
            .await?;

        Ok(())
    }
}

use sea_orm_migration::{prelude::*, schema::*};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use crate::entity::user::{self, ActiveModel as User};
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert Admin
        let _ = User {
            id: Set("1".into()),
            username: Set(Some("admin".to_string())),
            password: Set(Some("admin".to_string())),
            nickname: Set(Some("admin".to_string())),
            group_id: Set(Some("1".to_string())),
            role_id: Set(Some("1".to_string())),
            description: Set(Some("The System Admin.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
            .insert(conn)
            .await?;

        // 插入 User 角色
        let _ = User {
            id: Set("2".into()),
            username: Set(Some("user".to_string())),
            password: Set(Some("user".to_string())),
            nickname: Set(Some("basic user".to_string())),
            group_id: Set(Some("2".to_string())),
            role_id: Set(Some("2".to_string())),
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

        // delete 'Admin' , 'User' by id
        user::Entity::delete_by_id("1".to_string())
            .exec(conn)
            .await?;

        user::Entity::delete_by_id("2".to_string())
            .exec(conn)
            .await?;

        Ok(())
    }
}

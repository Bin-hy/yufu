use crate::entity::role;
use crate::entity::role::ActiveModel as Role;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert Admin
        let _ = Role {
            id: Set("1".into()),
            name: Set(Some("SuperAdmin".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        // 插入 admin 角色
        let _ = Role {
            id: Set("2".into()),
            name: Set(Some("Admin".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
            .insert(conn)
            .await?;

        // 插入 User 角色
        let _ = Role {
            id: Set("3".into()),
            name: Set(Some("User".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // delete 'Admin' , 'User' by id
        role::Entity::delete_by_id("1".to_string())
            .exec(conn)
            .await?;

        role::Entity::delete_by_id("2".to_string())
            .exec(conn)
            .await?;

        Ok(())
    }
}

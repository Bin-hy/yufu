use crate::entity::user::ActiveModel as User;
use sea_orm::{ActiveModelTrait, Set};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert SuperAdmin
        User {
            id: Set("1".into()),
            username: Set("sadmin".to_string()),
            password: Set(Some("sadmin".to_string())),
            nickname: Set(Some("sadmin".to_string())),
            role_id: Set("1".to_string()),
            description: Set(Some("The System Admin.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;
        // insert Admin
        User {
            id: Set("2".into()),
            username: Set("admin".to_string()),
            password: Set(Some("admin".to_string())),
            nickname: Set(Some("admin".to_string())),
            role_id: Set("2".to_string()),
            description: Set(Some("The System Admin.".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }
}

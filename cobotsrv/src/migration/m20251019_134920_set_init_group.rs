use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use sea_orm_migration::{prelude::*, schema::*};
use crate::entity::group::{self ,ActiveModel as Group};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert group
        let _ = Group {
            id: Set("1".into()),
            name: Set(Some("SuperAdminGroup".to_string())),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        // insert normal group
        let _ = Group {
            id: Set("2".into()),
            name: Set(Some("NormalGroup".to_string())),
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

        // delete 'SuperAdminGroup' , 'NormalGroup' by id
        group::Entity::delete_by_id("1".to_string())
            .exec(conn)
            .await?;

        group::Entity::delete_by_id("2".to_string())
            .exec(conn)
            .await?;

        Ok(())
    }
}

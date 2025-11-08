use crate::entity::role;
use crate::entity::role::RoleEnum;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        role::ActiveModel {
            id: ActiveValue::NotSet, // 你可以改成 ActiveValue::NotSet 如果想让它自增
            name: Set(RoleEnum::Superadmin.get_name().to_string()),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        role::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(RoleEnum::Admin.get_name().to_string()),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        role::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(RoleEnum::User.get_name().to_string()),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }
}

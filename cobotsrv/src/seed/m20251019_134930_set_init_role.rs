use crate::entity::role::{ActiveModel as Role, RoleEnum};
use sea_orm::{ActiveModelTrait, Set};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();

        // insert Superadmin
        Role {
            id: Set(RoleEnum::Superadmin.default_id().to_owned()),
            name: Set(RoleEnum::Superadmin.get_name().to_string()),
            role_enum: Set(RoleEnum::Superadmin.as_str().to_string()),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        // insert Admin
        Role {
            id: Set(RoleEnum::Admin.default_id().to_owned()),
            name: Set(RoleEnum::Admin.get_name().to_string()),
            role_enum: Set(RoleEnum::Admin.as_str().to_string()),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Role {
            id: Set(RoleEnum::User.default_id().to_owned()),
            name: Set(RoleEnum::User.get_name().to_string()),
            role_enum: Set(RoleEnum::User.as_str().to_string()),
            create_time: Set(chrono::Utc::now().naive_utc()),
            update_time: Set(chrono::Utc::now().naive_utc()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }
}

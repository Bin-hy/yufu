use crate::entity::authority;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let conn = manager.get_connection();
        // authority parts
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("添加权限".to_string()),
            authority: Set("post?/authority".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("更新权限".to_string()),
            authority: Set("patch?/authority".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("删除权限".to_string()),
            authority: Set("delete?/authority".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("获取所有权限".to_string()),
            authority: Set("get?/authority".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        // role parts
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("添加新角色".to_string()),
            authority: Set("post?/role".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("获取所有角色".to_string()),
            authority: Set("get?/role".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("更新角色".to_string()),
            authority: Set("patch?/role".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("删除角色".to_string()),
            authority: Set("delete?/role".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        // recordings parts
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("分页获取用户记录".to_string()),
            authority: Set("get?/recordings/{user_id}".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("获取所有用户记录".to_string()),
            authority: Set("get?/recordings".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;

        // user parts
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("分页获取所有用户信息".to_string()),
            authority: Set("get?/users".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("获取当前用户信息".to_string()),
            authority: Set("get?/user".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("更新用户信息角色".to_string()),
            authority: Set("patch?/user/role".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;
        authority::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set("更新用户基础信息".to_string()),
            authority: Set("patch?/user".to_string()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
            update_time: Set(Utc::now().naive_utc().to_owned()),
        }
        .insert(conn)
        .await?;
        Ok(())
    }
}

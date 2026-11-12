use crate::entity::authority;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let now = Utc::now().naive_utc();

        let seeds: Vec<(&str, &str, &str)> = vec![
            // application
            ("添加应用", "POST", "/application"),
            ("获取所有应用", "GET", "/application"),
            ("更新应用", "PATCH", "/application"),
            ("删除应用", "DELETE", "/application"),
            // permission
            ("添加权限规则", "POST", "/permission"),
            ("获取所有权限规则", "GET", "/permission"),
            ("更新权限规则", "PATCH", "/permission"),
            ("删除权限规则", "DELETE", "/permission"),
            // role_permission
            ("给角色添加权限规则", "POST", "/role_permission"),
            ("给角色删除权限规则", "DELETE", "/role_permission"),
            ("批量给角色添加权限规则", "POST", "/role_permission/batch"),
            ("批量删除角色权限规则", "DELETE", "/role_permission/batch"),
            ("获取角色权限规则详情", "GET", "/role_permission/{role_id}"),
            // user_permission
            ("添加用户权限覆写", "POST", "/user_permission_override"),
            ("删除用户权限覆写", "DELETE", "/user_permission_override"),
            ("更新用户权限覆写", "PATCH", "/user_permission_override"),
            ("获取用户权限覆写列表", "GET", "/user_permission_override"),
        ];

        for (name, method, uri) in seeds.into_iter() {
            let exists = authority::Entity::find()
                .filter(authority::Column::AuthorityUri.eq(uri))
                .one(conn)
                .await?;

            if exists.is_none() {
                authority::ActiveModel {
                    id: ActiveValue::NotSet,
                    name: Set(name.to_string()),
                    method: Set(method.to_string()),
                    authority_uri: Set(uri.to_string()),
                    create_time: Set(now),
                    update_time: Set(now),
                }
                .insert(conn)
                .await?;
            }
        }
        Ok(())
    }
}

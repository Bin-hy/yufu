use crate::entity::authority;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 权限初始化：幂等插入 + 事务包裹，避免重复插入导致唯一约束冲突
        let conn = manager.get_connection();
        let now = Utc::now().naive_utc();

        // 统一维护权限清单，覆盖所有受保护接口
        let seeds: Vec<(&str, &str, &str)> = vec![
            // authority
            ("添加权限", "POST", "/authority"),
            ("更新权限", "PATCH", "/authority"),
            ("删除权限", "DELETE", "/authority"),
            ("获取所有权限", "GET", "/authority"),
            // role
            ("添加新角色", "POST", "/role"),
            ("获取所有角色", "GET", "/role"),
            ("更新角色", "PATCH", "/role"),
            ("删除角色", "DELETE", "/role"),
            // role_authority
            ("给角色添加权限", "POST", "/role_authority"),
            ("给角色删除权限", "DELETE", "/role_authority"),
            ("批量给角色添加权限", "POST", "/role_authority/batch"),
            ("批量删除角色权限", "DELETE", "/role_authority/batch"),
            ("获取角色权限详情", "GET", "/role_authority/{role_id}"),
            // role/modifiable
            ("获取可修改角色", "GET", "/role/modifiable"),
            // recordings
            ("分页获取用户记录", "GET", "/recordings/{user_id}"),
            ("获取所有用户记录", "GET", "/recordings"),
            // user
            ("分页获取所有用户信息", "GET", "/users"),
            ("获取当前用户信息", "GET", "/user"),
            ("超级管理员创建带role的用户", "POST", "/user"),
            ("更新用户信息角色", "PATCH", "/user/role"),
            ("更新用户基础信息", "PATCH", "/user"),
            ("删除某个用户", "DELETE", "/user/{user_id}"),
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

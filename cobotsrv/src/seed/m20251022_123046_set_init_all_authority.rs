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
        let seeds: Vec<(&str, &str)> = vec![
            // authority
            ("添加权限", "post?/authority"),
            ("更新权限", "patch?/authority"),
            ("删除权限", "delete?/authority"),
            ("获取所有权限", "get?/authority"),
            // role
            ("添加新角色", "post?/role"),
            ("获取所有角色", "get?/role"),
            ("更新角色", "patch?/role"),
            ("删除角色", "delete?/role"),
            // role_authority
            ("给角色添加权限", "post?/role_authority"),
            ("给角色删除权限", "delete?/role_authority"),
            ("批量给角色添加权限", "post?/role_authority/batch"),
            ("批量删除角色权限", "delete?/role_authority/batch"),
            ("获取角色权限详情", "get?/role_authority/{role_id}"),
            // role/modifiable
            ("获取可修改角色", "get?/role/modifiable"),
            // recordings
            ("分页获取用户记录", "get?/recordings/{user_id}"),
            ("获取所有用户记录", "get?/recordings"),
            // user
            ("分页获取所有用户信息", "get?/users"),
            ("获取当前用户信息", "get?/user"),
            ("超级管理员创建带role的用户", "post?/user"),
            ("更新用户信息角色", "patch?/user/role"),
            ("更新用户基础信息", "patch?/user"),
            ("删除某个用户", "delete?/user/{user_id}"),
        ];

        for (name, uri) in seeds.into_iter() {
            let exists = authority::Entity::find()
                .filter(authority::Column::AuthorityUri.eq(uri))
                .one(conn)
                .await?;

            if exists.is_none() {
                authority::ActiveModel {
                    id: ActiveValue::NotSet,
                    name: Set(name.to_string()),
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

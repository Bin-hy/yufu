use crate::IdType;
use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::prelude::DateTime;
use sea_orm::{ActiveModelBehavior, DeriveEntityModel, Set};
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "refresh_token")]
pub struct Model {
    pub jti: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: IdType,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    // 插入 | 覆盖一条 refresh_token
    pub async fn upsert_refresh_token(
        db: &DatabaseConnection,
        jti: String,
        user_id: IdType,
    ) -> Result<(), DbErr> {
        let some_db_ = Self::find()
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await?;
        if let Some(db_rtc) = some_db_ {
            // 1. 修改
            let mut rt: ActiveModel = db_rtc.into();
            rt.jti = Set(jti.to_owned());
            rt.update_time = Set(Utc::now().naive_utc());
            rt.update(db).await?;
        } else {
            // 2. 插入
            ActiveModel {
                jti: Set(jti.to_owned()),
                user_id: Set(user_id),
                create_time: Set(Utc::now().naive_utc()),
                update_time: Set(Utc::now().naive_utc()),
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }

    // 查找 refresh_token 是否存在
    pub async fn find_exist(db: &DatabaseConnection, jti: String) -> Result<bool, DbErr> {
        if Self::find()
            .filter(Column::Jti.eq(jti))
            .one(db)
            .await?
            .is_some()
        {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // 删除刷新 用户退出登陆
    pub async fn delete_by_sub(db: &DatabaseConnection, sub: IdType) -> Result<(), DbErr> {
        // 删除 refresh_token 中的 sub主键的 记录
        Self::delete_by_id(sub).exec(db).await?;
        Ok(())
    }
}

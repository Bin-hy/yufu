use crate::IdType;
use crate::entity::{permission, user};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DeleteResult, EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_permission")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: IdType,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: IdType,
    /// `true` = explicit allow (overrides RBAC absence), `false` = explicit deny
    /// (overrides RBAC grant). Mirrors `user_authority.allow`.
    pub allow: bool,
    pub create_time: DateTime,
    pub update_time: DateTime,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: Option<user::Entity>,
    #[sea_orm(belongs_to, from = "permission_id", to = "id")]
    pub permission: Option<permission::Entity>,
}

impl Entity {
    /// All explicit overrides for this user — caller resolves precedence vs RBAC.
    pub async fn list_by_user(
        db: &DatabaseConnection,
        user_id: IdType,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .all(db)
            .await
    }

    pub async fn upsert_override(
        db: &DatabaseConnection,
        user_id: IdType,
        permission_id: IdType,
        allow: bool,
    ) -> Result<Model, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        match Self::find_pair(db, user_id, permission_id).await? {
            Some(existing) => {
                let mut am: ActiveModel = existing.into();
                am.allow = Set(allow);
                am.update_time = Set(now);
                am.update(db).await
            }
            None => {
                let am = ActiveModel {
                    user_id: Set(user_id),
                    permission_id: Set(permission_id),
                    allow: Set(allow),
                    create_time: Set(now),
                    update_time: Set(now),
                };
                am.insert(db).await
            }
        }
    }

    pub async fn delete_pair(
        db: &DatabaseConnection,
        user_id: IdType,
        permission_id: IdType,
    ) -> Result<DeleteResult, DbErr> {
        Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::PermissionId.eq(permission_id))
            .exec(db)
            .await
    }

    pub async fn find_pair(
        db: &DatabaseConnection,
        user_id: IdType,
        permission_id: IdType,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::PermissionId.eq(permission_id))
            .one(db)
            .await
    }

    pub async fn paginate(
        db: &DatabaseConnection,
        page_size: u64,
        page_number: u64,
    ) -> Result<(u64, Vec<Model>), DbErr> {
        use sea_orm::PaginatorTrait;
        let paginator = Self::find().paginate(db, page_size);
        let total_pages = paginator.num_pages().await?;
        let records = paginator.fetch_page(page_number - 1).await?;
        Ok((total_pages, records))
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserPermVO {
    pub user_id: IdType,
    pub permission_id: IdType,
    pub allow: bool,
    pub create_time: String,
    pub update_time: String,
    pub user_vo: Option<user::UserVO>,
    pub permission_vo: Option<permission::PermissionVO>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserPermAddReq {
    pub user_id: IdType,
    pub permission_id: IdType,
    pub allow: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserPermPatchReq {
    pub user_id: IdType,
    pub permission_id: IdType,
    pub allow: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserPermDeleteReq {
    pub user_id: IdType,
    pub permission_id: IdType,
}

use crate::IdType;
use crate::entity::{permission, role};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DeleteResult, EntityTrait, QueryFilter, Set, TransactionSession,
};
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "role_permission")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: IdType,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: IdType,
    pub create_time: DateTime,

    #[sea_orm(belongs_to, from = "role_id", to = "id")]
    pub role: Option<role::Entity>,
    #[sea_orm(belongs_to, from = "permission_id", to = "id")]
    pub permission: Option<permission::Entity>,
}

impl Entity {
    pub async fn list_permission_ids_by_role(
        db: &impl sea_orm::ConnectionTrait,
        role_id: IdType,
    ) -> Result<Vec<IdType>, DbErr> {
        Ok(Self::find()
            .filter(Column::RoleId.eq(role_id))
            .all(db)
            .await?
            .into_iter()
            .map(|m| m.permission_id)
            .collect())
    }

    pub async fn add_one(
        db: &impl sea_orm::ConnectionTrait,
        role_id: IdType,
        permission_id: IdType,
    ) -> Result<Model, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let am = ActiveModel {
            role_id: Set(role_id),
            permission_id: Set(permission_id),
            create_time: Set(now),
        };
        am.insert(db).await
    }

    pub async fn add_batch_tx(
        db: &(impl sea_orm::ConnectionTrait + sea_orm::TransactionTrait),
        role_id: IdType,
        permission_ids: Vec<IdType>,
    ) -> Result<Vec<Model>, DbErr> {
        let txn = db.begin().await?;
        let mut out = Vec::with_capacity(permission_ids.len());
        for pid in permission_ids {
            let now = chrono::Utc::now().naive_utc();
            let am = ActiveModel {
                role_id: Set(role_id),
                permission_id: Set(pid),
                create_time: Set(now),
            };
            let inserted = am.insert(&txn).await?;
            out.push(inserted);
        }
        txn.commit().await?;
        Ok(out)
    }

    pub async fn delete_pair(
        db: &impl sea_orm::ConnectionTrait,
        role_id: IdType,
        permission_id: IdType,
    ) -> Result<DeleteResult, DbErr> {
        Self::delete_many()
            .filter(Column::RoleId.eq(role_id))
            .filter(Column::PermissionId.eq(permission_id))
            .exec(db)
            .await
    }

    pub async fn delete_batch(
        db: &impl sea_orm::ConnectionTrait,
        role_id: IdType,
        permission_ids: Vec<IdType>,
    ) -> Result<DeleteResult, DbErr> {
        Self::delete_many()
            .filter(Column::RoleId.eq(role_id))
            .filter(Column::PermissionId.is_in(permission_ids))
            .exec(db)
            .await
    }

    pub async fn paginate_permissions_by_role(
        db: &impl sea_orm::ConnectionTrait,
        role_id: IdType,
        page_size: u64,
        page_number: u64,
    ) -> Result<(u64, Vec<(Model, Option<permission::Model>)>), DbErr> {
        use sea_orm::PaginatorTrait;
        let paginator = Self::find()
            .filter(Column::RoleId.eq(role_id))
            .find_also_related(permission::Entity)
            .paginate(db, page_size);
        let total_pages = paginator.num_pages().await?;
        let records = paginator.fetch_page(page_number - 1).await?;
        Ok((total_pages, records))
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Serialize, Deserialize)]
pub struct RolePermissionVO {
    pub role_id: IdType,
    pub permission_id: IdType,
    pub create_time: String,
    pub role: Option<role::RoleVO>,
    pub permission: Option<permission::PermissionVO>,
}

impl From<Model> for RolePermissionVO {
    fn from(value: Model) -> Self {
        Self {
            role_id: value.role_id,
            permission_id: value.permission_id,
            create_time: value.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            role: None,
            permission: None,
        }
    }
}

impl RolePermissionVO {
    pub fn set_role_permission(mut self, r: role::RoleVO, p: permission::PermissionVO) -> Self {
        self.role = Some(r);
        self.permission = Some(p);
        self
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RolePermAddReq {
    pub role_id: IdType,
    pub permission_id: IdType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RolePermBatchAddReq {
    pub role_id: IdType,
    pub permission_ids: Vec<IdType>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RolePermDeleteReq {
    pub role_id: IdType,
    pub permission_id: IdType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RolePermBatchDeleteReq {
    pub role_id: IdType,
    pub permission_ids: Vec<IdType>,
}

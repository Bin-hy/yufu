use crate::IdType;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, NotSet, PaginatorTrait, Set};
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "permission")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    pub application_id: IdType,
    /// Unique within an application — used as the canonical reference in tokens
    /// and configuration (e.g. `device:read`, `task:submit`).
    pub name: String,
    pub description: Option<String>,
    /// Verb / capability, e.g. "read", "submit".
    pub action: String,
    /// Structured resource kind, e.g. "device", "task". Legacy NULL rules
    /// require correction before use by the strict authorization adapter.
    pub resource_type: Option<String>,
    /// Serialized auth::security::policy::ResourceSelector. Missing values and
    /// legacy globs are never promoted to a wildcard by strict authorization.
    pub resource_pattern: Option<String>,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    pub async fn list_by_application(
        db: &impl sea_orm::ConnectionTrait,
        application_id: IdType,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::ApplicationId.eq(application_id))
            .all(db)
            .await
    }

    pub async fn get_by_app_and_name(
        db: &impl sea_orm::ConnectionTrait,
        application_id: IdType,
        name: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::ApplicationId.eq(application_id))
            .filter(Column::Name.eq(name))
            .one(db)
            .await
    }

    pub async fn insert_one(
        db: &impl sea_orm::ConnectionTrait,
        insert: PermissionInsertReq,
    ) -> Result<Model, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let am = ActiveModel {
            id: NotSet,
            application_id: Set(insert.application_id),
            name: Set(insert.name),
            description: Set(insert.description),
            action: Set(insert.action),
            resource_type: Set(insert.resource_type),
            resource_pattern: Set(insert.resource_pattern),
            create_time: Set(now),
            update_time: Set(now),
        };
        am.insert(db).await
    }

    pub async fn paginate(
        db: &impl sea_orm::ConnectionTrait,
        page_size: u64,
        page_number: u64,
    ) -> Result<(u64, Vec<Model>), DbErr> {
        let paginator = Self::find().paginate(db, page_size);
        let total_pages = paginator.num_pages().await?;
        let records = paginator.fetch_page(page_number - 1).await?;
        Ok((total_pages, records))
    }

    pub async fn update_partial(
        db: &impl sea_orm::ConnectionTrait,
        patch: PermissionPatchReq,
    ) -> Result<Model, DbErr> {
        let dbm = Self::find_by_id(patch.id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("permission".to_string()))?;
        let mut am: ActiveModel = dbm.into();
        am.name = patch.name.map(Set).unwrap_or(NotSet);
        am.description = Set(patch.description);
        am.action = patch.action.map(Set).unwrap_or(NotSet);
        am.resource_type = Set(patch.resource_type);
        am.resource_pattern = Set(patch.resource_pattern);
        am.application_id = patch.application_id.map(Set).unwrap_or(NotSet);
        am.update_time = Set(chrono::Utc::now().naive_utc());
        am.update(db).await
    }

    pub async fn delete_checked(
        db: &impl sea_orm::ConnectionTrait,
        id: IdType,
    ) -> Result<bool, DbErr> {
        let res = Self::delete_by_id(id).exec(db).await?;
        Ok(res.rows_affected == 1)
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Serialize, Deserialize)]
pub struct PermissionVO {
    pub id: IdType,
    pub application_id: IdType,
    pub name: String,
    pub description: Option<String>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_pattern: Option<String>,
    pub create_time: String,
    pub update_time: String,
}

impl From<Model> for PermissionVO {
    fn from(value: Model) -> Self {
        Self {
            id: value.id,
            application_id: value.application_id,
            name: value.name,
            description: value.description,
            action: value.action,
            resource_type: value.resource_type,
            resource_pattern: value.resource_pattern,
            create_time: value.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            update_time: value.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PermissionInsertReq {
    pub application_id: IdType,
    pub name: String,
    pub description: Option<String>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_pattern: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PermissionPatchReq {
    pub id: IdType,
    pub name: Option<String>,
    pub description: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_pattern: Option<String>,
    pub application_id: Option<IdType>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PermissionDeleteReq {
    pub id: IdType,
}

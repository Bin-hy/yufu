use crate::IdType;
use sea_orm::entity::prelude::*;
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
    /// Object class, e.g. "device", "task". None = action only (no object).
    pub resource_type: Option<String>,
    /// Glob pattern matched against object id, e.g. "cobotlet-0:*". None = match anything.
    pub resource_pattern: Option<String>,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    pub async fn list_by_application(
        db: &DatabaseConnection,
        application_id: IdType,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::ApplicationId.eq(application_id))
            .all(db)
            .await
    }

    pub async fn get_by_app_and_name(
        db: &DatabaseConnection,
        application_id: IdType,
        name: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::ApplicationId.eq(application_id))
            .filter(Column::Name.eq(name))
            .one(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

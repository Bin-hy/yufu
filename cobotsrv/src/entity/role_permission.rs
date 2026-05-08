use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "role_permission")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: IdType,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: IdType,
    pub create_time: DateTime,
}

impl Entity {
    pub async fn list_permission_ids_by_role(
        db: &DatabaseConnection,
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
}

impl ActiveModelBehavior for ActiveModel {}

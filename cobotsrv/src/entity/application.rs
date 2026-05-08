use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "application")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    /// Stable, human/admin-facing handle (e.g. `cobotrix`). Unique.
    #[sea_orm(unique)]
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    /// Optional link to the OIDC client that represents this application's
    /// browser/SDK identity. NULL for M2M-only or backend-only apps.
    pub oidc_client_id: Option<IdType>,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    pub async fn get_by_app_key(
        db: &DatabaseConnection,
        app_key: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::AppKey.eq(app_key))
            .one(db)
            .await
    }

    pub async fn get_by_oidc_client_id(
        db: &DatabaseConnection,
        oidc_client_id: IdType,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::OidcClientId.eq(oidc_client_id))
            .one(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth_token")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    pub user_id: IdType,
    pub client_id: String,
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: String,
    pub scope: Option<String>,
    pub token_type: String,
    pub create_time: DateTime,
    pub access_expires_at: DateTime,
    pub refresh_expires_at: DateTime,
    pub update_time: DateTime,
    pub revoked: bool,
}

impl Entity {
    pub async fn find_by_refresh_token(
        db: &DatabaseConnection,
        rt: &str,
    ) -> Result<Option<Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::RefreshToken.eq(rt))
            .filter(Column::Revoked.eq(false))
            .one(db)
            .await
    }

    pub async fn find_by_access_token(
        db: &DatabaseConnection,
        at: &str,
    ) -> Result<Option<Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::AccessToken.eq(at))
            .one(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

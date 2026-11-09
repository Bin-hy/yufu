use crate::IdType;
use sea_orm::DeleteResult;
use sea_orm::entity::prelude::*;
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth_consent")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub user_id: IdType,
    pub client_id: String, // references oauth_client.client_id
    pub scope: String,     // space-separated consented scopes
    pub create_time: DateTime,
    pub update_time: DateTime,
    pub environment_id: Option<Uuid>,
    pub config_version_id: Option<Uuid>,
}

impl Entity {
    pub async fn find_by_user_and_client<C: ConnectionTrait>(
        db: &C,
        user_id: IdType,
        client_id: &str,
    ) -> Result<Option<Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ClientId.eq(client_id))
            .one(db)
            .await
    }

    pub async fn find_all_by_user(
        db: &DatabaseConnection,
        user_id: IdType,
    ) -> Result<Vec<Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter, QueryOrder};
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreateTime)
            .all(db)
            .await
    }

    pub async fn delete_by_id_and_user(
        db: &DatabaseConnection,
        id: i32,
        user_id: IdType,
    ) -> Result<DeleteResult, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::delete_many()
            .filter(Column::Id.eq(id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

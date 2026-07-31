use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth_client")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    #[sea_orm(unique)]
    pub client_id: String,
    #[serde(skip_serializing)]
    pub user_id: IdType,
    pub name: String,
    pub redirect_uris: String,  // comma-separated
    pub allowed_scopes: String, // comma-separated
    #[serde(skip_serializing)]
    pub client_secret_hash: String, // bcrypt hash
    pub public_client: bool,
    pub grant_types: String, // comma-separated: authorization_code,refresh_token,client_credentials
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    pub async fn get_by_client_id(
        db: &DatabaseConnection,
        cid: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find().filter(Column::ClientId.eq(cid)).one(db).await
    }

    pub async fn list_by_user(
        db: &DatabaseConnection,
        user_id: IdType,
    ) -> Result<Vec<Model>, DbErr> {
        use sea_orm::QueryOrder;
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreateTime)
            .all(db)
            .await
    }

    pub async fn get_by_id_and_owner(
        db: &DatabaseConnection,
        id: i32,
        user_id: IdType,
    ) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

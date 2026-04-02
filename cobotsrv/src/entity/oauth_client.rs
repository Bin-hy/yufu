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
    pub name: String,
    pub redirect_uris: String,  // comma-separated
    pub allowed_scopes: String, // comma-separated
    #[serde(skip_serializing)]
    pub client_secret_hash: String, // bcrypt hash
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

    pub async fn get_by_id(db: &DatabaseConnection, id: i32) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(id).one(db).await
    }

    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<Model>, DbErr> {
        use sea_orm::QueryOrder;
        Self::find().order_by_desc(Column::CreateTime).all(db).await
    }
}

impl ActiveModelBehavior for ActiveModel {}

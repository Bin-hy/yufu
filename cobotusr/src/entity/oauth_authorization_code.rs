use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth_authorization_code")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    #[sea_orm(unique)]
    pub code: String,
    pub user_id: IdType,
    pub client_id: String,
    pub redirect_uri: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub used: bool,
    pub expires_at: DateTime,
    pub create_time: DateTime,
}

impl Entity {
    /// Find an unused authorization code by code value.
    pub async fn find_unused_by_code(
        db: &DatabaseConnection,
        code: &str,
    ) -> Result<Option<Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::Code.eq(code))
            .filter(Column::Used.eq(false))
            .one(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

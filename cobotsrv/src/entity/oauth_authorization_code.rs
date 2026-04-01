use crate::IdType;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, QueryFilter, Set};
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
    /// Take (consume) an authorization code: find it, mark as used, return it.
    /// Returns None if not found or already used.
    pub async fn take_code(
        db: &DatabaseConnection,
        code: &str,
    ) -> Result<Option<Model>, DbErr> {
        let rec = Self::find()
            .filter(Column::Code.eq(code))
            .filter(Column::Used.eq(false))
            .one(db)
            .await?;

        if let Some(rec) = rec {
            let mut am: ActiveModel = rec.clone().into();
            am.used = Set(true);
            am.update(db).await?;
            Ok(Some(rec))
        } else {
            Ok(None)
        }
    }

    /// Delete expired authorization codes.
    pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let res = Self::delete_many()
            .filter(Column::ExpiresAt.lt(now))
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }
}

impl ActiveModelBehavior for ActiveModel {}

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
    #[serde(skip_serializing)]
    pub code_digest: Option<Vec<u8>>,
    pub flow_id: Option<Uuid>,
    pub sso_session_id: Option<Uuid>,
    pub oidc_rp_session_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub config_version_id: Option<Uuid>,
    pub auth_time: Option<i64>,
    pub auth_epoch: Option<i64>,
    pub lifecycle_epoch: Option<i64>,
    pub access_generation: Option<i64>,
}

impl Entity {
    /// Find an unused authorization code by code value.
    pub async fn find_unused_by_code(
        db: &DatabaseConnection,
        code: &str,
    ) -> Result<Option<Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        use sha2::{Digest, Sha256};
        Self::find()
            .filter(
                sea_orm::Condition::any()
                    .add(Column::Code.eq(format!(
                        "sha256:{}",
                        hex::encode(Sha256::digest(code.as_bytes()))
                    )))
                    .add(
                        sea_orm::Condition::all()
                            .add(Column::Code.not_like("sha256:%"))
                            .add(Column::Code.eq(code)),
                    ),
            )
            .filter(Column::Used.eq(false))
            .one(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

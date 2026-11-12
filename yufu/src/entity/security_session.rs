use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "security_session")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub issuer: String,
    pub principal_kind: String,
    pub subject: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    pub sso_session_id: Option<Uuid>,
    pub oidc_rp_session_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub config_version_id: Option<Uuid>,
    pub auth_epoch: Option<i64>,
    pub lifecycle_epoch: Option<i64>,
    pub access_generation: Option<i64>,
    pub identity_scopes: Option<String>,
}
impl ActiveModelBehavior for ActiveModel {}

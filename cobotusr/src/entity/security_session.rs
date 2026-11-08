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
}
impl ActiveModelBehavior for ActiveModel {}

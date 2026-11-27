use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_session_client")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub sso_session_id: Uuid,
    pub kind: String,
    pub device_id: Option<String>,
    pub device_label: Option<String>,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

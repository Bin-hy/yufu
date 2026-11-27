use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_handoff_ticket")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub secret_digest: Vec<u8>,
    pub purpose: String,
    pub sso_session_id: Uuid,
    pub user_id: i32,
    pub auth_epoch: i64,
    pub oidc_flow_id: Option<Uuid>,
    pub application_id: Option<i32>,
    pub environment_id: Option<Uuid>,
    pub next_path: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_handoff_ticket::Model(REDACTED)")
    }
}

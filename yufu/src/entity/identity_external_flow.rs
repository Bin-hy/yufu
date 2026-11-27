use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_external_flow")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub provider_key: String,
    pub purpose: String,
    pub state_digest: Vec<u8>,
    pub browser_binding_digest: Vec<u8>,
    pub user_id: Option<i32>,
    pub oidc_flow_id: Option<Uuid>,
    pub nonce: Option<String>,
    pub secret_ciphertext: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_external_flow::Model(REDACTED)")
    }
}

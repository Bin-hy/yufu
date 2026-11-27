use sea_orm::entity::prelude::*;

/// Provider credentials are stored as AEAD ciphertext; this entity never exposes plaintext.
#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_external_provider")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub provider_key: String,
    pub family: String,
    pub enabled: bool,
    pub client_id: String,
    pub secret_ciphertext: Option<String>,
    pub secret_key_version: i32,
    pub config: Json,
    pub revision: i64,
    pub updated_by: Option<i32>,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_external_provider::Model(REDACTED)")
    }
}

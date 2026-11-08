use sea_orm::entity::prelude::*;

/// Persisted restricted permit, not a general user Bearer. Never serialize this
/// entity as a response. Its wire value is redacted even in generated ActiveModels.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "security_permit")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub issuer: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub jti: String,
    pub parent_jti: String,
    pub grant_id: Uuid,
    pub application: String,
    pub request_id: Uuid,
    pub task_id: Option<Uuid>,
    pub actor: Json,
    pub presenter: Json,
    pub operation: String,
    pub resource: Json,
    pub request_digest: Vec<u8>,
    pub capability_revision: Option<String>,
    pub target_binding_revision: Option<String>,
    pub signing_key_id: String,
    pub signed_permit: StoredPermitValue,
    pub checked_at: i64,
    pub valid_until: i64,
    pub issued_at: i64,
    pub expires_at: i64,
}
#[derive(Clone, PartialEq, Eq, DeriveValueType)]
pub struct StoredPermitValue(String);
impl StoredPermitValue {
    pub fn new(value: String) -> Result<Self, auth::security::error::SecurityError> {
        auth::security::message::SignedDocument::new(value.clone())?;
        Ok(Self(value))
    }
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for StoredPermitValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoredPermitValue([REDACTED])")
    }
}
impl ActiveModelBehavior for ActiveModel {}

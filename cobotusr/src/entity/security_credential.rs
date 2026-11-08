use sea_orm::entity::prelude::*;

/// Current credential registration, never the JWT/refresh value itself. Times are
/// UTC Unix milliseconds. Parse bounded types in the trusted repository adapter.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "security_credential")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub issuer: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub jti: String,
    pub principal_kind: String,
    pub subject: String,
    pub credential_kind: String,
    pub token_use: String,
    pub audience: String,
    pub application: String,
    pub bounds: Json,
    pub binding: Json,
    /// Stable authorization root; rotation keeps this identity.
    pub grant_id: Uuid,
    pub session_id: Option<Uuid>,
    pub family_id: Option<Uuid>,
    /// Only delegated operations depend on a still-active parent credential.
    pub revocation_parent_jti: Option<String>,
    /// Audit source only; consumption/revocation of this source does not revoke
    /// its replacements. This must not be treated as a revocation parent.
    pub minted_by: Option<Json>,
    pub signing_key_id: Option<String>,
    pub secret_digest: Option<Vec<u8>>,
    pub issued_at: i64,
    pub not_before: Option<i64>,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub consumed_at: Option<i64>,
    pub replacement_jti: Option<String>,
}
impl ActiveModelBehavior for ActiveModel {}

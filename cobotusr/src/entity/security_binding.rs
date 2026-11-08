//! Trusted identity registrations. These models are not request DTOs.
use sea_orm::entity::prelude::*;

/// One machine-client purpose, application and audience binding. Empty operations
/// never mean wildcard permission; the repository validates the typed JSON value.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "security_binding")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub issuer: String,
    pub client_id: String,
    pub token_use: String,
    pub application: String,
    pub audience: String,
    pub allowed_operations: Json,
    pub identity_binding: Json,
    pub enabled: bool,
}
impl ActiveModelBehavior for ActiveModel {}

pub mod device {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "security_device_binding")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub issuer: String,
        pub subject: String,
        pub application: String,
        pub gateway_id: String,
        pub device_id: String,
        pub binding_revision: String,
        pub enabled: bool,
    }
    impl ActiveModelBehavior for ActiveModel {}
}

pub mod gateway_key {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "security_gateway_signing_key")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub issuer: String,
        pub application: String,
        pub gateway_id: String,
        pub kid: String,
        /// Public material only. The private signing key stays on its gateway.
        pub public_key: String,
        /// SHA-256 of canonical public-key DER, computed by the trusted registrar.
        pub public_key_fingerprint: Vec<u8>,
        pub enabled: bool,
        pub valid_from: i64,
        pub valid_until: i64,
    }
    impl ActiveModelBehavior for ActiveModel {}
}

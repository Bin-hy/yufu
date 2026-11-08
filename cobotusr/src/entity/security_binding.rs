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

/// A browser client's access audience is server configuration, independent of
/// the client audience carried by its ID Token.
pub mod session_client {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "security_session_client")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub issuer: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub client_id: String,
        pub audience: String,
        pub application: String,
        pub enabled: bool,
    }
    impl ActiveModelBehavior for ActiveModel {}
}

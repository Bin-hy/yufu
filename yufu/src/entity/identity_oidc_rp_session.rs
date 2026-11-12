use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_oidc_rp_session")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sso_session_id: Uuid,
    pub issuer: String,
    pub environment_id: Uuid,
    pub config_version_id: Uuid,
    pub protocol_sid: Uuid,
    pub subject: String,
    pub auth_time: i64,
    pub nonce: Option<String>,
    pub oauth_client_id: i32,
    pub identity_scopes: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::identity_sso_session::Entity",
        from = "Column::SsoSessionId",
        to = "super::identity_sso_session::Column::Id"
    )]
    Root,
    #[sea_orm(
        belongs_to = "super::identity_app_environment::Entity",
        from = "Column::EnvironmentId",
        to = "super::identity_app_environment::Column::Id"
    )]
    Environment,
    #[sea_orm(
        belongs_to = "super::identity_app_config_version::Entity",
        from = "Column::ConfigVersionId",
        to = "super::identity_app_config_version::Column::Id"
    )]
    Version,
}

impl Related<super::identity_sso_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Root.def()
    }
}

impl Related<super::identity_app_environment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Environment.def()
    }
}

impl Related<super::identity_app_config_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Version.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_oidc_rp_session::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

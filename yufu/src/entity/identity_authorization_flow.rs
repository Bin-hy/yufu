use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_authorization_flow")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub request_payload: Json,
    pub environment_id: Uuid,
    pub config_version_id: Uuid,
    pub browser_proof_digest: Vec<u8>,
    pub browser_binding_digest: Vec<u8>,
    pub bound_sso_session_id: Option<Uuid>,
    pub created_at: i64,
    pub expires_at: i64,
    pub completed_at: Option<i64>,
    pub reauth_completed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
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
    #[sea_orm(
        belongs_to = "super::identity_sso_session::Entity",
        from = "Column::BoundSsoSessionId",
        to = "super::identity_sso_session::Column::Id"
    )]
    Root,
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

impl Related<super::identity_sso_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Root.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_authorization_flow::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

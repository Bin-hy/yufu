use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_app_environment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub application_id: i32,
    pub kind: String,
    pub oauth_client_id: i32,
    pub active_version_id: Option<Uuid>,
    pub lifecycle_epoch: i64,
    pub revision: i64,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::application::Entity",
        from = "Column::ApplicationId",
        to = "super::application::Column::Id"
    )]
    Application,
    #[sea_orm(
        belongs_to = "super::oauth_client::Entity",
        from = "Column::OauthClientId",
        to = "super::oauth_client::Column::Id"
    )]
    Client,
    #[sea_orm(
        belongs_to = "super::identity_app_config_version::Entity",
        from = "Column::ActiveVersionId",
        to = "super::identity_app_config_version::Column::Id"
    )]
    ActiveVersion,
}

impl Related<super::application::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Application.def()
    }
}

impl Related<super::oauth_client::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Client.def()
    }
}

impl Related<super::identity_app_config_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ActiveVersion.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_app_environment::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

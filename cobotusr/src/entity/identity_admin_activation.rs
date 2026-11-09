use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_admin_activation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub environment_id: Uuid,
    pub version_id: Uuid,
    pub actor_id: i32,
    pub snapshot: Json,
    pub logout_limitation_accepted: bool,
    pub created_at: i64,
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
        from = "Column::VersionId",
        to = "super::identity_app_config_version::Column::Id"
    )]
    Version,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::ActorId",
        to = "super::user::Column::Id"
    )]
    Actor,
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

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Actor.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_admin_activation::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

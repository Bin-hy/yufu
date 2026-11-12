use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_access_generation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub environment_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    pub generation: i64,
    pub eligible: bool,
    pub changed_at: i64,
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
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::identity_app_environment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Environment.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_access_generation::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_introspection_reader")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub reader_id: String,
    pub secret_hash: String,
    pub environment_id: Uuid,
    pub approved_audience: String,
    pub enabled: bool,
    pub secret_revision: i64,
    pub created_by: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::identity_app_environment::Entity",
        from = "Column::EnvironmentId",
        to = "super::identity_app_environment::Column::Id"
    )]
    Environment,
}

impl Related<super::identity_app_environment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Environment.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_introspection_reader::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

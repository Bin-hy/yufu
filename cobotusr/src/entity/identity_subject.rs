use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_subject")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub issuer: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub sub: String,
    pub user_id: Option<i32>,
    pub created_at: i64,
    pub tombstoned_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::identity_provider::Entity",
        from = "Column::Issuer",
        to = "super::identity_provider::Column::Issuer"
    )]
    Provider,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::identity_provider::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Provider.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_subject::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

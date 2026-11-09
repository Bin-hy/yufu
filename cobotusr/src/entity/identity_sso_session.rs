use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_sso_session")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub issuer: String,
    pub user_id: i32,
    pub subject: String,
    pub secret_digest: Vec<u8>,
    pub auth_epoch: i64,
    pub authenticated_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
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
        f.write_str("identity_sso_session::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_invitation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub secret_digest: Vec<u8>,
    pub purpose: String,
    pub target_user_id: Option<i32>,
    pub person_kind: String,
    pub developer_grant: bool,
    pub created_by: i32,
    pub created_at: i64,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
    pub accepted_user_id: Option<i32>,
    pub revoked_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::TargetUserId",
        to = "super::user::Column::Id"
    )]
    TargetUser,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::CreatedBy",
        to = "super::user::Column::Id"
    )]
    Creator,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AcceptedUserId",
        to = "super::user::Column::Id"
    )]
    AcceptedUser,
}

/// Explicit link for multiple foreign keys to the same target.
pub struct TargetUserLink;
impl Linked for TargetUserLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::TargetUser.def()]
    }
}

/// Explicit link for multiple foreign keys to the same target.
pub struct CreatorLink;
impl Linked for CreatorLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::Creator.def()]
    }
}

/// Explicit link for multiple foreign keys to the same target.
pub struct AcceptedUserLink;
impl Linked for AcceptedUserLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::AcceptedUser.def()]
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_invitation::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

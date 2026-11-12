use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_collaboration_candidate")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub requester_user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub candidate_user_id: i32,
    pub created_by: i32,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::RequesterUserId",
        to = "super::user::Column::Id"
    )]
    Requester,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::CandidateUserId",
        to = "super::user::Column::Id"
    )]
    Candidate,
}

/// Explicit link for multiple foreign keys to the same target.
pub struct RequesterLink;
impl Linked for RequesterLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::Requester.def()]
    }
}

/// Explicit link for multiple foreign keys to the same target.
pub struct CandidateLink;
impl Linked for CandidateLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::Candidate.def()]
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_collaboration_candidate::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_migration_conflict")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub oauth_client_id: Option<i32>,
    pub candidate_application_id: Option<i32>,
    pub candidate_owner_id: Option<i32>,
    pub conflict_kind: String,
    pub evidence_summary: Json,
    pub resolved_by: Option<i32>,
    pub resolution: Option<String>,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oauth_client::Entity",
        from = "Column::OauthClientId",
        to = "super::oauth_client::Column::Id"
    )]
    Client,
    #[sea_orm(
        belongs_to = "super::application::Entity",
        from = "Column::CandidateApplicationId",
        to = "super::application::Column::Id"
    )]
    Application,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::CandidateOwnerId",
        to = "super::user::Column::Id"
    )]
    Owner,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::ResolvedBy",
        to = "super::user::Column::Id"
    )]
    Resolver,
}

impl Related<super::oauth_client::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Client.def()
    }
}

impl Related<super::application::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Application.def()
    }
}

/// Explicit link for multiple foreign keys to the same target.
pub struct OwnerLink;
impl Linked for OwnerLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::Owner.def()]
    }
}

/// Explicit link for multiple foreign keys to the same target.
pub struct ResolverLink;
impl Linked for ResolverLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::Resolver.def()]
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_migration_conflict::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

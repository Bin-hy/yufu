use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_app_approval")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub environment_id: Uuid,
    pub version_id: Uuid,
    pub snapshot: Json,
    pub snapshot_hash: String,
    pub requested_by: i32,
    pub status: String,
    pub reviewed_by: Option<i32>,
    pub reason: Option<String>,
    pub created_at: i64,
    pub reviewed_at: Option<i64>,
    pub logout_limitation_accepted: bool,
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
        from = "Column::RequestedBy",
        to = "super::user::Column::Id"
    )]
    Requester,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::ReviewedBy",
        to = "super::user::Column::Id"
    )]
    Reviewer,
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
pub struct ReviewerLink;
impl Linked for ReviewerLink {
    type FromEntity = Entity;
    type ToEntity = super::user::Entity;
    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::Reviewer.def()]
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_app_approval::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

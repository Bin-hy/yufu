use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_audit_event")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub request_id: Uuid,
    pub actor_id: Option<i32>,
    pub action: String,
    pub object_kind: String,
    pub object_id: String,
    pub happened_at: i64,
    pub result: String,
    pub reason_code: Option<String>,
    pub change_summary: Json,
    pub operation_id: Option<Uuid>,
    pub commit_marker: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::ActorId",
        to = "super::user::Column::Id"
    )]
    Actor,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Actor.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_audit_event::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

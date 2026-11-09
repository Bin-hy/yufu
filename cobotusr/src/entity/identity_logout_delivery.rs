use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_logout_delivery")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub event_id: Uuid,
    pub rp_session_id: Uuid,
    pub client_id: String,
    pub endpoint_snapshot: String,
    pub status: String,
    pub attempt_count: i32,
    pub next_attempt_at: i64,
    pub lease_until: Option<i64>,
    pub lease_owner: Option<Uuid>,
    pub token_jti: Uuid,
    pub last_error_code: Option<String>,
    pub created_at: i64,
    pub delivered_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::identity_audit_event::Entity",
        from = "Column::EventId",
        to = "super::identity_audit_event::Column::Id"
    )]
    Event,
    #[sea_orm(
        belongs_to = "super::identity_oidc_rp_session::Entity",
        from = "Column::RpSessionId",
        to = "super::identity_oidc_rp_session::Column::Id"
    )]
    RpSession,
}

impl Related<super::identity_audit_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Event.def()
    }
}

impl Related<super::identity_oidc_rp_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RpSession.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_logout_delivery::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

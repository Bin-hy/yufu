use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "identity_action_flow")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub action_kind: String,
    pub invitation_id: Option<Uuid>,
    pub password_reset_id: Option<Uuid>,
    pub sso_session_id: Option<Uuid>,
    pub request_payload: Json,
    pub browser_proof_digest: Vec<u8>,
    pub browser_binding_digest: Vec<u8>,
    pub created_at: i64,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::identity_invitation::Entity",
        from = "Column::InvitationId",
        to = "super::identity_invitation::Column::Id"
    )]
    Invitation,
    #[sea_orm(
        belongs_to = "super::identity_password_reset::Entity",
        from = "Column::PasswordResetId",
        to = "super::identity_password_reset::Column::Id"
    )]
    PasswordReset,
    #[sea_orm(
        belongs_to = "super::identity_sso_session::Entity",
        from = "Column::SsoSessionId",
        to = "super::identity_sso_session::Column::Id"
    )]
    Root,
}

impl Related<super::identity_invitation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invitation.def()
    }
}

impl Related<super::identity_password_reset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PasswordReset.def()
    }
}

impl Related<super::identity_sso_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Root.def()
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_action_flow::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

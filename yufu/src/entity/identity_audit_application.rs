use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "identity_audit_application")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub event_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub application_id: i32,
    pub environment_id: Option<Uuid>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("identity_audit_application::Model(REDACTED)")
    }
}
impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

use crate::modules::im::model::enums::ConversationType;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "conversations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::conversation_member::Entity")]
    ConversationMember,
    #[sea_orm(has_many = "super::message::Entity")]
    Message,
    #[sea_orm(has_one = "super::group::Entity")]
    Group,
}

impl Related<super::conversation_member::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ConversationMember.def()
    }
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Message.def()
    }
}

impl Related<super::group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Group.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

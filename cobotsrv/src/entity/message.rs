use sea_orm::entity::prelude::*;
use serde_json::Value;

use crate::modules::im::model::enums::MessageType;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    pub conversation_id: i64,

    pub sender_id: i64,

    pub msg_type: MessageType,

    pub content: Option<String>,

    pub extra: Option<Value>,

    pub is_read: bool,

    pub is_recalled: bool,

    pub recalled_at: Option<DateTimeWithTimeZone>,

    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::conversation::Entity",
        from = "Column::ConversationId",
        to = "super::conversation::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Conversation,
}

impl Related<super::conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

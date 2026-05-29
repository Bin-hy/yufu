use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::entity::message;
use crate::modules::im::model::enums::MessageType;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageModel {
    pub id: i64,
    pub conversation_id: i64,
    pub sender_id: i64,
    pub msg_type: MessageType,
    pub content: Option<String>,
    pub extra: Option<Value>,
    pub is_recalled: bool,
    pub recalled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<crate::entity::message::Model> for MessageModel {
    fn from(value: crate::entity::message::Model) -> Self {
        Self {
            id: value.id,
            conversation_id: value.conversation_id,
            sender_id: value.sender_id,
            msg_type: value.msg_type,
            content: value.content,
            extra: value.extra,
            is_recalled: value.is_recalled,
            recalled_at: value.recalled_at.map(|v| v.with_timezone(&Utc)),
            created_at: value.created_at.with_timezone(&Utc),
        }
    }
}

pub async fn insert_message(
    db: &DatabaseConnection,
    conversation_id: i64,
    sender_id: i64,
    msg_type: MessageType,
    content: Option<String>,
    extra: Option<Value>,
) -> Result<message::Model, sea_orm::DbErr> {
    let msg = message::ActiveModel {
        conversation_id: Set(conversation_id),
        sender_id: Set(sender_id),
        msg_type: Set(msg_type),
        content: Set(content),
        extra: Set(extra),
        ..Default::default()
    };

    msg.insert(db).await
}

pub async fn find_message_by_id(
    db: &DatabaseConnection,
    message_id: i64,
) -> Result<Option<message::Model>, sea_orm::DbErr> {
    message::Entity::find_by_id(message_id).one(db).await
}

pub async fn find_message_by_client_msg_id(
    db: &DatabaseConnection,
    conversation_id: i64,
    sender_id: i64,
    client_msg_id: &str,
) -> Result<Option<message::Model>, sea_orm::DbErr> {
    let items = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::SenderId.eq(sender_id))
        .order_by_desc(message::Column::Id)
        .limit(500)
        .all(db)
        .await?;

    for item in items {
        let matched = item
            .extra
            .as_ref()
            .and_then(|extra| extra.get("client_msg_id"))
            .and_then(|v| v.as_str())
            .map(|v| v == client_msg_id)
            .unwrap_or(false);
        if matched {
            return Ok(Some(item));
        }
    }

    Ok(None)
}

pub async fn mark_message_recalled(
    db: &DatabaseConnection,
    model: message::Model,
    recalled_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<message::Model, sea_orm::DbErr> {
    let mut am: message::ActiveModel = model.into();
    am.is_recalled = Set(true);
    am.recalled_at = Set(Some(recalled_at));
    am.content = Set(None);
    am.extra = Set(None);
    am.update(db).await
}

pub async fn list_history_messages(
    db: &DatabaseConnection,
    conversation_id: i64,
    before_id: Option<i64>,
    limit: u64,
) -> Result<(Vec<message::Model>, bool), sea_orm::DbErr> {
    let fetch_limit = limit.saturating_add(1);
    let mut query =
        message::Entity::find().filter(message::Column::ConversationId.eq(conversation_id));

    if let Some(v) = before_id {
        query = query.filter(message::Column::Id.lt(v));
    }

    let mut items = query
        .order_by_desc(message::Column::Id)
        .limit(fetch_limit)
        .all(db)
        .await?;

    let has_more = items.len() as u64 > limit;
    if has_more {
        items.truncate(limit as usize);
    }
    items.reverse();

    Ok((items, has_more))
}

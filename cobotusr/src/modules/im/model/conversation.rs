use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use sea_orm::sea_query::{NullOrdering, OnConflict, Order};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use crate::entity::{conversation, conversation_member, message, user_conversation};
use crate::modules::im::model::enums::{ConversationType, MessageType};

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationModel {
    pub id: i64,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ConversationListItem {
    pub conversation_id: i64,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub unread_count: i32,
    pub is_pinned: bool,
    pub last_message_preview: Option<String>,
}

fn now_fixed() -> chrono::DateTime<chrono::FixedOffset> {
    Utc::now().fixed_offset()
}

pub const CONVERSATION_MEMBER_ROLE_MEMBER: i16 = 0;
pub const CONVERSATION_MEMBER_ROLE_OWNER: i16 = 1;

pub async fn ensure_private_conversation(
    db: &DatabaseConnection,
    conversation_id: i64,
) -> Result<(), sea_orm::DbErr> {
    if conversation::Entity::find_by_id(conversation_id)
        .one(db)
        .await?
        .is_some()
    {
        return Ok(());
    }

    let model = conversation::ActiveModel {
        id: Set(conversation_id),
        conversation_type: Set(ConversationType::Single),
        ..Default::default()
    };

    let _ = conversation::Entity::insert(model)
        .on_conflict(
            OnConflict::column(conversation::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}

pub async fn create_conversation(
    db: &DatabaseConnection,
    conversation_type: ConversationType,
    title: Option<String>,
) -> Result<conversation::Model, sea_orm::DbErr> {
    conversation::ActiveModel {
        conversation_type: Set(conversation_type),
        title: Set(title),
        ..Default::default()
    }
    .insert(db)
    .await
}

pub async fn ensure_user_conversations(
    db: &DatabaseConnection,
    conversation_id: i64,
    user_ids: &[i64],
    unread_count: i32,
    last_read_message_id: Option<i64>,
) -> Result<(), sea_orm::DbErr> {
    let now = now_fixed();
    for user_id in user_ids {
        user_conversation::Entity::insert(user_conversation::ActiveModel {
            user_id: Set(*user_id),
            conversation_id: Set(conversation_id),
            last_read_message_id: Set(last_read_message_id),
            unread_count: Set(unread_count),
            pinned_at: Set(None),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                user_conversation::Column::UserId,
                user_conversation::Column::ConversationId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(db)
        .await?;
    }

    Ok(())
}

pub async fn ensure_conversation_members(
    db: &DatabaseConnection,
    conversation_id: i64,
    user_ids: &[i64],
    role: i16,
) -> Result<(), sea_orm::DbErr> {
    for user_id in user_ids {
        conversation_member::Entity::insert(conversation_member::ActiveModel {
            conversation_id: Set(conversation_id),
            user_id: Set(*user_id),
            role: Set(role),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                conversation_member::Column::ConversationId,
                conversation_member::Column::UserId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(db)
        .await?;
    }

    Ok(())
}

pub async fn set_conversation_member_role(
    db: &DatabaseConnection,
    conversation_id: i64,
    user_id: i64,
    role: i16,
) -> Result<(), sea_orm::DbErr> {
    let existing = conversation_member::Entity::find()
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .filter(conversation_member::Column::UserId.eq(user_id))
        .one(db)
        .await?;

    match existing {
        Some(row) => {
            let mut am: conversation_member::ActiveModel = row.into();
            am.role = Set(role);
            am.update(db).await?;
        }
        None => {
            conversation_member::ActiveModel {
                conversation_id: Set(conversation_id),
                user_id: Set(user_id),
                role: Set(role),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }

    Ok(())
}

pub async fn is_user_in_conversation(
    db: &DatabaseConnection,
    user_id: i64,
    conversation_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    let found = conversation_member::Entity::find()
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .filter(conversation_member::Column::UserId.eq(user_id))
        .one(db)
        .await?;
    Ok(found.is_some())
}

pub async fn list_member_ids(
    db: &DatabaseConnection,
    conversation_id: i64,
) -> Result<Vec<i64>, sea_orm::DbErr> {
    let rows = conversation_member::Entity::find()
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|row| row.user_id).collect())
}

pub async fn remove_user_from_conversation(
    db: &DatabaseConnection,
    conversation_id: i64,
    user_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    let res = conversation_member::Entity::delete_many()
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .filter(conversation_member::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    let _ = user_conversation::Entity::delete_by_id((user_id, conversation_id))
        .exec(db)
        .await?;
    Ok(res.rows_affected > 0)
}

pub async fn touch_group_conversation_on_send(
    db: &DatabaseConnection,
    conversation_id: i64,
    sender_id: i64,
    message_id: i64,
) -> Result<Vec<i64>, sea_orm::DbErr> {
    let now = now_fixed();
    let member_ids = list_member_ids(db, conversation_id).await?;

    for member_id in &member_ids {
        match user_conversation::Entity::find_by_id((*member_id, conversation_id))
            .one(db)
            .await?
        {
            Some(row) => {
                let unread = row.unread_count;
                let mut am: user_conversation::ActiveModel = row.into();
                if *member_id == sender_id {
                    am.last_read_message_id = Set(Some(message_id));
                    am.unread_count = Set(0);
                } else {
                    am.unread_count = Set(unread.saturating_add(1));
                }
                am.updated_at = Set(now);
                am.update(db).await?;
            }
            None => {
                user_conversation::ActiveModel {
                    user_id: Set(*member_id),
                    conversation_id: Set(conversation_id),
                    last_read_message_id: Set((*member_id == sender_id).then_some(message_id)),
                    unread_count: Set(if *member_id == sender_id { 0 } else { 1 }),
                    pinned_at: Set(None),
                    updated_at: Set(now),
                }
                .insert(db)
                .await?;
            }
        }
    }

    Ok(member_ids)
}

pub async fn find_conversation(
    db: &DatabaseConnection,
    conversation_id: i64,
) -> Result<Option<conversation::Model>, sea_orm::DbErr> {
    conversation::Entity::find_by_id(conversation_id)
        .one(db)
        .await
}

pub async fn touch_private_conversations_on_send(
    db: &DatabaseConnection,
    conversation_id: i64,
    sender_id: i64,
    receiver_id: i64,
    message_id: i64,
) -> Result<(), sea_orm::DbErr> {
    let now = now_fixed();

    match user_conversation::Entity::find_by_id((sender_id, conversation_id))
        .one(db)
        .await?
    {
        Some(row) => {
            let mut am: user_conversation::ActiveModel = row.into();
            am.last_read_message_id = Set(Some(message_id));
            am.unread_count = Set(0);
            am.updated_at = Set(now);
            am.update(db).await?;
        }
        None => {
            user_conversation::ActiveModel {
                user_id: Set(sender_id),
                conversation_id: Set(conversation_id),
                last_read_message_id: Set(Some(message_id)),
                unread_count: Set(0),
                pinned_at: Set(None),
                updated_at: Set(now),
            }
            .insert(db)
            .await?;
        }
    }

    match user_conversation::Entity::find_by_id((receiver_id, conversation_id))
        .one(db)
        .await?
    {
        Some(row) => {
            let unread = row.unread_count;
            let mut am: user_conversation::ActiveModel = row.into();
            am.unread_count = Set(unread.saturating_add(1));
            am.updated_at = Set(now);
            am.update(db).await?;
        }
        None => {
            user_conversation::ActiveModel {
                user_id: Set(receiver_id),
                conversation_id: Set(conversation_id),
                last_read_message_id: Set(None),
                unread_count: Set(1),
                pinned_at: Set(None),
                updated_at: Set(now),
            }
            .insert(db)
            .await?;
        }
    }

    Ok(())
}

pub async fn list_user_conversations(
    db: &DatabaseConnection,
    user_id: i64,
    limit: u64,
) -> Result<Vec<ConversationListItem>, sea_orm::DbErr> {
    let rows = user_conversation::Entity::find()
        .filter(user_conversation::Column::UserId.eq(user_id))
        .order_by_with_nulls(
            user_conversation::Column::PinnedAt,
            Order::Desc,
            NullOrdering::Last,
        )
        .order_by_desc(user_conversation::Column::UpdatedAt)
        .limit(limit)
        .all(db)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        if !is_user_in_conversation(db, user_id, row.conversation_id).await? {
            continue;
        }

        let Some(conv) = conversation::Entity::find_by_id(row.conversation_id)
            .one(db)
            .await?
        else {
            continue;
        };

        let last = message::Entity::find()
            .filter(message::Column::ConversationId.eq(row.conversation_id))
            .order_by_desc(message::Column::Id)
            .one(db)
            .await?;

        let preview = last.map(|m| {
            if m.msg_type == MessageType::Text {
                m.content.unwrap_or_default()
            } else {
                format!("[msg_type:{}]", i16::from(m.msg_type))
            }
        });

        items.push(ConversationListItem {
            conversation_id: row.conversation_id,
            conversation_type: conv.conversation_type,
            title: conv.title,
            unread_count: row.unread_count,
            is_pinned: row.is_pinned(),
            last_message_preview: preview,
        });
    }

    Ok(items)
}

pub async fn clear_unread(
    db: &DatabaseConnection,
    user_id: i64,
    conversation_id: i64,
) -> Result<(Option<i64>, i32), sea_orm::DbErr> {
    let now = now_fixed();
    let last_msg_id = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .order_by_desc(message::Column::Id)
        .one(db)
        .await?
        .map(|m| m.id);

    match user_conversation::Entity::find_by_id((user_id, conversation_id))
        .one(db)
        .await?
    {
        Some(row) => {
            let mut am: user_conversation::ActiveModel = row.into();
            am.unread_count = Set(0);
            am.last_read_message_id = Set(last_msg_id);
            am.updated_at = Set(now);
            am.update(db).await?;
        }
        None => {
            user_conversation::ActiveModel {
                user_id: Set(user_id),
                conversation_id: Set(conversation_id),
                last_read_message_id: Set(last_msg_id),
                unread_count: Set(0),
                pinned_at: Set(None),
                updated_at: Set(now),
            }
            .insert(db)
            .await?;
        }
    }

    Ok((last_msg_id, 0))
}

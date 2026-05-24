use axum::extract::ws::Message;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::Value;

use crate::modules::im::model::{
    enums::MessageType,
    ws_message::{
        EVENT_CHAT_PUSH, EVENT_CONVERSATION_READ_PUSH, EVENT_GROUP_CHAT_PUSH,
        EVENT_MESSAGE_DELETE_PUSH, EVENT_MESSAGE_RECALL_PUSH, WsChatPushPayload,
        WsConversationReadPushPayload, WsGroupChatPushPayload, WsMessageDeletePushPayload,
        WsMessageRecallPushPayload, WsServerEnvelope,
    },
};

use super::{outbox_service, websocket_service::SharedConnectionManager};

pub struct PrivateChatPush {
    pub message_id: i64,
    pub conversation_id: i64,
    pub from: i64,
    pub to: i64,
    pub msg_type: MessageType,
    pub content: Option<String>,
    pub extra: Option<Value>,
    pub created_at: DateTime<Utc>,
}

pub struct GroupChatPush {
    pub message_id: i64,
    pub conversation_id: i64,
    pub from: i64,
    pub targets: Vec<i64>,
    pub msg_type: MessageType,
    pub content: Option<String>,
    pub extra: Option<Value>,
    pub created_at: DateTime<Utc>,
}

async fn send_envelope_to_users<T>(
    manager: &SharedConnectionManager,
    user_ids: Vec<i64>,
    envelope: &WsServerEnvelope<T>,
) where
    T: Serialize,
{
    if let Ok(text) = serde_json::to_string(envelope) {
        manager
            .send_to_users(user_ids, Message::Text(text.into()))
            .await;
    }
}

async fn dispatch_message_envelope<T>(
    db: &DatabaseConnection,
    manager: &SharedConnectionManager,
    message_id: i64,
    conversation_id: i64,
    targets: Vec<i64>,
    envelope: WsServerEnvelope<T>,
) where
    T: Serialize,
{
    let mut delivered_with_outbox = false;
    if let Ok(envelope_value) = serde_json::to_value(&envelope) {
        let enqueue_result = outbox_service::enqueue_message_push(
            db,
            message_id,
            conversation_id,
            targets.clone(),
            envelope_value,
        )
        .await;

        if enqueue_result.is_ok() {
            let _ = outbox_service::dispatch_due(db, manager.clone(), 100).await;
            delivered_with_outbox = true;
        }
    }

    if !delivered_with_outbox {
        send_envelope_to_users(manager, targets, &envelope).await;
    }
}

pub async fn push_private_chat(
    db: &DatabaseConnection,
    manager: &SharedConnectionManager,
    push: PrivateChatPush,
) {
    let PrivateChatPush {
        message_id,
        conversation_id,
        from,
        to,
        msg_type,
        content,
        extra,
        created_at,
    } = push;

    let envelope = WsServerEnvelope::ok(
        EVENT_CHAT_PUSH,
        WsChatPushPayload {
            message_id,
            from,
            to,
            msg_type,
            conversation_id,
            content,
            extra,
            created_at,
        },
        None,
    );

    dispatch_message_envelope(db, manager, message_id, conversation_id, vec![to], envelope).await;
}

pub async fn push_group_chat(
    db: &DatabaseConnection,
    manager: &SharedConnectionManager,
    push: GroupChatPush,
) {
    let GroupChatPush {
        message_id,
        conversation_id,
        from,
        targets,
        msg_type,
        content,
        extra,
        created_at,
    } = push;

    let envelope = WsServerEnvelope::ok(
        EVENT_GROUP_CHAT_PUSH,
        WsGroupChatPushPayload {
            message_id,
            conversation_id,
            from,
            msg_type,
            content,
            extra,
            created_at,
        },
        None,
    );

    dispatch_message_envelope(db, manager, message_id, conversation_id, targets, envelope).await;
}

pub async fn push_message_recall(
    manager: &SharedConnectionManager,
    targets: Vec<i64>,
    message_id: i64,
    conversation_id: i64,
    operator_id: i64,
    recalled_at: DateTime<Utc>,
) {
    let envelope = WsServerEnvelope::ok(
        EVENT_MESSAGE_RECALL_PUSH,
        WsMessageRecallPushPayload {
            message_id,
            conversation_id,
            operator_id,
            recalled_at,
        },
        None,
    );

    send_envelope_to_users(manager, targets, &envelope).await;
}

pub async fn push_message_delete(
    manager: &SharedConnectionManager,
    targets: Vec<i64>,
    message_id: i64,
    conversation_id: i64,
    operator_id: i64,
    deleted_at: DateTime<Utc>,
) {
    let envelope = WsServerEnvelope::ok(
        EVENT_MESSAGE_DELETE_PUSH,
        WsMessageDeletePushPayload {
            message_id,
            conversation_id,
            operator_id,
            deleted_at,
        },
        None,
    );

    send_envelope_to_users(manager, targets, &envelope).await;
}

pub async fn push_conversation_read(
    manager: &SharedConnectionManager,
    targets: Vec<i64>,
    conversation_id: i64,
    reader_id: i64,
    last_read_message_id: Option<i64>,
    read_at: DateTime<Utc>,
) {
    let envelope = WsServerEnvelope::ok(
        EVENT_CONVERSATION_READ_PUSH,
        WsConversationReadPushPayload {
            conversation_id,
            reader_id,
            last_read_message_id,
            read_at,
        },
        None,
    );

    send_envelope_to_users(manager, targets, &envelope).await;
}

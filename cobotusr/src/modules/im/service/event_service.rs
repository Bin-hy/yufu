use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::modules::im::model::enums::MessageType;

use super::{
    push_service::{self, GroupChatPush, PrivateChatPush},
    websocket_service::SharedConnectionManager,
};

pub type ImEventSender = UnboundedSender<ImEvent>;
pub type ImEvents = ImEventSender;

#[derive(Debug, Clone)]
pub enum ImEvent {
    PrivateChat {
        message_id: i64,
        conversation_id: i64,
        from: i64,
        to: i64,
        msg_type: MessageType,
        content: Option<String>,
        extra: Option<Value>,
        created_at: DateTime<Utc>,
    },
    GroupChat {
        message_id: i64,
        conversation_id: i64,
        from: i64,
        targets: Vec<i64>,
        msg_type: MessageType,
        content: Option<String>,
        extra: Option<Value>,
        created_at: DateTime<Utc>,
    },
    MessageRecall {
        targets: Vec<i64>,
        message_id: i64,
        conversation_id: i64,
        operator_id: i64,
        recalled_at: DateTime<Utc>,
    },
    MessageDelete {
        targets: Vec<i64>,
        message_id: i64,
        conversation_id: i64,
        operator_id: i64,
        deleted_at: DateTime<Utc>,
    },
    ConversationRead {
        targets: Vec<i64>,
        conversation_id: i64,
        reader_id: i64,
        last_read_message_id: Option<i64>,
        read_at: DateTime<Utc>,
    },
}

pub fn channel() -> (ImEventSender, UnboundedReceiver<ImEvent>) {
    unbounded_channel()
}

pub fn spawn_event_service(
    db: DatabaseConnection,
    manager: SharedConnectionManager,
    mut receiver: UnboundedReceiver<ImEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = receiver.recv().await {
            handle_event(&db, &manager, event).await;
        }
    })
}

async fn handle_event(db: &DatabaseConnection, manager: &SharedConnectionManager, event: ImEvent) {
    match event {
        ImEvent::PrivateChat {
            message_id,
            conversation_id,
            from,
            to,
            msg_type,
            content,
            extra,
            created_at,
        } => {
            push_service::push_private_chat(
                db,
                manager,
                PrivateChatPush {
                    message_id,
                    conversation_id,
                    from,
                    to,
                    msg_type,
                    content,
                    extra,
                    created_at,
                },
            )
            .await;
        }
        ImEvent::GroupChat {
            message_id,
            conversation_id,
            from,
            targets,
            msg_type,
            content,
            extra,
            created_at,
        } => {
            push_service::push_group_chat(
                db,
                manager,
                GroupChatPush {
                    message_id,
                    conversation_id,
                    from,
                    targets,
                    msg_type,
                    content,
                    extra,
                    created_at,
                },
            )
            .await;
        }
        ImEvent::MessageRecall {
            targets,
            message_id,
            conversation_id,
            operator_id,
            recalled_at,
        } => {
            push_service::push_message_recall(
                manager,
                targets,
                message_id,
                conversation_id,
                operator_id,
                recalled_at,
            )
            .await;
        }
        ImEvent::MessageDelete {
            targets,
            message_id,
            conversation_id,
            operator_id,
            deleted_at,
        } => {
            push_service::push_message_delete(
                manager,
                targets,
                message_id,
                conversation_id,
                operator_id,
                deleted_at,
            )
            .await;
        }
        ImEvent::ConversationRead {
            targets,
            conversation_id,
            reader_id,
            last_read_message_id,
            read_at,
        } => {
            push_service::push_conversation_read(
                manager,
                targets,
                conversation_id,
                reader_id,
                last_read_message_id,
                read_at,
            )
            .await;
        }
    }
}

use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::modules::im::model::enums::MessageType;

pub const EVENT_CHAT_PUSH: &str = "chat.push";
pub const EVENT_GROUP_CHAT_PUSH: &str = "group.push";
pub const EVENT_MESSAGE_RECALL_PUSH: &str = "message.recall.push";
pub const EVENT_MESSAGE_DELETE_PUSH: &str = "message.delete.push";
pub const EVENT_CONVERSATION_READ_PUSH: &str = "conversation.read.push";

pub const EVENT_HEARTBEAT_PING: &str = "heartbeat.ping";
pub const EVENT_HEARTBEAT_PONG: &str = "heartbeat.pong";

#[derive(Debug, Serialize, Deserialize)]
pub struct WsClientEnvelope {
    pub event: String,
    pub seq: Option<String>,
    pub data: Option<Value>,
}

impl WsClientEnvelope {
    pub fn decode_data<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.data.clone().unwrap_or(Value::Null))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsServerEnvelope<T>
where
    T: Serialize,
{
    pub event: String,
    pub status: u16,
    pub message: String,
    pub data: Option<T>,
    pub seq: Option<String>,
    pub server_ts: i64,
}

impl<T> WsServerEnvelope<T>
where
    T: Serialize,
{
    pub fn ok(event: impl Into<String>, data: T, seq: Option<String>) -> Self {
        Self {
            event: event.into(),
            status: StatusCode::OK.as_u16(),
            message: "ok".to_string(),
            data: Some(data),
            seq,
            server_ts: Utc::now().timestamp_millis(),
        }
    }

    pub fn err(event: impl Into<String>, status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            status: status.as_u16(),
            message: message.into(),
            data: None,
            seq: None,
            server_ts: Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsChatPushPayload {
    pub message_id: i64,
    pub from: i64,
    pub to: i64,
    pub msg_type: MessageType,
    pub conversation_id: i64,
    pub content: Option<String>,
    pub extra: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsGroupChatPushPayload {
    pub message_id: i64,
    pub conversation_id: i64,
    pub from: i64,
    pub msg_type: MessageType,
    pub content: Option<String>,
    pub extra: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsHeartbeatPayload {
    pub ts: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsMessageRecallPushPayload {
    pub message_id: i64,
    pub conversation_id: i64,
    pub operator_id: i64,
    pub recalled_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsMessageDeletePushPayload {
    pub message_id: i64,
    pub conversation_id: i64,
    pub operator_id: i64,
    pub deleted_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsConversationReadPushPayload {
    pub conversation_id: i64,
    pub reader_id: i64,
    pub last_read_message_id: Option<i64>,
    pub read_at: DateTime<Utc>,
}

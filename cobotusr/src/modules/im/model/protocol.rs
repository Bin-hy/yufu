use axum::{Json, response::IntoResponse};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::modules::im::model::enums::{ConversationType, MessageType};

pub fn parse_message_type(v: i16) -> Result<MessageType, String> {
    MessageType::from_i16(v)
}

pub fn parse_conversation_type(v: i16) -> Result<ConversationType, String> {
    ConversationType::from_i16(v)
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    #[serde(skip)]
    pub status: StatusCode,
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn ok(data: T) -> Self {
        Self {
            status: StatusCode::OK,
            message: "ok".to_string(),
            data: Some(data),
        }
    }

    pub fn err(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            data: None,
        }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        let status = self.status;
        let body = Json(self);
        (status, body).into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct SendConversationMessageRequest {
    pub msg_type: i16,
    pub content: Option<String>,
    pub extra: Option<Value>,
    pub client_msg_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendConversationMessageResponse {
    pub message_id: i64,
    pub conversation_id: i64,
    pub msg_type: MessageType,
    pub client_msg_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RecallMessageRequest {
    pub message_id: i64,
}

#[derive(Debug, Serialize)]
pub struct RecallMessageResponse {
    pub message_id: i64,
    pub conversation_id: i64,
    pub recalled_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteMessageRequest {
    pub message_id: i64,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessageResponse {
    pub message_id: i64,
    pub conversation_id: i64,
    pub deleted_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GetConversationMessagesQuery {
    pub before_id: Option<i64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct GetMessagesResponse {
    pub conversation_id: i64,
    pub before_id: Option<i64>,
    pub limit: u64,
    pub has_more: bool,
    pub items: Vec<crate::modules::im::model::message::MessageModel>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    pub conversation_type: i16,
    pub title: Option<String>,
    pub member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateConversationResponse {
    pub conversation_id: i64,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub member_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ConversationListQuery {
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ConversationSummary {
    pub conversation_id: i64,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub is_pinned: bool,
    pub unread_count: i32,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConversationListResponse {
    pub limit: u64,
    pub items: Vec<ConversationSummary>,
}

#[derive(Debug, Serialize)]
pub struct ConversationDetailResponse {
    pub conversation_id: i64,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct DeleteConversationResponse {
    pub conversation_id: i64,
    pub deleted: bool,
}

#[derive(Debug, Serialize)]
pub struct ConversationMembersResponse {
    pub conversation_id: i64,
    pub member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct ClearUnreadResponse {
    pub conversation_id: i64,
    pub unread_count: i32,
    pub last_read_message_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct UploadFileResponse {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct DispatchOutboxRequest {
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct DispatchOutboxResponse {
    pub limit: u64,
    pub sent_count: u64,
}

#[derive(Debug, Serialize)]
pub struct AddGroupMembersResponse {
    pub conversation_id: i64,
    pub added_member_ids: Vec<i64>,
    pub skipped_member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct RemoveGroupMemberResponse {
    pub conversation_id: i64,
    pub user_id: i64,
    pub removed: bool,
}

#[derive(Debug, Serialize)]
pub struct LeaveGroupResponse {
    pub conversation_id: i64,
    pub user_id: i64,
}

#[derive(Debug, Serialize)]
pub struct TransferGroupOwnerResponse {
    pub conversation_id: i64,
    pub old_owner_id: i64,
    pub new_owner_id: i64,
}

fn normalize_optional_content(content: Option<String>) -> Option<String> {
    content.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn ensure_storage_path(extra: &Value) -> Result<(), String> {
    let Some(storage_path) = extra
        .get("storage_path")
        .and_then(|v| v.as_str())
        .map(|v| v.trim())
    else {
        return Err("extra.storage_path is required for media/file message".to_string());
    };
    if storage_path.is_empty() {
        return Err("extra.storage_path cannot be empty".to_string());
    }
    Ok(())
}

pub fn normalize_message_payload(
    msg_type: MessageType,
    content: Option<String>,
    extra: Option<Value>,
) -> Result<(Option<String>, Option<Value>), String> {
    match msg_type {
        MessageType::Text => {
            let content = normalize_optional_content(content)
                .ok_or_else(|| "content is required for text message".to_string())?;
            Ok((Some(content), None))
        }
        MessageType::Image | MessageType::Audio | MessageType::Video | MessageType::File => {
            let extra =
                extra.ok_or_else(|| "extra is required for media/file message".to_string())?;
            ensure_storage_path(&extra)?;
            Ok((normalize_optional_content(content), Some(extra)))
        }
        MessageType::System => {
            let content = normalize_optional_content(content);
            if content.is_none() && extra.is_none() {
                return Err("system message requires content or extra".to_string());
            }
            Ok((content, extra))
        }
    }
}

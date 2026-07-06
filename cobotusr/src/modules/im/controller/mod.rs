pub mod conversation_controller;
pub mod group_controller;
pub mod message_controller;
pub mod outbox_controller;
pub mod upload_controller;
pub mod user_controller;
pub mod websocket_controller;

use http::StatusCode;
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait};
use serde::Serialize;

use crate::{
    entity::{
        conversation as conversation_entity, group as group_entity, message as message_entity,
    },
    modules::im::model::{
        conversation, enums::ConversationType, group as group_model, protocol::ApiResponse,
    },
};

pub async fn join_response<T>(handle: tokio::task::JoinHandle<ApiResponse<T>>) -> ApiResponse<T>
where
    T: Serialize,
{
    match handle.await {
        Ok(response) => response,
        Err(e) => internal_error(format!("request task failed: {e}")),
    }
}

pub fn api_error<T>(status: StatusCode, message: impl Into<String>) -> ApiResponse<T>
where
    T: Serialize,
{
    ApiResponse::err(status, message)
}

pub fn bad_request<T>(message: impl Into<String>) -> ApiResponse<T>
where
    T: Serialize,
{
    api_error(StatusCode::BAD_REQUEST, message)
}

pub fn internal_error<T>(message: impl Into<String>) -> ApiResponse<T>
where
    T: Serialize,
{
    api_error(StatusCode::INTERNAL_SERVER_ERROR, message)
}

pub fn normalize_limit(limit: Option<u64>, default: u64, max: u64) -> u64 {
    limit.unwrap_or(default).clamp(1, max)
}

pub async fn load_member_conversation<T>(
    db: &DatabaseConnection,
    user_id: i64,
    conversation_id: i64,
) -> Result<conversation_entity::Model, ApiResponse<T>>
where
    T: Serialize,
{
    if conversation_id <= 0 {
        return Err(bad_request("conversation_id must be positive"));
    }

    let is_member = conversation::is_user_in_conversation(db, user_id, conversation_id)
        .await
        .map_err(|e| internal_error(format!("query member failed: {e}")))?;
    if !is_member {
        return Err(bad_request("conversation not found or no access"));
    }

    conversation::find_conversation(db, conversation_id)
        .await
        .map_err(|e| internal_error(format!("query conversation failed: {e}")))?
        .ok_or_else(|| bad_request("conversation not found"))
}

pub async fn load_group<T>(
    db: &DatabaseConnection,
    conversation_id: i64,
) -> Result<group_entity::Model, ApiResponse<T>>
where
    T: Serialize,
{
    if conversation_id <= 0 {
        return Err(bad_request("conversation_id must be positive"));
    }

    let conversation = conversation::find_conversation(db, conversation_id)
        .await
        .map_err(|e| internal_error(format!("query conversation failed: {e}")))?
        .ok_or_else(|| bad_request("conversation not found"))?;
    if conversation.conversation_type != ConversationType::Group {
        return Err(bad_request("conversation is not a group chat"));
    }

    group_model::find_by_conversation_id(db, conversation_id)
        .await
        .map_err(|e| internal_error(format!("query group failed: {e}")))?
        .ok_or_else(|| bad_request("group metadata not found"))
}

pub async fn load_group_owned_by<T>(
    db: &DatabaseConnection,
    conversation_id: i64,
    owner_id: i64,
    permission_error: &'static str,
) -> Result<group_entity::Model, ApiResponse<T>>
where
    T: Serialize,
{
    let group = load_group(db, conversation_id).await?;
    if group.owner_id != owner_id {
        return Err(bad_request(permission_error));
    }

    Ok(group)
}

pub async fn load_owned_message<T, C>(
    db: &C,
    operator_id: i64,
    message_id: i64,
    expected_conversation_id: Option<i64>,
    permission_error: &'static str,
) -> Result<message_entity::Model, ApiResponse<T>>
where
    T: Serialize,
    C: ConnectionTrait,
{
    if let Some(conversation_id) = expected_conversation_id
        && conversation_id <= 0
    {
        return Err(bad_request("conversation_id must be positive"));
    }

    if message_id <= 0 {
        return Err(bad_request("message_id must be positive"));
    }

    let message = message_entity::Entity::find_by_id(message_id)
        .one(db)
        .await
        .map_err(|e| bad_request(format!("query message failed: {e}")))?
        .ok_or_else(|| bad_request("message not found"))?;

    if let Some(conversation_id) = expected_conversation_id
        && message.conversation_id != conversation_id
    {
        return Err(bad_request("message does not belong to conversation"));
    }

    if message.sender_id != operator_id {
        return Err(bad_request(permission_error));
    }

    Ok(message)
}

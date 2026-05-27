use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cobotusr::common::{ApiEnvelope, bearer_token, register_user, spawn_test_server};

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    to: i64,
    msg_type: i16,
    content: Option<String>,
    extra: Option<serde_json::Value>,
    client_msg_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateConversationRequest {
    conversation_type: i16,
    title: Option<String>,
    member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct RecallMessageRequest {
    message_id: i64,
}

#[derive(Debug, Serialize)]
struct DeleteMessageRequest {
    message_id: i64,
}

#[derive(Debug, Deserialize)]
struct SendMessageResponse {
    message_id: i64,
    conversation_id: i64,
    client_msg_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateConversationResponse {
    conversation_id: i64,
}

#[derive(Debug, Deserialize)]
struct ConversationListResponse {
    items: Vec<ConversationSummary>,
}

#[derive(Debug, Deserialize)]
struct ConversationSummary {
    conversation_id: i64,
    unread_count: i32,
    last_message_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetMessagesResponse {
    items: Vec<MessageItem>,
}

#[derive(Debug, Deserialize)]
struct MessageItem {
    id: i64,
    conversation_id: i64,
    sender_id: i64,
    content: Option<String>,
    is_recalled: bool,
}

#[derive(Debug, Deserialize)]
struct ClearUnreadResponse {
    conversation_id: i64,
    unread_count: i32,
    last_read_message_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RecallMessageResponse {
    message_id: i64,
    conversation_id: i64,
}

#[derive(Debug, Deserialize)]
struct DeleteMessageResponse {
    message_id: i64,
    conversation_id: i64,
}

#[tokio::test]
async fn test_private_chat_send_and_unread_flow() -> Result<()> {
    let server = spawn_test_server().await?;
    let alice = register_user(&server, "alice").await?;
    let bob = register_user(&server, "bob").await?;

    let client_msg_id = format!("client-{}", uuid::Uuid::new_v4());
    let content = "hello from alice".to_string();
    let conversation_id = create_private_conversation(&server, &alice.token, bob.id).await?;

    let send_resp = server
        .client
        .post(server.api_url(&format!("/im/conversations/{conversation_id}/messages")))
        .header("Authorization", bearer_token(&alice.token))
        .json(&SendMessageRequest {
            to: bob.id,
            msg_type: 1,
            content: Some(content.clone()),
            extra: None,
            client_msg_id: Some(client_msg_id.clone()),
        })
        .send()
        .await?
        .error_for_status()?;

    let send_data: SendMessageResponse = send_resp
        .json::<ApiEnvelope<SendMessageResponse>>()
        .await?
        .into_data()?;

    assert!(send_data.message_id > 0);
    assert!(send_data.conversation_id > 0);
    assert_eq!(send_data.conversation_id, conversation_id);
    assert_eq!(
        send_data.client_msg_id.as_deref(),
        Some(client_msg_id.as_str())
    );

    let alice_list = fetch_conversations(&server, &alice.token).await?;
    let alice_conversation = alice_list
        .items
        .iter()
        .find(|item| item.conversation_id == send_data.conversation_id)
        .context("alice conversation missing after private chat")?;
    assert_eq!(alice_conversation.unread_count, 0);
    assert_eq!(
        alice_conversation.last_message_preview.as_deref(),
        Some(content.as_str())
    );

    let bob_list = fetch_conversations(&server, &bob.token).await?;
    let bob_conversation = bob_list
        .items
        .iter()
        .find(|item| item.conversation_id == send_data.conversation_id)
        .context("bob conversation missing after private chat")?;
    assert_eq!(bob_conversation.unread_count, 1);
    assert_eq!(
        bob_conversation.last_message_preview.as_deref(),
        Some(content.as_str())
    );

    let messages_resp = server
        .client
        .get(server.api_url(&format!(
            "/im/conversations/{}/messages",
            send_data.conversation_id
        )))
        .header("Authorization", bearer_token(&bob.token))
        .query(&[("limit", "20".to_string())])
        .send()
        .await?
        .error_for_status()?;

    let messages_data: GetMessagesResponse = messages_resp
        .json::<ApiEnvelope<GetMessagesResponse>>()
        .await?
        .into_data()?;
    assert_eq!(messages_data.items.len(), 1);

    let message = &messages_data.items[0];
    assert_eq!(message.id, send_data.message_id);
    assert_eq!(message.conversation_id, send_data.conversation_id);
    assert_eq!(message.sender_id, alice.id);
    assert_eq!(message.content.as_deref(), Some(content.as_str()));
    assert!(!message.is_recalled);

    let clear_resp = server
        .client
        .put(server.api_url(&format!(
            "/im/conversations/{}/read-state",
            send_data.conversation_id
        )))
        .header("Authorization", bearer_token(&bob.token))
        .send()
        .await?
        .error_for_status()?;

    let clear_data: ClearUnreadResponse = clear_resp
        .json::<ApiEnvelope<ClearUnreadResponse>>()
        .await?
        .into_data()?;
    assert_eq!(clear_data.conversation_id, send_data.conversation_id);
    assert_eq!(clear_data.unread_count, 0);
    assert_eq!(clear_data.last_read_message_id, Some(send_data.message_id));

    let bob_list_after_clear = fetch_conversations(&server, &bob.token).await?;
    let bob_conversation_after_clear = bob_list_after_clear
        .items
        .iter()
        .find(|item| item.conversation_id == send_data.conversation_id)
        .context("bob conversation missing after clear_unread")?;
    assert_eq!(bob_conversation_after_clear.unread_count, 0);

    let recall_resp = server
        .client
        .patch(server.api_url(&format!(
            "/im/conversations/{}/messages",
            send_data.conversation_id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&RecallMessageRequest {
            message_id: send_data.message_id,
        })
        .send()
        .await?
        .error_for_status()?;

    let recall_data: RecallMessageResponse = recall_resp
        .json::<ApiEnvelope<RecallMessageResponse>>()
        .await?
        .into_data()?;
    assert_eq!(recall_data.message_id, send_data.message_id);
    assert_eq!(recall_data.conversation_id, send_data.conversation_id);

    let messages_after_recall =
        fetch_messages(&server, &bob.token, send_data.conversation_id).await?;
    assert_eq!(messages_after_recall.items.len(), 1);
    let recalled = &messages_after_recall.items[0];
    assert_eq!(recalled.id, send_data.message_id);
    assert!(recalled.is_recalled);
    assert!(recalled.content.is_none());

    let delete_content = "delete me".to_string();
    let delete_send_resp = server
        .client
        .post(server.api_url(&format!(
            "/im/conversations/{}/messages",
            send_data.conversation_id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&SendMessageRequest {
            to: bob.id,
            msg_type: 1,
            content: Some(delete_content),
            extra: None,
            client_msg_id: Some(format!("delete-{}", uuid::Uuid::new_v4())),
        })
        .send()
        .await?
        .error_for_status()?;

    let delete_send_data: SendMessageResponse = delete_send_resp
        .json::<ApiEnvelope<SendMessageResponse>>()
        .await?
        .into_data()?;
    assert!(delete_send_data.message_id > send_data.message_id);

    let delete_resp = server
        .client
        .delete(server.api_url(&format!(
            "/im/conversations/{}/messages",
            send_data.conversation_id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&DeleteMessageRequest {
            message_id: delete_send_data.message_id,
        })
        .send()
        .await?
        .error_for_status()?;

    let delete_data: DeleteMessageResponse = delete_resp
        .json::<ApiEnvelope<DeleteMessageResponse>>()
        .await?
        .into_data()?;
    assert_eq!(delete_data.message_id, delete_send_data.message_id);
    assert_eq!(delete_data.conversation_id, send_data.conversation_id);

    let messages_after_delete =
        fetch_messages(&server, &bob.token, send_data.conversation_id).await?;
    assert_eq!(messages_after_delete.items.len(), 1);
    assert_eq!(messages_after_delete.items[0].id, send_data.message_id);

    Ok(())
}

async fn create_private_conversation(
    server: &crate::cobotusr::common::TestServer,
    token: &str,
    peer_id: i64,
) -> Result<i64> {
    let response = server
        .client
        .post(server.api_url("/im/conversations"))
        .header("Authorization", bearer_token(token))
        .json(&CreateConversationRequest {
            conversation_type: 1,
            title: None,
            member_ids: vec![peer_id],
        })
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("create private conversation failed: {status}, body: {body}");
    }

    let data: CreateConversationResponse = response
        .json::<ApiEnvelope<CreateConversationResponse>>()
        .await?
        .into_data()?;
    Ok(data.conversation_id)
}

async fn fetch_messages(
    server: &crate::cobotusr::common::TestServer,
    token: &str,
    conversation_id: i64,
) -> Result<GetMessagesResponse> {
    let response = server
        .client
        .get(server.api_url(&format!("/im/conversations/{conversation_id}/messages")))
        .header("Authorization", bearer_token(token))
        .query(&[("limit", "20".to_string())])
        .send()
        .await?
        .error_for_status()?;

    response
        .json::<ApiEnvelope<GetMessagesResponse>>()
        .await?
        .into_data()
}

async fn fetch_conversations(
    server: &crate::cobotusr::common::TestServer,
    token: &str,
) -> Result<ConversationListResponse> {
    let response = server
        .client
        .get(server.api_url("/im/conversations"))
        .header("Authorization", bearer_token(token))
        .query(&[("limit", "20")])
        .send()
        .await?
        .error_for_status()?;

    response
        .json::<ApiEnvelope<ConversationListResponse>>()
        .await?
        .into_data()
}

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::cobotusr::common::{ApiEnvelope, bearer_token, register_user, spawn_test_server};

#[derive(Debug, Serialize)]
struct CreateConversationRequest {
    conversation_type: i16,
    title: Option<String>,
    member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct AddMembersRequest {
    conversation_id: i64,
    member_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct TransferOwnerRequest {
    conversation_id: i64,
    new_owner_id: i64,
}

#[derive(Debug, Serialize)]
struct RemoveMemberRequest {
    conversation_id: i64,
    user_id: i64,
}

#[derive(Debug, Serialize)]
struct SendGroupMessageRequest {
    conversation_id: i64,
    msg_type: i16,
    content: Option<String>,
    extra: Option<serde_json::Value>,
    client_msg_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateConversationResponse {
    conversation_id: i64,
    conversation_type: i16,
    title: Option<String>,
    member_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct AddMembersResponse {
    conversation_id: i64,
    added_member_ids: Vec<i64>,
    skipped_member_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct TransferOwnerResponse {
    conversation_id: i64,
    old_owner_id: i64,
    new_owner_id: i64,
}

#[derive(Debug, Deserialize)]
struct RemoveMemberResponse {
    conversation_id: i64,
    user_id: i64,
    removed: bool,
}

#[derive(Debug, Deserialize)]
struct SendGroupMessageResponse {
    message_id: i64,
    conversation_id: i64,
    client_msg_id: Option<String>,
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
}

#[tokio::test]
async fn test_group_creation_and_owner_permissions() -> Result<()> {
    let server = spawn_test_server().await?;
    let alice = register_user(&server, "alice").await?;
    let bob = register_user(&server, "bob").await?;
    let carol = register_user(&server, "carol").await?;
    let dave = register_user(&server, "dave").await?;
    let erin = register_user(&server, "erin").await?;

    let create_resp = server
        .client
        .post(server.api_url("/im/conversations"))
        .header("Authorization", bearer_token(&alice.token))
        .json(&CreateConversationRequest {
            conversation_type: 2,
            title: Some("Project Group".to_string()),
            member_ids: vec![bob.id, carol.id],
        })
        .send()
        .await?
        .error_for_status()?;

    let mut create_data: CreateConversationResponse = create_resp
        .json::<ApiEnvelope<CreateConversationResponse>>()
        .await?
        .into_data()?;

    assert!(create_data.conversation_id > 0);
    assert_eq!(create_data.conversation_type, 2);
    assert_eq!(create_data.title.as_deref(), Some("Project Group"));

    create_data.member_ids.sort_unstable();
    assert_eq!(create_data.member_ids, vec![alice.id, bob.id, carol.id]);

    let group_client_msg_id = format!("group-{}", uuid::Uuid::new_v4());
    let group_content = "hello group".to_string();
    let send_group_resp = server
        .client
        .post(server.api_url(&format!(
            "/im/conversations/{}/messages",
            create_data.conversation_id
        )))
        .header("Authorization", bearer_token(&bob.token))
        .json(&SendGroupMessageRequest {
            conversation_id: create_data.conversation_id,
            msg_type: 1,
            content: Some(group_content.clone()),
            extra: None,
            client_msg_id: Some(group_client_msg_id.clone()),
        })
        .send()
        .await?
        .error_for_status()?;

    let send_group_data: SendGroupMessageResponse = send_group_resp
        .json::<ApiEnvelope<SendGroupMessageResponse>>()
        .await?
        .into_data()?;
    assert!(send_group_data.message_id > 0);
    assert_eq!(send_group_data.conversation_id, create_data.conversation_id);
    assert_eq!(
        send_group_data.client_msg_id.as_deref(),
        Some(group_client_msg_id.as_str())
    );

    let group_messages = fetch_messages(&server, &carol.token, create_data.conversation_id).await?;
    assert_eq!(group_messages.items.len(), 1);
    let group_message = &group_messages.items[0];
    assert_eq!(group_message.id, send_group_data.message_id);
    assert_eq!(group_message.conversation_id, create_data.conversation_id);
    assert_eq!(group_message.sender_id, bob.id);
    assert_eq!(
        group_message.content.as_deref(),
        Some(group_content.as_str())
    );

    let unauthorized_add_resp = server
        .client
        .post(server.api_url(&format!(
            "/im/conversations/{}/members",
            create_data.conversation_id
        )))
        .header("Authorization", bearer_token(&bob.token))
        .json(&AddMembersRequest {
            conversation_id: create_data.conversation_id,
            member_ids: vec![dave.id],
        })
        .send()
        .await?;
    assert_eq!(
        unauthorized_add_resp.status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    let unauthorized_add_error: ApiEnvelope<()> = unauthorized_add_resp.json().await?;
    assert!(
        unauthorized_add_error
            .message
            .contains("only group owner can add members")
    );

    let add_resp = server
        .client
        .post(server.api_url(&format!(
            "/im/conversations/{}/members",
            create_data.conversation_id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&AddMembersRequest {
            conversation_id: create_data.conversation_id,
            member_ids: vec![dave.id],
        })
        .send()
        .await?
        .error_for_status()?;

    let add_data: AddMembersResponse = add_resp
        .json::<ApiEnvelope<AddMembersResponse>>()
        .await?
        .into_data()?;
    assert_eq!(add_data.conversation_id, create_data.conversation_id);
    assert_eq!(add_data.added_member_ids, vec![dave.id]);
    assert!(add_data.skipped_member_ids.is_empty());

    let transfer_resp = server
        .client
        .patch(server.api_url(&format!(
            "/im/conversations/{}/owner",
            create_data.conversation_id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&TransferOwnerRequest {
            conversation_id: create_data.conversation_id,
            new_owner_id: bob.id,
        })
        .send()
        .await?
        .error_for_status()?;

    let transfer_data: TransferOwnerResponse = transfer_resp
        .json::<ApiEnvelope<TransferOwnerResponse>>()
        .await?
        .into_data()?;
    assert_eq!(transfer_data.conversation_id, create_data.conversation_id);
    assert_eq!(transfer_data.old_owner_id, alice.id);
    assert_eq!(transfer_data.new_owner_id, bob.id);

    let former_owner_resp = server
        .client
        .post(server.api_url(&format!(
            "/im/conversations/{}/members",
            create_data.conversation_id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&AddMembersRequest {
            conversation_id: create_data.conversation_id,
            member_ids: vec![erin.id],
        })
        .send()
        .await?;
    assert_eq!(former_owner_resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let former_owner_error: ApiEnvelope<()> = former_owner_resp.json().await?;
    assert!(
        former_owner_error
            .message
            .contains("only group owner can add members")
    );

    let new_owner_add_resp = server
        .client
        .post(server.api_url(&format!(
            "/im/conversations/{}/members",
            create_data.conversation_id
        )))
        .header("Authorization", bearer_token(&bob.token))
        .json(&AddMembersRequest {
            conversation_id: create_data.conversation_id,
            member_ids: vec![erin.id],
        })
        .send()
        .await?
        .error_for_status()?;

    let new_owner_add_data: AddMembersResponse = new_owner_add_resp
        .json::<ApiEnvelope<AddMembersResponse>>()
        .await?
        .into_data()?;
    assert_eq!(new_owner_add_data.added_member_ids, vec![erin.id]);

    let former_owner_remove_resp = server
        .client
        .delete(server.api_url(&format!(
            "/im/conversations/{}/members/{}",
            create_data.conversation_id, carol.id
        )))
        .header("Authorization", bearer_token(&alice.token))
        .json(&RemoveMemberRequest {
            conversation_id: create_data.conversation_id,
            user_id: carol.id,
        })
        .send()
        .await?;
    assert_eq!(
        former_owner_remove_resp.status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    let former_owner_remove_error: ApiEnvelope<()> = former_owner_remove_resp.json().await?;
    assert!(
        former_owner_remove_error
            .message
            .contains("only group owner can remove members")
    );

    let remove_resp = server
        .client
        .delete(server.api_url(&format!(
            "/im/conversations/{}/members/{}",
            create_data.conversation_id, carol.id
        )))
        .header("Authorization", bearer_token(&bob.token))
        .json(&RemoveMemberRequest {
            conversation_id: create_data.conversation_id,
            user_id: carol.id,
        })
        .send()
        .await?
        .error_for_status()?;

    let remove_data: RemoveMemberResponse = remove_resp
        .json::<ApiEnvelope<RemoveMemberResponse>>()
        .await?
        .into_data()?;
    assert_eq!(remove_data.conversation_id, create_data.conversation_id);
    assert_eq!(remove_data.user_id, carol.id);
    assert!(remove_data.removed);

    Ok(())
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

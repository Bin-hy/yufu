use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use cobotusr::modules::im::model::ws_message::{
    EVENT_CHAT_PUSH, EVENT_CONVERSATION_READ_PUSH, EVENT_HEARTBEAT_PING, EVENT_HEARTBEAT_PONG,
    WsChatPushPayload, WsClientEnvelope, WsConversationReadPushPayload, WsHeartbeatPayload,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use crate::cobotusr::common::{ApiEnvelope, bearer_token, register_user, spawn_test_server};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
struct ClearUnreadResponse {
    conversation_id: i64,
    unread_count: i32,
    last_read_message_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct DispatchOutboxRequest {
    limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DispatchOutboxResponse {
    sent_count: u64,
}

#[derive(Debug, Deserialize)]
struct WsEnvelope {
    event: String,
    status: u16,
    message: String,
    data: Option<serde_json::Value>,
    seq: Option<String>,
}

#[tokio::test]
async fn test_websocket_heartbeat_and_http_push_end_to_end() -> Result<()> {
    let server = spawn_test_server().await?;
    let alice = register_user(&server, "alice").await?;
    let bob = register_user(&server, "bob").await?;

    let mut bob_ws = connect_ws(&server, &bob.token).await?;

    let heartbeat_seq = format!("heartbeat-{}", uuid::Uuid::new_v4());
    let heartbeat = WsClientEnvelope {
        event: EVENT_HEARTBEAT_PING.to_string(),
        seq: Some(heartbeat_seq.clone()),
        data: Some(serde_json::json!({})),
    };

    bob_ws
        .send(Message::Text(serde_json::to_string(&heartbeat)?.into()))
        .await?;

    let heartbeat_msg = recv_ws_envelope(&mut bob_ws).await?;
    assert_eq!(heartbeat_msg.event, EVENT_HEARTBEAT_PONG);
    assert_eq!(heartbeat_msg.status, 200);
    assert_eq!(heartbeat_msg.message, "ok");
    assert_eq!(heartbeat_msg.seq.as_deref(), Some(heartbeat_seq.as_str()));

    let heartbeat_payload: WsHeartbeatPayload = serde_json::from_value(
        heartbeat_msg
            .data
            .context("heartbeat payload missing from websocket response")?,
    )?;
    assert!(heartbeat_payload.ts > 0);

    let client_msg_id = format!("ws-client-{}", uuid::Uuid::new_v4());
    let content = "hello over http".to_string();
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
        .await?;

    let send_data: SendMessageResponse = send_resp
        .error_for_status()?
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

    let bob_msg = recv_ws_envelope(&mut bob_ws).await?;
    assert_eq!(bob_msg.event, EVENT_CHAT_PUSH);
    assert_eq!(bob_msg.status, 200);
    assert_eq!(bob_msg.message, "ok");
    assert!(bob_msg.seq.is_none());

    let push: WsChatPushPayload = serde_json::from_value(
        bob_msg
            .data
            .context("bob push payload missing from websocket response")?,
    )?;
    assert_eq!(push.message_id, send_data.message_id);
    assert_eq!(push.conversation_id, send_data.conversation_id);
    assert_eq!(push.from, alice.id);
    assert_eq!(push.to, bob.id);
    assert_eq!(push.content.as_deref(), Some(content.as_str()));
    assert_eq!(
        push.extra
            .as_ref()
            .and_then(|value| value.get("client_msg_id"))
            .and_then(|value| value.as_str()),
        Some(client_msg_id.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn test_websocket_rejects_legacy_command_events() -> Result<()> {
    let server = spawn_test_server().await?;
    let alice = register_user(&server, "alice").await?;
    let mut alice_ws = connect_ws(&server, &alice.token).await?;

    let legacy_command = WsClientEnvelope {
        event: "chat.send".to_string(),
        seq: Some(format!("legacy-{}", uuid::Uuid::new_v4())),
        data: Some(serde_json::json!({
            "to": 999,
            "msg_type": 1,
            "content": "legacy command"
        })),
    };

    alice_ws
        .send(Message::Text(
            serde_json::to_string(&legacy_command)?.into(),
        ))
        .await?;

    let response = recv_ws_envelope(&mut alice_ws).await?;
    assert_eq!(response.event, "chat.send");
    assert_eq!(response.status, 400);
    assert_eq!(
        response.message,
        "websocket is receive-only; use HTTP endpoints"
    );
    assert!(response.data.is_none());

    Ok(())
}

#[tokio::test]
async fn test_websocket_replays_message_after_recipient_reconnects() -> Result<()> {
    let server = spawn_test_server().await?;
    let alice = register_user(&server, "alice").await?;
    let bob = register_user(&server, "bob").await?;

    let client_msg_id = format!("offline-{}", uuid::Uuid::new_v4());
    let content = "hello after reconnect".to_string();

    let send_data = send_private_message(
        &server,
        &alice.token,
        bob.id,
        content.clone(),
        client_msg_id.clone(),
    )
    .await?;

    sleep(Duration::from_millis(100)).await;
    let dispatch_resp = server
        .client
        .post(server.api_url("/im/outbox-dispatches"))
        .header("Authorization", bearer_token(&alice.token))
        .json(&DispatchOutboxRequest { limit: Some(100) })
        .send()
        .await?
        .error_for_status()?;

    let dispatch_data: DispatchOutboxResponse = dispatch_resp
        .json::<ApiEnvelope<DispatchOutboxResponse>>()
        .await?
        .into_data()?;
    assert_eq!(dispatch_data.sent_count, 0);

    let mut bob_ws = connect_ws(&server, &bob.token).await?;
    let bob_msg = recv_ws_envelope(&mut bob_ws).await?;
    assert_eq!(bob_msg.event, EVENT_CHAT_PUSH);
    assert_eq!(bob_msg.status, 200);
    assert_eq!(bob_msg.message, "ok");

    let push: WsChatPushPayload = serde_json::from_value(
        bob_msg
            .data
            .context("bob replay payload missing from websocket response")?,
    )?;
    assert_eq!(push.message_id, send_data.message_id);
    assert_eq!(push.conversation_id, send_data.conversation_id);
    assert_eq!(push.from, alice.id);
    assert_eq!(push.to, bob.id);
    assert_eq!(push.content.as_deref(), Some(content.as_str()));
    assert_eq!(
        push.extra
            .as_ref()
            .and_then(|value| value.get("client_msg_id"))
            .and_then(|value| value.as_str()),
        Some(client_msg_id.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn test_websocket_receives_conversation_read_push_after_http_clear_unread() -> Result<()> {
    let server = spawn_test_server().await?;
    let alice = register_user(&server, "alice").await?;
    let bob = register_user(&server, "bob").await?;

    let mut alice_ws = connect_ws(&server, &alice.token).await?;

    let send_data = send_private_message(
        &server,
        &alice.token,
        bob.id,
        "message awaiting read".to_string(),
        format!("read-{}", uuid::Uuid::new_v4()),
    )
    .await?;

    let clear_data = clear_unread(&server, &bob.token, send_data.conversation_id).await?;
    assert_eq!(clear_data.conversation_id, send_data.conversation_id);
    assert_eq!(clear_data.unread_count, 0);
    assert_eq!(clear_data.last_read_message_id, Some(send_data.message_id));

    let alice_msg = recv_ws_envelope(&mut alice_ws).await?;
    assert_eq!(alice_msg.event, EVENT_CONVERSATION_READ_PUSH);
    assert_eq!(alice_msg.status, 200);
    assert_eq!(alice_msg.message, "ok");

    let push: WsConversationReadPushPayload = serde_json::from_value(
        alice_msg
            .data
            .context("conversation.read.push payload missing from websocket response")?,
    )?;
    assert_eq!(push.conversation_id, send_data.conversation_id);
    assert_eq!(push.reader_id, bob.id);
    assert_eq!(push.last_read_message_id, Some(send_data.message_id));

    Ok(())
}

async fn connect_ws(server: &crate::cobotusr::common::TestServer, token: &str) -> Result<WsStream> {
    let mut request = server.ws_url("/im/ws").into_client_request()?;
    request
        .headers_mut()
        .insert("Authorization", bearer_token(token).parse()?);

    let (stream, _) = connect_async(request).await?;
    Ok(stream)
}

async fn recv_ws_envelope(stream: &mut WsStream) -> Result<WsEnvelope> {
    let text = timeout(Duration::from_secs(5), async {
        loop {
            match stream.next().await {
                Some(Ok(Message::Text(text))) => break Ok(text.to_string()),
                Some(Ok(Message::Binary(_))) => continue,
                Some(Ok(Message::Ping(_))) => continue,
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Frame(_))) => continue,
                Some(Ok(Message::Close(_))) => break Err(anyhow!("websocket closed unexpectedly")),
                Some(Err(err)) => break Err(anyhow!(err)),
                None => break Err(anyhow!("websocket stream ended unexpectedly")),
            }
        }
    })
    .await
    .context("timed out waiting for websocket message")??;

    Ok(serde_json::from_str(&text)?)
}

async fn send_private_message(
    server: &crate::cobotusr::common::TestServer,
    token: &str,
    to: i64,
    content: String,
    client_msg_id: String,
) -> Result<SendMessageResponse> {
    let conversation_id = create_private_conversation(server, token, to).await?;
    let response = server
        .client
        .post(server.api_url(&format!("/im/conversations/{conversation_id}/messages")))
        .header("Authorization", bearer_token(token))
        .json(&SendMessageRequest {
            to,
            msg_type: 1,
            content: Some(content),
            extra: None,
            client_msg_id: Some(client_msg_id),
        })
        .send()
        .await?
        .error_for_status()?;

    response
        .json::<ApiEnvelope<SendMessageResponse>>()
        .await?
        .into_data()
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
        .await?
        .error_for_status()?;

    let data: CreateConversationResponse = response
        .json::<ApiEnvelope<CreateConversationResponse>>()
        .await?
        .into_data()?;
    Ok(data.conversation_id)
}

async fn clear_unread(
    server: &crate::cobotusr::common::TestServer,
    token: &str,
    conversation_id: i64,
) -> Result<ClearUnreadResponse> {
    let response = server
        .client
        .put(server.api_url(&format!("/im/conversations/{conversation_id}/read-state")))
        .header("Authorization", bearer_token(token))
        .send()
        .await?
        .error_for_status()?;

    response
        .json::<ApiEnvelope<ClearUnreadResponse>>()
        .await?
        .into_data()
}

use std::{
    collections::HashMap,
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
};

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use http::StatusCode;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tokio::sync::RwLock;

use crate::modules::im::model::ws_message::{
    EVENT_HEARTBEAT_PING, EVENT_HEARTBEAT_PONG, WsClientEnvelope, WsHeartbeatPayload,
    WsServerEnvelope,
};

use super::outbox_service;

pub type UserId = i64;
type ConnectionId = u64;
pub type WsSender = tokio::sync::mpsc::UnboundedSender<Message>;

pub struct ConnectionManager {
    connections: RwLock<HashMap<UserId, HashMap<ConnectionId, WsSender>>>,
    next_connection_id: AtomicU64,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            next_connection_id: AtomicU64::new(1),
        }
    }

    pub fn alloc_connection_id(&self) -> ConnectionId {
        self.next_connection_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn add_connection(
        &self,
        user_id: UserId,
        connection_id: ConnectionId,
        sender: WsSender,
    ) {
        let mut map = self.connections.write().await;
        map.entry(user_id)
            .or_default()
            .insert(connection_id, sender);
    }

    pub async fn remove_connection(&self, user_id: UserId, connection_id: ConnectionId) {
        let mut map = self.connections.write().await;
        if let Some(user_map) = map.get_mut(&user_id) {
            user_map.remove(&connection_id);
            if user_map.is_empty() {
                map.remove(&user_id);
            }
        }
    }

    pub async fn send_to_user(&self, user_id: UserId, msg: Message) {
        let map = self.connections.read().await;
        if let Some(sender_map) = map.get(&user_id) {
            for sender in sender_map.values() {
                let _ = sender.send(msg.clone());
            }
        }
    }

    pub async fn send_to_users(&self, user_ids: Vec<UserId>, msg: Message) {
        let map = self.connections.read().await;
        for uid in user_ids {
            if let Some(sender_map) = map.get(&uid) {
                for sender in sender_map.values() {
                    let _ = sender.send(msg.clone());
                }
            }
        }
    }

    pub async fn online_user_ids(&self, user_ids: &[UserId]) -> Vec<UserId> {
        let map = self.connections.read().await;
        user_ids
            .iter()
            .copied()
            .filter(|uid| map.get(uid).map(|v| !v.is_empty()).unwrap_or(false))
            .collect()
    }

    pub async fn send_text_to_online_users(&self, user_ids: &[UserId], text: &str) -> usize {
        let map = self.connections.read().await;
        let mut delivered = 0usize;
        for uid in user_ids {
            if let Some(sender_map) = map.get(uid) {
                let mut user_delivered = false;
                for sender in sender_map.values() {
                    if sender.send(Message::Text(text.to_string().into())).is_ok() {
                        user_delivered = true;
                    }
                }
                if user_delivered {
                    delivered = delivered.saturating_add(1);
                }
            }
        }
        delivered
    }
}

pub type SharedConnectionManager = Arc<ConnectionManager>;
pub type WsHandle = SharedConnectionManager;

async fn send_envelope<T>(
    manager: &SharedConnectionManager,
    user_id: UserId,
    envelope: WsServerEnvelope<T>,
) where
    T: Serialize,
{
    if let Ok(text) = serde_json::to_string(&envelope) {
        manager
            .send_to_user(user_id, Message::Text(text.into()))
            .await;
    }
}

async fn send_error(
    manager: &SharedConnectionManager,
    user_id: UserId,
    event: &str,
    status: StatusCode,
    msg: &str,
) {
    send_envelope::<serde_json::Value>(manager, user_id, WsServerEnvelope::err(event, status, msg))
        .await;
}

pub async fn handle_socket(
    socket: WebSocket,
    user_id: UserId,
    manager: SharedConnectionManager,
    db: DatabaseConnection,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let connection_id = manager.alloc_connection_id();

    manager.add_connection(user_id, connection_id, tx).await;
    println!("user {} connected", user_id);

    let boot_manager = manager.clone();
    let boot_db = db.clone();
    tokio::spawn(async move {
        let _ =
            outbox_service::dispatch_pending_for_online_users(&boot_db, boot_manager, 200).await;
    });

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let manager_clone = manager.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    let envelope = match serde_json::from_str::<WsClientEnvelope>(&text) {
                        Ok(v) => v,
                        Err(_) => {
                            send_error(
                                &manager,
                                user_id,
                                "unknown",
                                StatusCode::BAD_REQUEST,
                                "invalid websocket payload",
                            )
                            .await;
                            continue;
                        }
                    };

                    match envelope.event.as_str() {
                        EVENT_HEARTBEAT_PING => {
                            let pong = WsServerEnvelope::ok(
                                EVENT_HEARTBEAT_PONG,
                                WsHeartbeatPayload {
                                    ts: chrono::Utc::now().timestamp_millis(),
                                },
                                envelope.seq.clone(),
                            );
                            send_envelope(&manager, user_id, pong).await;
                        }
                        _ => {
                            send_error(
                                &manager,
                                user_id,
                                &envelope.event,
                                StatusCode::BAD_REQUEST,
                                "websocket is receive-only; use HTTP endpoints",
                            )
                            .await;
                        }
                    }
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }

        manager_clone
            .remove_connection(user_id, connection_id)
            .await;
        println!("user {} disconnected", user_id);
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
}

use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::modules::im::model::outbox;

use super::websocket_service::SharedConnectionManager;

const MAX_RETRY: i32 = 30;
const RETRY_BACKOFF_SECS: i32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutboxPayload {
    targets: Vec<i64>,
    envelope: Value,
}

fn normalize_targets(mut targets: Vec<i64>) -> Vec<i64> {
    targets.retain(|id| *id > 0);
    targets.sort_unstable();
    targets.dedup();
    targets
}

pub async fn enqueue_message_push(
    db: &DatabaseConnection,
    message_id: i64,
    conversation_id: i64,
    targets: Vec<i64>,
    envelope: Value,
) -> Result<(), String> {
    let targets = normalize_targets(targets);
    if targets.is_empty() {
        return Ok(());
    }

    let payload = serde_json::to_value(OutboxPayload { targets, envelope })
        .map_err(|e| format!("serialize outbox payload failed: {e}"))?;

    outbox::insert_pending(db, message_id, conversation_id, payload)
        .await
        .map_err(|e| format!("insert outbox failed: {e}"))?;

    Ok(())
}

pub async fn dispatch_due(
    db: &DatabaseConnection,
    manager: SharedConnectionManager,
    limit: u64,
) -> Result<usize, String> {
    let tasks = outbox::list_due_tasks(db, limit)
        .await
        .map_err(|e| format!("list outbox tasks failed: {e}"))?;

    let mut sent_count = 0usize;
    for task in tasks {
        if dispatch_one(db, &manager, task).await? {
            sent_count += 1;
        }
    }
    Ok(sent_count)
}

pub async fn dispatch_pending_for_online_users(
    db: &DatabaseConnection,
    manager: SharedConnectionManager,
    limit: u64,
) -> Result<usize, String> {
    let tasks = outbox::list_pending_retry_tasks(db, limit)
        .await
        .map_err(|e| format!("list outbox tasks failed: {e}"))?;

    let mut sent_count = 0usize;
    for task in tasks {
        if dispatch_one(db, &manager, task).await? {
            sent_count += 1;
        }
    }
    Ok(sent_count)
}

async fn dispatch_one(
    db: &DatabaseConnection,
    manager: &SharedConnectionManager,
    task: outbox::OutboxTask,
) -> Result<bool, String> {
    let payload: OutboxPayload = serde_json::from_value(task.payload)
        .map_err(|e| format!("decode outbox payload failed: {e}"))?;

    let text = serde_json::to_string(&payload.envelope)
        .map_err(|e| format!("serialize outbox envelope failed: {e}"))?;

    let online_targets = manager.online_user_ids(&payload.targets).await;
    if online_targets.is_empty() {
        outbox::mark_retry(
            db,
            task.id,
            task.retry_count,
            MAX_RETRY,
            RETRY_BACKOFF_SECS,
            &format!(
                "message {} conversation {}: no online targets",
                task.message_id, task.conversation_id
            ),
        )
        .await
        .map_err(|e| format!("mark outbox retry failed: {e}"))?;
        return Ok(false);
    }

    let delivered = manager
        .send_text_to_online_users(&online_targets, &text)
        .await;
    if delivered == 0 {
        outbox::mark_retry(
            db,
            task.id,
            task.retry_count,
            MAX_RETRY,
            RETRY_BACKOFF_SECS,
            &format!(
                "message {} conversation {}: all online sends failed",
                task.message_id, task.conversation_id
            ),
        )
        .await
        .map_err(|e| format!("mark outbox retry failed: {e}"))?;
        return Ok(false);
    }

    outbox::mark_sent(db, task.id)
        .await
        .map_err(|e| format!("mark outbox sent failed: {e}"))?;
    Ok(true)
}

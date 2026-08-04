use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, Publish, QoS};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use crate::config::Mqtt as MqttConfig;
use crate::service::device::{DeviceService, UpsertDevice};

const PUBLISH_CHANNEL_CAPACITY: usize = 64;

/// Mirror of `cobotlet::dispatcher::Task` (kept independent so cobotsrv does
/// not depend on cobotlet at compile time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    /// MCP-style tool name. Accepts legacy `capability_name` for backward compat.
    #[serde(alias = "capability_name")]
    pub name: String,
    #[serde(default)]
    pub device_id: Option<String>,
    /// MCP-style tool input. Accepts legacy `body` for backward compat.
    #[serde(default, alias = "body")]
    pub input: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// MCP-style content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    pub r#type: String,
    pub text: String,
}

/// Mirror of `cobotlet::dispatcher::TaskResult`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    /// MCP-style error flag.
    pub is_error: bool,
    /// MCP-style content blocks carrying the response payload.
    pub content: Vec<ContentBlock>,
    /// HTTP status code from the device service. 0 for dispatch-time errors.
    pub status: u16,
}

/// Per-cobotlet device online payload as sent by cobotlet.
#[derive(Debug, Deserialize)]
struct DeviceOnlinePayload {
    pub service_url: String,
    #[serde(default)]
    pub capabilities: serde_json::Value,
    #[serde(default = "Utc::now")]
    pub last_heartbeat: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatPayload {
    #[serde(default = "Utc::now")]
    pub ts: DateTime<Utc>,
}

/// Snapshot of a known cobotlet's online state.
#[derive(Debug, Clone, Serialize)]
pub struct CobotletStatus {
    pub alias: String,
    /// None when the cobotlet has never been seen online via MQTT.
    pub online: bool,
    pub last_seen: Option<chrono::DateTime<Utc>>,
}

/// Result of polling an async task.
pub enum TaskPollResult {
    /// Result is ready.
    Ready(TaskResult),
    /// Task was submitted and is still being processed.
    Pending,
    /// Unknown task ID (never submitted or already evicted beyond cache window).
    NotFound,
}

#[derive(Clone)]
pub struct MqttHandle {
    client: AsyncClient,
    topic_prefix: Arc<String>,
    /// Waiting oneshot channels for synchronous (wait=true) submissions.
    pending: Arc<DashMap<String, oneshot::Sender<TaskResult>>>,
    /// Tracks async (wait=false) submitted task IDs until result arrives or TTL.
    pending_async: Arc<DashMap<String, Instant>>,
    cache: Arc<DashMap<String, (TaskResult, Instant)>>,
    cache_ttl: Duration,
    /// Cobotlet alias → last-online timestamp (None = offline via LWT).
    cobotlets: Arc<DashMap<String, Option<DateTime<Utc>>>>,
}

impl MqttHandle {
    fn task_topic(&self, alias: &str, task_id: &str) -> String {
        format!("{}/cobotlets/{alias}/tasks/{task_id}", self.topic_prefix)
    }

    /// Submit a task and block until the result arrives or `timeout` elapses.
    pub async fn submit(&self, alias: &str, task: Task, timeout: Duration) -> Result<TaskResult> {
        let task_id = task.id.clone();
        let payload = serde_json::to_vec(&task).context("serialize task")?;
        let topic = self.task_topic(alias, &task_id);

        let (tx, rx) = oneshot::channel();
        self.pending.insert(task_id.clone(), tx);

        if let Err(e) = self
            .client
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
        {
            self.pending.remove(&task_id);
            return Err(anyhow!("publish task failed: {e}"));
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(anyhow!("task channel closed before result")),
            Err(_) => {
                self.pending.remove(&task_id);
                Err(anyhow!("timeout"))
            }
        }
    }

    /// Submit a task without waiting for a result. The task ID can be polled
    /// via [`poll_result`] to check completion.
    pub async fn submit_async(&self, alias: &str, task: Task) -> Result<()> {
        let task_id = task.id.clone();
        let payload = serde_json::to_vec(&task).context("serialize task")?;
        let topic = self.task_topic(alias, &task.id);
        self.pending_async.insert(task_id.clone(), Instant::now());
        if let Err(e) = self
            .client
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
        {
            self.pending_async.remove(&task_id);
            return Err(anyhow!("publish task failed: {e}"));
        }
        Ok(())
    }

    /// Returns a snapshot of all cobotlets seen via MQTT.
    pub fn cobotlet_status(&self) -> Vec<CobotletStatus> {
        self.cobotlets
            .iter()
            .map(|e| CobotletStatus {
                alias: e.key().clone(),
                online: e.value().is_some(),
                last_seen: *e.value(),
            })
            .collect()
    }

    /// Poll the state of any submitted task (sync or async).
    pub fn poll_result(&self, task_id: &str) -> TaskPollResult {
        if let Some(entry) = self.cache.get(task_id) {
            TaskPollResult::Ready(entry.value().0.clone())
        } else if self.pending.contains_key(task_id) || self.pending_async.contains_key(task_id) {
            TaskPollResult::Pending
        } else {
            TaskPollResult::NotFound
        }
    }
}

pub struct MqttRuntime {
    pub handle: Arc<MqttHandle>,
}

pub async fn spawn(
    cfg: &MqttConfig,
    db: Arc<DatabaseConnection>,
    cache_ttl_secs: u64,
) -> Result<MqttRuntime> {
    let url = url::Url::parse(&cfg.url).with_context(|| format!("parse mqtt url {}", cfg.url))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("mqtt url missing host"))?;
    let port = url.port().unwrap_or(1883);
    let mut opts = MqttOptions::new(cfg.client_id.clone(), host, port);
    opts.set_keep_alive(Duration::from_secs(cfg.keep_alive_secs.max(5)));
    if !url.username().is_empty() {
        opts.set_credentials(url.username(), url.password().unwrap_or_default());
    }

    let (client, mut eventloop) = AsyncClient::new(opts, PUBLISH_CHANNEL_CAPACITY);

    let device_sub = format!("{}/cobotlets/+/devices/+/+", cfg.topic_prefix);
    let results_sub = format!("{}/cobotlets/+/results/+", cfg.topic_prefix);
    let cobotlet_sub = format!("{}/cobotlets/+/online", cfg.topic_prefix);
    client
        .subscribe(&device_sub, QoS::AtLeastOnce)
        .await
        .with_context(|| format!("subscribe {device_sub}"))?;
    client
        .subscribe(&results_sub, QoS::AtLeastOnce)
        .await
        .with_context(|| format!("subscribe {results_sub}"))?;
    client
        .subscribe(&cobotlet_sub, QoS::AtLeastOnce)
        .await
        .with_context(|| format!("subscribe {cobotlet_sub}"))?;
    info!(
        device_sub,
        results_sub, cobotlet_sub, "cobotsrv mqtt subscribed"
    );

    let handle = Arc::new(MqttHandle {
        client,
        topic_prefix: Arc::new(cfg.topic_prefix.clone()),
        pending: Arc::new(DashMap::new()),
        pending_async: Arc::new(DashMap::new()),
        cache: Arc::new(DashMap::new()),
        cache_ttl: Duration::from_secs(cache_ttl_secs.max(10)),
        cobotlets: Arc::new(DashMap::new()),
    });

    let task_handle = handle.clone();
    let task_db = db;
    let topic_prefix = cfg.topic_prefix.clone();
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    handle_publish(publish, &topic_prefix, &task_db, &task_handle).await;
                }
                Ok(_) => {}
                Err(e) => {
                    error!(error = %e, "mqtt eventloop error; backing off");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    // Periodic cache + pending_async eviction.
    let evict_handle = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        tick.tick().await;
        loop {
            tick.tick().await;
            let now = Instant::now();
            let ttl = evict_handle.cache_ttl;
            evict_handle
                .cache
                .retain(|_, (_, t)| now.duration_since(*t) < ttl);
            evict_handle
                .pending_async
                .retain(|_, t| now.duration_since(*t) < ttl);
        }
    });

    Ok(MqttRuntime { handle })
}

enum Topic<'a> {
    DeviceEvent {
        alias: &'a str,
        device_id: &'a str,
        kind: &'a str,
    },
    Result {
        task_id: &'a str,
    },
    CobotletStatus {
        alias: &'a str,
    },
}

fn parse_topic<'a>(prefix: &str, topic: &'a str) -> Option<Topic<'a>> {
    let head = format!("{prefix}/cobotlets/");
    let rest = topic.strip_prefix(head.as_str())?;
    let mut parts = rest.split('/');
    let alias = parts.next()?;
    let kind_marker = parts.next()?;
    match kind_marker {
        "devices" => {
            let device_id = parts.next()?;
            let kind = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            Some(Topic::DeviceEvent {
                alias,
                device_id,
                kind,
            })
        }
        "results" => {
            let task_id = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            Some(Topic::Result { task_id })
        }
        "online" => {
            if parts.next().is_some() {
                return None;
            }
            Some(Topic::CobotletStatus { alias })
        }
        _ => None,
    }
}

async fn handle_publish(
    publish: Publish,
    topic_prefix: &str,
    db: &Arc<DatabaseConnection>,
    handle: &Arc<MqttHandle>,
) {
    let topic = publish.topic.clone();
    let Some(parsed) = parse_topic(topic_prefix, &topic) else {
        debug!(topic, "mqtt message ignored");
        return;
    };

    match parsed {
        Topic::DeviceEvent {
            alias,
            device_id,
            kind,
        } => {
            handle_device_event(db, alias, device_id, kind, &publish.payload).await;
        }
        Topic::Result { task_id } => {
            handle_task_result(handle, task_id, &publish.payload);
        }
        Topic::CobotletStatus { alias } => {
            handle_cobotlet_status(handle, alias, &publish.payload);
        }
    }
}

fn handle_cobotlet_status(handle: &Arc<MqttHandle>, alias: &str, payload: &[u8]) {
    if payload.is_empty() {
        // LWT or explicit offline clear — mark offline.
        handle.cobotlets.insert(alias.to_string(), None);
        info!(alias, "cobotlet offline");
    } else {
        handle.cobotlets.insert(alias.to_string(), Some(Utc::now()));
        info!(alias, "cobotlet online");
    }
}

async fn handle_device_event(
    db: &DatabaseConnection,
    alias: &str,
    device_id: &str,
    kind: &str,
    payload: &[u8],
) {
    match kind {
        "online" => {
            if payload.is_empty() {
                debug!(alias, device_id, "device online cleared");
                return;
            }
            let parsed: DeviceOnlinePayload = match serde_json::from_slice(payload) {
                Ok(p) => p,
                Err(e) => {
                    warn!(alias, device_id, error = %e, "decode online payload failed");
                    return;
                }
            };
            if let Err(e) = DeviceService::upsert(
                db,
                UpsertDevice {
                    cobotlet_alias: alias,
                    device_id,
                    service_url: parsed.service_url,
                    capabilities: parsed.capabilities,
                    state: "ready".to_string(),
                    last_heartbeat: parsed.last_heartbeat.naive_utc(),
                    registered_at: parsed.registered_at.naive_utc(),
                },
            )
            .await
            {
                warn!(alias, device_id, error = %e, "upsert device failed");
            } else {
                info!(alias, device_id, "device online");
            }
        }
        "offline" => {
            if let Err(e) = DeviceService::mark_offline(db, alias, device_id).await {
                warn!(alias, device_id, error = %e, "mark offline failed");
            } else {
                info!(alias, device_id, "device offline");
            }
        }
        "heartbeat" => {
            let ts = serde_json::from_slice::<HeartbeatPayload>(payload)
                .map(|p| p.ts)
                .unwrap_or_else(|_| Utc::now());
            if let Err(e) =
                DeviceService::update_heartbeat(db, alias, device_id, ts.naive_utc()).await
            {
                warn!(alias, device_id, error = %e, "heartbeat update failed");
            }
        }
        other => {
            debug!(kind = other, "unknown device event kind");
        }
    }
}

fn handle_task_result(handle: &Arc<MqttHandle>, task_id: &str, payload: &[u8]) {
    let result: TaskResult = match serde_json::from_slice(payload) {
        Ok(r) => r,
        Err(e) => {
            warn!(task_id, error = %e, "decode task result failed");
            return;
        }
    };
    handle.pending_async.remove(task_id);
    handle
        .cache
        .insert(task_id.to_string(), (result.clone(), Instant::now()));
    if let Some((_, tx)) = handle.pending.remove(task_id) {
        let _ = tx.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_event_topic() {
        let t = parse_topic("cobotrix", "cobotrix/cobotlets/c1/devices/d42/online").unwrap();
        match t {
            Topic::DeviceEvent {
                alias,
                device_id,
                kind,
            } => {
                assert_eq!(alias, "c1");
                assert_eq!(device_id, "d42");
                assert_eq!(kind, "online");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_result_topic() {
        let t = parse_topic("cobotrix", "cobotrix/cobotlets/c1/results/abc").unwrap();
        match t {
            Topic::Result { task_id } => assert_eq!(task_id, "abc"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_cobotlet_status_topic() {
        let t = parse_topic("cobotrix", "cobotrix/cobotlets/c1/online").unwrap();
        match t {
            Topic::CobotletStatus { alias } => assert_eq!(alias, "c1"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn rejects_extra_segments() {
        assert!(parse_topic("cobotrix", "cobotrix/cobotlets/c1/devices/d/online/x").is_none());
    }

    #[test]
    fn rejects_other_prefix() {
        assert!(parse_topic("cobotrix", "other/cobotlets/c1/results/abc").is_none());
    }
}

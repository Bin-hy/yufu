use std::collections::HashMap;
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

/// Mirror of `cobotlet::dispatcher::Task` (kept independent so cobotrix does
/// not depend on cobotlet at compile time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub capability_name: String,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
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

#[derive(Clone)]
pub struct MqttHandle {
    client: AsyncClient,
    topic_prefix: Arc<String>,
    pending: Arc<DashMap<String, oneshot::Sender<TaskResult>>>,
    cache: Arc<DashMap<String, (TaskResult, Instant)>>,
    cache_ttl: Duration,
}

impl MqttHandle {
    fn task_topic(&self, alias: &str, task_id: &str) -> String {
        format!("{}/cobotlets/{alias}/tasks/{task_id}", self.topic_prefix)
    }

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
                Err(anyhow!("task timed out after {:?}", timeout))
            }
        }
    }

    pub async fn submit_async(&self, alias: &str, task: Task) -> Result<()> {
        let payload = serde_json::to_vec(&task).context("serialize task")?;
        let topic = self.task_topic(alias, &task.id);
        self.client
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
            .map_err(|e| anyhow!("publish task failed: {e}"))?;
        Ok(())
    }

    pub fn take_result(&self, task_id: &str) -> Option<TaskResult> {
        self.cache.remove(task_id).map(|(_, (r, _))| r)
    }

    pub fn peek_result(&self, task_id: &str) -> Option<TaskResult> {
        self.cache.get(task_id).map(|e| e.value().0.clone())
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
    client
        .subscribe(&device_sub, QoS::AtLeastOnce)
        .await
        .with_context(|| format!("subscribe {device_sub}"))?;
    client
        .subscribe(&results_sub, QoS::AtLeastOnce)
        .await
        .with_context(|| format!("subscribe {results_sub}"))?;
    info!(device_sub, results_sub, "cobotrix mqtt subscribed");

    let handle = Arc::new(MqttHandle {
        client,
        topic_prefix: Arc::new(cfg.topic_prefix.clone()),
        pending: Arc::new(DashMap::new()),
        cache: Arc::new(DashMap::new()),
        cache_ttl: Duration::from_secs(cache_ttl_secs.max(10)),
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

    // Periodic cache eviction.
    let evict_handle = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        tick.tick().await;
        loop {
            tick.tick().await;
            let now = Instant::now();
            evict_handle
                .cache
                .retain(|_, (_, t)| now.duration_since(*t) < evict_handle.cache_ttl);
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
                    online: true,
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
    fn rejects_extra_segments() {
        assert!(parse_topic("cobotrix", "cobotrix/cobotlets/c1/devices/d/online/x").is_none());
    }

    #[test]
    fn rejects_other_prefix() {
        assert!(parse_topic("cobotrix", "other/cobotlets/c1/results/abc").is_none());
    }
}

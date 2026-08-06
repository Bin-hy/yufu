use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rumqttc::{AsyncClient, Event, Incoming, LastWill, MqttOptions, Packet, Publish, QoS};
use tracing::{debug, error, info, warn};

use crate::config::Mqtt as MqttConfig;
use crate::dispatcher::{Task, TaskResult, dispatch};
use crate::events::EventSink;
use crate::store::{Device, DeviceStore};

const PUBLISH_CHANNEL_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct MqttSink {
    client: AsyncClient,
    topic_root: Arc<String>,
}

impl MqttSink {
    fn topic(&self, suffix: &str) -> String {
        format!("{}/{}", self.topic_root, suffix)
    }

    fn spawn_publish(&self, topic: String, retain: bool, payload: Vec<u8>) {
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = client
                .publish(&topic, QoS::AtLeastOnce, retain, payload)
                .await
            {
                warn!(topic, error = %e, "mqtt publish failed");
            }
        });
    }
}

impl EventSink for MqttSink {
    fn publish_online(&self, device: &Device) {
        let payload = match serde_json::to_vec(device) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "serialize device online failed");
                return;
            }
        };
        let topic = self.topic(&format!("devices/{}/online", device.id));
        self.spawn_publish(topic, true, payload);
        // Clear any prior offline retained message.
        let off_topic = self.topic(&format!("devices/{}/offline", device.id));
        self.spawn_publish(off_topic, true, Vec::new());
    }

    fn publish_heartbeat(&self, device: &Device) {
        let payload = serde_json::json!({
            "ts": chrono::Utc::now(),
            // Carry the device state so cobotsrv can reflect maintenance /
            // busy / error / registering without waiting for a full re-register.
            "state": device.state.as_str(),
        })
        .to_string()
        .into_bytes();
        let topic = self.topic(&format!("devices/{}/heartbeat", device.id));
        self.spawn_publish(topic, false, payload);
    }

    fn publish_offline(&self, device: &Device) {
        let payload = match serde_json::to_vec(device) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "serialize device offline failed");
                return;
            }
        };
        let topic = self.topic(&format!("devices/{}/offline", device.id));
        self.spawn_publish(topic, true, payload);
        // Clear the retained online marker.
        let on_topic = self.topic(&format!("devices/{}/online", device.id));
        self.spawn_publish(on_topic, true, Vec::new());
    }

    fn publish_task_result(&self, result: &TaskResult) {
        let payload = match serde_json::to_vec(result) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "serialize task result failed");
                return;
            }
        };
        let topic = self.topic(&format!("results/{}", result.id));
        self.spawn_publish(topic, false, payload);
    }
}

pub struct MqttRuntime {
    pub sink: Arc<MqttSink>,
}

/// Connects to the MQTT broker, subscribes to the task topic, and spawns the
/// background event loop. The returned [`MqttSink`] can be used to publish
/// events; tasks are dispatched automatically as they arrive.
pub async fn spawn(
    cfg: &MqttConfig,
    store: Arc<DeviceStore>,
    task_store: Arc<crate::store::TaskStore>,
    http_client: reqwest::Client,
    request_timeout_ms: u64,
) -> Result<MqttRuntime> {
    let url = url::Url::parse(&cfg.url).with_context(|| format!("parse mqtt url {}", cfg.url))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("mqtt url missing host"))?;
    let port = url.port().unwrap_or(1883);
    let mut opts = MqttOptions::new(cfg.resolved_client_id(), host, port);
    opts.set_keep_alive(Duration::from_secs(cfg.keep_alive_secs.max(5)));
    if !url.username().is_empty() {
        opts.set_credentials(url.username(), url.password().unwrap_or_default());
    }

    let topic_root = cfg.topic_root();
    let lwt_topic = format!("{topic_root}/online");
    opts.set_last_will(LastWill::new(
        &lwt_topic,
        Vec::new(),
        QoS::AtLeastOnce,
        true,
    ));

    let (client, mut eventloop) = AsyncClient::new(opts, PUBLISH_CHANNEL_CAPACITY);

    // Mark cobotlet itself online (retained).
    client
        .publish(
            &lwt_topic,
            QoS::AtLeastOnce,
            true,
            serde_json::json!({ "alias": cfg.alias }).to_string(),
        )
        .await
        .context("publish cobotlet online marker")?;

    let task_subscription = format!("{topic_root}/tasks/+");
    client
        .subscribe(&task_subscription, QoS::AtLeastOnce)
        .await
        .with_context(|| format!("subscribe {task_subscription}"))?;
    info!(topic = task_subscription, "mqtt subscribed");

    let sink = Arc::new(MqttSink {
        client: client.clone(),
        topic_root: Arc::new(topic_root.clone()),
    });

    let task_sink = sink.clone();
    let task_store_inner = store;
    let task_record_store = task_store;
    let task_client = http_client;
    let task_topic_root = topic_root;
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    handle_publish(
                        publish,
                        &task_topic_root,
                        &task_store_inner,
                        &task_record_store,
                        &task_client,
                        request_timeout_ms,
                        &task_sink,
                    )
                    .await;
                }
                Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                    debug!(?ack, "mqtt connack");
                }
                Ok(_) => {}
                Err(e) => {
                    error!(error = %e, "mqtt eventloop error; backing off");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    Ok(MqttRuntime { sink })
}

async fn handle_publish(
    publish: Publish,
    topic_root: &str,
    store: &Arc<DeviceStore>,
    task_store: &Arc<crate::store::TaskStore>,
    http_client: &reqwest::Client,
    request_timeout_ms: u64,
    sink: &Arc<MqttSink>,
) {
    let topic = publish.topic.clone();
    let task_prefix = format!("{topic_root}/tasks/");
    let Some(task_id) = topic.strip_prefix(&task_prefix) else {
        debug!(topic, "mqtt message ignored");
        return;
    };
    let task_id = task_id.to_string();

    let task: Task = match serde_json::from_slice(&publish.payload) {
        Ok(t) => t,
        Err(e) => {
            warn!(topic, error = %e, "decode task payload failed");
            let result = TaskResult {
                id: task_id,
                is_error: true,
                content: vec![crate::dispatcher::ContentBlock {
                    r#type: "text".into(),
                    text: format!("decode task: {e}"),
                }],
                status: 0,
            };
            sink.publish_task_result(&result);
            return;
        }
    };

    if let Err(e) = task_store.enqueue(&task) {
        warn!(task_id = task.id, error = %e, "task enqueue failed");
    }
    let result = dispatch(store, task_store, http_client, request_timeout_ms, task).await;
    sink.publish_task_result(&result);
}

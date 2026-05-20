use std::sync::Arc;

use crate::dispatcher::TaskResult;
use crate::store::Device;

/// Sink for outbound events that get forwarded to cobotsrv (typically over MQTT).
/// In standalone mode (no MQTT configured) the [`NoopSink`] is used.
pub trait EventSink: Send + Sync {
    fn publish_online(&self, device: &Device);
    fn publish_heartbeat(&self, device_id: &str);
    fn publish_offline(&self, device: &Device);
    fn publish_task_result(&self, result: &TaskResult);
}

pub struct NoopSink;

impl EventSink for NoopSink {
    fn publish_online(&self, _device: &Device) {}
    fn publish_heartbeat(&self, _device_id: &str) {}
    fn publish_offline(&self, _device: &Device) {}
    fn publish_task_result(&self, _result: &TaskResult) {}
}

pub type SharedEventSink = Arc<dyn EventSink>;

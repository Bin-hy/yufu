use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::events::SharedEventSink;
use crate::store::{DeviceStore, TaskStore};

pub fn spawn_watchdog(
    store: Arc<DeviceStore>,
    task_store: Arc<TaskStore>,
    events: SharedEventSink,
    interval_secs: u64,
    timeout_secs: u64,
) {
    let interval = Duration::from_secs(interval_secs.max(1));
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        // Skip the immediate first tick so freshly-registered devices aren't
        // swept on startup.
        tick.tick().await;
        loop {
            tick.tick().await;
            let dropped = store.sweep_offline(timeout_secs);
            for device in dropped {
                info!(device_id = %device.id, "device heartbeat timeout, marking offline");
                // Fail any task still dispatched to this device so it doesn't
                // stay in-flight until the next process restart.
                if let Err(e) =
                    task_store.fail_inflight_for_device(&device.id, "device went offline")
                {
                    warn!(device_id = %device.id, error = %e, "failed to fail in-flight tasks");
                }
                events.publish_offline(&device);
            }
        }
    });
}

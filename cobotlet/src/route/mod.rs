use std::sync::Arc;

use crate::config::Config;
use crate::events::SharedEventSink;
use crate::store::{DeviceStore, TaskStore};

mod device;
mod hello;
mod task;
mod tool;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub store: Arc<DeviceStore>,
    pub task_store: Arc<TaskStore>,
    pub http_client: reqwest::Client,
    pub events: SharedEventSink,
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .merge(hello::route())
        .merge(device::route())
        .merge(task::route())
        .merge(tool::route())
}

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Method};
use serde::{Deserialize, Serialize};

use crate::store::{Capability, DeviceStore};

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    /// Pass-through HTTP headers for the device service call (implementation detail).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// MCP-style content block. Currently only "text" type is produced.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContentBlock {
    pub r#type: String,
    pub text: String,
}

impl ContentBlock {
    fn text(s: impl Into<String>) -> Self {
        Self {
            r#type: "text".into(),
            text: s.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskResult {
    pub id: String,
    /// MCP-style error flag: true when the tool call failed.
    pub is_error: bool,
    /// MCP-style content blocks carrying the response payload.
    pub content: Vec<ContentBlock>,
    /// HTTP status code from the device service. 0 for dispatch-time errors.
    pub status: u16,
}

impl TaskResult {
    fn err(id: String, msg: impl Into<String>) -> Self {
        Self {
            id,
            is_error: true,
            content: vec![ContentBlock::text(msg)],
            status: 0,
        }
    }

    /// Returns the first content block text if this result represents an error.
    pub fn error_message(&self) -> Option<&str> {
        if self.is_error {
            self.content.first().map(|c| c.text.as_str())
        } else {
            None
        }
    }
}

pub async fn dispatch(
    store: &Arc<DeviceStore>,
    http_client: &reqwest::Client,
    default_timeout_ms: u64,
    task: Task,
) -> TaskResult {
    let device = match &task.device_id {
        Some(id) => match store.get(id) {
            Some(d) if d.state.is_available() => d,
            Some(d) => {
                return TaskResult::err(task.id, format!("device {id} is {}", d.state.as_str()));
            }
            None => return TaskResult::err(task.id, format!("device {id} not found")),
        },
        None => match store.find_by_capability(&task.name).into_iter().next() {
            Some(d) => d,
            None => {
                return TaskResult::err(
                    task.id,
                    format!("no online device for capability '{}'", task.name),
                );
            }
        },
    };

    let Some(cap) = device.capabilities.iter().find(|c| c.name == task.name) else {
        return TaskResult::err(
            task.id,
            format!(
                "device '{}' does not declare capability '{}'",
                device.id, task.name
            ),
        );
    };

    if store.acquire_for_task(&device.id, &task.id).is_err() {
        return TaskResult::err(task.id, format!("device '{}' not available", device.id));
    }

    let result = forward(
        http_client,
        &device.service_url,
        cap,
        &task,
        default_timeout_ms,
    )
    .await;

    if result.status >= 500 || (result.is_error && result.status == 0) {
        let reason = result
            .error_message()
            .unwrap_or("unknown error")
            .to_string();
        let _ = store.mark_error(&device.id, reason);
    } else {
        let _ = store.release_from_task(&device.id);
    }

    result
}

async fn forward(
    http_client: &reqwest::Client,
    base_url: &str,
    capability: &Capability,
    task: &Task,
    default_timeout_ms: u64,
) -> TaskResult {
    let method = match Method::from_bytes(capability.method.as_bytes()) {
        Ok(m) => m,
        Err(e) => return TaskResult::err(task.id.clone(), format!("invalid method: {e}")),
    };

    let url = format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        if capability.path.starts_with('/') {
            capability.path.clone()
        } else {
            format!("/{}", capability.path)
        }
    );

    let mut headers = HeaderMap::new();
    for (k, v) in &task.headers {
        match (HeaderName::try_from(k.as_str()), HeaderValue::try_from(v)) {
            (Ok(name), Ok(value)) => {
                headers.insert(name, value);
            }
            _ => {
                return TaskResult::err(task.id.clone(), format!("invalid header '{k}'"));
            }
        }
    }

    let timeout = Duration::from_millis(task.timeout_ms.unwrap_or(default_timeout_ms));

    let mut req = http_client
        .request(method, &url)
        .headers(headers)
        .timeout(timeout);
    if let Some(input) = &task.input {
        req = req.json(input);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content = match resp.bytes().await {
                Ok(bytes) if bytes.is_empty() => vec![],
                Ok(bytes) => {
                    let text = match serde_json::from_slice::<serde_json::Value>(&bytes) {
                        Ok(v) => v.to_string(),
                        Err(_) => String::from_utf8_lossy(&bytes).into_owned(),
                    };
                    vec![ContentBlock::text(text)]
                }
                Err(e) => {
                    return TaskResult::err(task.id.clone(), format!("read body failed: {e}"));
                }
            };
            TaskResult {
                id: task.id.clone(),
                is_error: status >= 400,
                content,
                status,
            }
        }
        Err(e) => TaskResult::err(task.id.clone(), format!("forward failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Capability, Device, DeviceStore};
    use chrono::Utc;

    fn store() -> (Arc<DeviceStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cobotlet.redb");
        (Arc::new(DeviceStore::open(path).unwrap()), dir)
    }

    fn task(cap: &str) -> Task {
        Task {
            id: "t1".into(),
            name: cap.into(),
            device_id: None,
            headers: HashMap::new(),
            input: None,
            timeout_ms: Some(500),
        }
    }

    fn cap(name: &str) -> Capability {
        Capability {
            name: name.into(),
            description: None,
            method: "POST".into(),
            path: format!("/{name}"),
            tags: vec![],
            input_schema: None,
        }
    }

    #[tokio::test]
    async fn no_device_for_capability() {
        let (s, _g) = store();
        let client = reqwest::Client::new();
        let r = dispatch(&s, &client, 5000, task("move")).await;
        assert!(r.is_error);
        assert!(r.error_message().unwrap().contains("no online device"));
    }

    #[tokio::test]
    async fn device_id_not_found() {
        let (s, _g) = store();
        let client = reqwest::Client::new();
        let mut t = task("move");
        t.device_id = Some("missing".into());
        let r = dispatch(&s, &client, 5000, t).await;
        assert!(r.is_error);
        assert!(r.error_message().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn offline_device_rejected() {
        let (s, _g) = store();
        let now = Utc::now();
        let d = Device {
            id: "d1".into(),
            service_url: "http://127.0.0.1:1".into(),
            capabilities: vec![cap("move")],
            last_heartbeat: now,
            registered_at: now,
            state: crate::store::DeviceState::Offline,
        };
        s.register(d).unwrap();
        let client = reqwest::Client::new();
        let mut t = task("move");
        t.device_id = Some("d1".into());
        let r = dispatch(&s, &client, 5000, t).await;
        assert!(r.is_error);
        assert!(r.error_message().unwrap().contains("offline"));
    }

    #[tokio::test]
    async fn forward_failure_surfaces_in_result() {
        let (s, _g) = store();
        let now = Utc::now();
        s.register(Device {
            id: "d1".into(),
            service_url: "http://127.0.0.1:1".into(),
            capabilities: vec![cap("move")],
            last_heartbeat: now,
            registered_at: now,
            state: crate::store::DeviceState::Ready,
        })
        .unwrap();
        // Need to heartbeat to make it Ready (register forces Registering)
        s.heartbeat("d1").unwrap();
        let client = reqwest::Client::new();
        let r = dispatch(&s, &client, 200, task("move")).await;
        assert!(r.is_error);
        assert!(r.error_message().is_some());
        assert_eq!(r.id, "t1");
    }
}

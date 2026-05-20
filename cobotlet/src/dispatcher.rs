use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Method};
use serde::{Deserialize, Serialize};

use crate::store::{Capability, DeviceStore};

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskResult {
    pub id: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

impl TaskResult {
    fn err(id: String, msg: impl Into<String>) -> Self {
        Self {
            id,
            status: 0,
            headers: HashMap::new(),
            body: None,
            error: Some(msg.into()),
        }
    }
}

pub async fn dispatch(
    store: &Arc<DeviceStore>,
    http_client: &reqwest::Client,
    default_timeout_ms: u64,
    task: Task,
) -> TaskResult {
    let candidates = match &task.device_id {
        Some(id) => match store.get(id) {
            Some(d) if d.online => vec![d],
            Some(_) => return TaskResult::err(task.id, format!("device {id} is offline")),
            None => return TaskResult::err(task.id, format!("device {id} not found")),
        },
        None => store.find_by_capability(&task.capability_name),
    };

    let Some(device) = candidates.into_iter().next() else {
        return TaskResult::err(
            task.id,
            format!("no online device for capability {}", task.capability_name),
        );
    };

    let Some(cap) = device
        .capabilities
        .iter()
        .find(|c| c.name == task.capability_name)
    else {
        return TaskResult::err(
            task.id,
            format!(
                "device {} does not declare capability {}",
                device.id, task.capability_name
            ),
        );
    };

    forward(
        http_client,
        &device.service_url,
        cap,
        &task,
        default_timeout_ms,
    )
    .await
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
                return TaskResult::err(task.id.clone(), format!("invalid header {k}"));
            }
        }
    }

    let timeout = Duration::from_millis(task.timeout_ms.unwrap_or(default_timeout_ms));

    let mut req = http_client
        .request(method, &url)
        .headers(headers)
        .timeout(timeout);
    if let Some(body) = &task.body {
        req = req.json(body);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let mut out_headers = HashMap::new();
            for (name, value) in resp.headers() {
                if let Ok(s) = value.to_str() {
                    out_headers.insert(name.to_string(), s.to_string());
                }
            }
            let body = match resp.bytes().await {
                Ok(bytes) if bytes.is_empty() => None,
                Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(v) => Some(v),
                    Err(_) => Some(serde_json::Value::String(
                        String::from_utf8_lossy(&bytes).into_owned(),
                    )),
                },
                Err(e) => {
                    return TaskResult::err(task.id.clone(), format!("read body failed: {e}"));
                }
            };
            TaskResult {
                id: task.id.clone(),
                status,
                headers: out_headers,
                body,
                error: None,
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
            capability_name: cap.into(),
            device_id: None,
            headers: HashMap::new(),
            body: None,
            timeout_ms: Some(500),
        }
    }

    #[tokio::test]
    async fn no_device_for_capability() {
        let (s, _g) = store();
        let client = reqwest::Client::new();
        let r = dispatch(&s, &client, 5000, task("move")).await;
        assert!(r.error.unwrap().contains("no online device"));
    }

    #[tokio::test]
    async fn device_id_not_found() {
        let (s, _g) = store();
        let client = reqwest::Client::new();
        let mut t = task("move");
        t.device_id = Some("missing".into());
        let r = dispatch(&s, &client, 5000, t).await;
        assert!(r.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn offline_device_rejected() {
        let (s, _g) = store();
        let now = Utc::now();
        let mut d = Device {
            id: "d1".into(),
            service_url: "http://127.0.0.1:1".into(),
            capabilities: vec![Capability {
                name: "move".into(),
                method: "POST".into(),
                path: "/move".into(),
                tags: vec![],
                schema: None,
            }],
            last_heartbeat: now,
            registered_at: now,
            online: false,
        };
        d.online = false;
        s.register(d).unwrap();
        let client = reqwest::Client::new();
        let mut t = task("move");
        t.device_id = Some("d1".into());
        let r = dispatch(&s, &client, 5000, t).await;
        assert!(r.error.unwrap().contains("offline"));
    }

    #[tokio::test]
    async fn forward_failure_surfaces_in_result() {
        // No device service bound on port 1; the request will fail to connect.
        let (s, _g) = store();
        let now = Utc::now();
        s.register(Device {
            id: "d1".into(),
            service_url: "http://127.0.0.1:1".into(),
            capabilities: vec![Capability {
                name: "move".into(),
                method: "POST".into(),
                path: "/move".into(),
                tags: vec![],
                schema: None,
            }],
            last_heartbeat: now,
            registered_at: now,
            online: true,
        })
        .unwrap();
        let client = reqwest::Client::new();
        let r = dispatch(&s, &client, 200, task("move")).await;
        assert!(r.error.is_some());
        assert_eq!(r.id, "t1");
    }
}

use std::collections::HashMap;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::route::AppState;

pub fn route() -> Router<AppState> {
    Router::new().route("/tools", get(list_tools))
}

#[derive(Debug, Serialize)]
pub struct ToolProvider {
    pub device_id: String,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    pub providers: Vec<ToolProvider>,
}

#[derive(Debug, Serialize)]
pub struct ToolListResponse {
    pub tools: Vec<ToolDescriptor>,
}

pub(crate) async fn list_tools(State(state): State<AppState>) -> Json<ToolListResponse> {
    // Group all devices (online or not) by the capabilities they declare.
    let mut seen: HashMap<String, ToolDescriptor> = HashMap::new();
    for device in state.store.list() {
        for cap in &device.capabilities {
            let entry = seen
                .entry(cap.name.clone())
                .or_insert_with(|| ToolDescriptor {
                    name: cap.name.clone(),
                    description: cap.description.clone(),
                    input_schema: cap.input_schema.clone(),
                    providers: Vec::new(),
                });
            entry.providers.push(ToolProvider {
                device_id: device.id.clone(),
                state: device.state.as_str().to_string(),
            });
        }
    }
    let mut tools: Vec<ToolDescriptor> = seen.into_values().collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    Json(ToolListResponse { tools })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use chrono::Utc;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::events::NoopSink;
    use crate::route::AppState;
    use crate::store::{Capability, Device, DeviceStore};

    fn make_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(redb::Database::create(dir.path().join("test.redb")).unwrap());
        let store = Arc::new(DeviceStore::new(db.clone()).unwrap());
        let task_store = Arc::new(crate::store::TaskStore::new(db));
        let state = AppState {
            config: crate::config::Config::default(),
            store,
            task_store,
            http_client: reqwest::Client::new(),
            events: Arc::new(NoopSink),
        };
        (state, dir)
    }

    fn device(id: &str, caps: &[&str], online: bool) -> Device {
        let now = Utc::now();
        Device {
            id: id.to_string(),
            service_url: format!("http://device-{id}:9000"),
            capabilities: caps
                .iter()
                .map(|name| Capability {
                    name: name.to_string(),
                    description: Some(format!("desc-{name}")),
                    method: "POST".to_string(),
                    path: format!("/{name}"),
                    tags: vec![],
                    input_schema: None,
                })
                .collect(),
            last_heartbeat: now,
            registered_at: now,
            state: if online {
                crate::store::DeviceState::Ready
            } else {
                crate::store::DeviceState::Offline
            },
            control_preset: None,
        }
    }

    async fn call(app: Router, path: &str) -> (StatusCode, serde_json::Value) {
        let req = Request::builder().uri(path).body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn list_tools_empty_store() {
        let (state, _dir) = make_state();
        let app = Router::new().merge(route()).with_state(state);
        let (status, json) = call(app, "/tools").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["tools"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_tools_shows_all_devices_including_offline() {
        let (state, _dir) = make_state();
        state
            .store
            .register(device("d-online", &["move"], true))
            .unwrap();
        state
            .store
            .register(device("d-offline", &["grip"], false))
            .unwrap();

        let app = Router::new().merge(route()).with_state(state);
        let (status, json) = call(app, "/tools").await;
        assert_eq!(status, StatusCode::OK);

        let tools = json["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"move"), "online device tool must appear");
        assert!(
            names.contains(&"grip"),
            "offline device tool must also appear"
        );
    }

    #[tokio::test]
    async fn list_tools_aggregates_by_name_and_lists_providers() {
        let (state, _dir) = make_state();
        // Two devices both offer "move".
        state.store.register(device("d1", &["move"], true)).unwrap();
        state
            .store
            .register(device("d2", &["move", "grip"], false))
            .unwrap();

        let app = Router::new().merge(route()).with_state(state);
        let (status, json) = call(app, "/tools").await;
        assert_eq!(status, StatusCode::OK);

        let tools = json["tools"].as_array().unwrap();
        // "move" and "grip" only (no duplicate "move" tool).
        assert_eq!(tools.len(), 2);

        let move_tool = tools.iter().find(|t| t["name"] == "move").unwrap();
        let providers = move_tool["providers"].as_array().unwrap();
        assert_eq!(providers.len(), 2, "both devices must appear as providers");

        let states: Vec<&str> = providers
            .iter()
            .map(|p| p["state"].as_str().unwrap())
            .collect();
        assert!(states.contains(&"ready"));
        assert!(states.contains(&"offline"));
    }

    #[tokio::test]
    async fn list_tools_sorted_alphabetically() {
        let (state, _dir) = make_state();
        state
            .store
            .register(device("d1", &["zoom", "auto", "move"], true))
            .unwrap();

        let app = Router::new().merge(route()).with_state(state);
        let (_, json) = call(app, "/tools").await;
        let names: Vec<&str> = json["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}

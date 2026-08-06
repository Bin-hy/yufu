use std::future::Future;
use std::sync::Arc;

use axum::response::IntoResponse;
use http::{StatusCode, Uri};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

pub mod config;
pub mod dispatcher;
pub mod error;
pub mod events;
pub mod heartbeat;
pub mod mqtt;
pub mod result;
pub mod route;
pub mod store;

#[cfg(feature = "webui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../assets/cobotlet/"]
struct Assets;

use crate::events::{EventSink, NoopSink};
use crate::route::AppState;

pub async fn serve(
    cfg: config::Config,
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) {
    let storage_path = cfg
        .storage
        .as_ref()
        .map(|s| s.path.clone())
        .unwrap_or_else(|| String::from("data/cobotlet.redb"));

    let db = store::open_database(&storage_path)
        .unwrap_or_else(|e| panic!("failed to open database at {storage_path}: {e}"));
    let db = Arc::new(db);

    let store = Arc::new(
        store::DeviceStore::new(db.clone())
            .unwrap_or_else(|e| panic!("failed to init device store: {e}")),
    );
    let task_store = Arc::new(store::TaskStore::new(db));
    info!(path = storage_path, "database opened");

    let recovered = task_store.recover().unwrap_or_default();
    if !recovered.is_empty() {
        info!(
            count = recovered.len(),
            "recovered inflight tasks as failed"
        );
    }

    let http_client = reqwest::Client::new();

    let events: events::SharedEventSink = match &cfg.mqtt {
        Some(mqtt_cfg) => {
            match mqtt::spawn(
                mqtt_cfg,
                store.clone(),
                task_store.clone(),
                http_client.clone(),
                cfg.device.request_timeout_ms,
            )
            .await
            {
                Ok(rt) => {
                    info!(alias = mqtt_cfg.alias, "mqtt connected");
                    republish_loaded_online(store.clone(), rt.sink.as_ref());
                    rt.sink
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mqtt connect failed, running standalone");
                    Arc::new(NoopSink)
                }
            }
        }
        None => Arc::new(NoopSink),
    };

    if cfg.mqtt.is_some() {
        heartbeat::spawn_watchdog(
            store.clone(),
            events.clone(),
            cfg.device.watchdog_interval_secs,
            cfg.device.heartbeat_timeout_secs,
        );
    }

    let state = AppState {
        config: cfg.clone(),
        store,
        task_store,
        http_client,
        events,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = route::routes()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
        .fallback(static_handler);

    info!("cobotlet serving");

    #[cfg(feature = "easytier")]
    let easytier_stop: Option<cobotnet::StopHandle> = {
        if let Some(mut c) = cfg.easytier.clone() {
            c.validate();

            let maybe_config = if c.should_fetch_remote() {
                fetch_easytier_config(&c).await
            } else if !c.network_name.is_empty() {
                tracing::info!("using local easytier config");
                Some(c.into_cobotnet_config())
            } else {
                tracing::info!(
                    "easytier: no network_name configured and no server_url, skipping P2P node"
                );
                None
            };

            maybe_config.and_then(start_easytier_node)
        } else {
            None
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .unwrap_or_else(|e| tracing::error!(error = %e, "server error"));

    #[cfg(feature = "easytier")]
    if let Some(handle) = easytier_stop {
        tracing::info!("stopping easytier node");
        handle.stop();
    }
}

// ── easytier helpers ──────────────────────────────────────────────────

#[cfg(feature = "easytier")]
async fn fetch_easytier_config(c: &crate::config::EasytierConfig) -> Option<cobotnet::Config> {
    let server_url = c.server_url.as_deref().unwrap_or("");
    let robot_sn = c.robot_sn.as_deref().unwrap_or(c.hostname.as_str());
    let register_token = c.register_token.as_deref().unwrap_or("");

    let url = format!("{}?robot_sn={}", server_url.trim_end_matches('/'), robot_sn);

    tracing::info!(
        server_url = server_url,
        robot_sn = robot_sn,
        "fetching easytier config from cobotsrv"
    );

    let resp = match reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", register_token))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("failed to fetch easytier config from cobotsrv: {:?}", e);
            return None;
        }
    };

    if !resp.status().is_success() {
        tracing::warn!(
            "cobotsrv returned {} for easytier config fetch",
            resp.status()
        );
        return None;
    }

    match resp.json::<crate::config::EasytierConfig>().await {
        Ok(mut remote_cfg) => {
            remote_cfg.validate();
            tracing::info!(
                "fetched easytier config: network_name={}, ipv4={}",
                remote_cfg.network_name,
                remote_cfg.ipv4
            );
            Some(remote_cfg.into_cobotnet_config())
        }
        Err(e) => {
            tracing::warn!("failed to parse easytier config from cobotsrv: {:?}", e);
            None
        }
    }
}

#[cfg(feature = "easytier")]
fn start_easytier_node(cfg: cobotnet::Config) -> Option<cobotnet::StopHandle> {
    let node = cobotnet::EasytierNode::new(cfg);
    match node.start() {
        Ok((stop_handle, mut receiver)) => {
            tracing::info!("easytier node started on cobotlet");
            std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(async move {
                        while let Some(event) = receiver.recv().await {
                            match event {
                                cobotnet::PeerEvent::Online(peer) => {
                                    tracing::info!(
                                        "easytier peer online: {} at {}",
                                        peer.name,
                                        peer.virtual_ip
                                    );
                                }
                                cobotnet::PeerEvent::Offline(name) => {
                                    tracing::info!("easytier peer offline: {}", name);
                                }
                            }
                        }
                        tracing::warn!("easytier peer channel closed");
                    });
            });
            Some(stop_handle)
        }
        Err(e) => {
            tracing::error!("failed to start easytier node on cobotlet: {:?}", e);
            None
        }
    }
}

#[cfg(feature = "webui")]
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    let file_path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(file_path) {
        Some(content) => {
            let mime = mime_guess::from_path(file_path).first_or_octet_stream();
            ([(http::header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => match Assets::get("index.html") {
            Some(index) => {
                let mime = mime_guess::from_path("index.html").first_or_octet_stream();
                ([(http::header::CONTENT_TYPE, mime.as_ref())], index.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "not found").into_response(),
        },
    }
}

#[cfg(not(feature = "webui"))]
async fn static_handler(_: Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "feature webui not enable")
}

/// Re-broadcast `online` for devices loaded from disk (persisted across
/// restarts) whose state is `Registering` or `Ready`. When this cobotlet
/// restarts, devices that the local watchdog already flipped to `Offline`
/// have had their retained `online` marker cleared (`publish_offline`), so
/// without this a restart would leave cobotsrv showing them offline until
/// their next heartbeat. `Registering` devices are included for parity with
/// the register path, which also announces them as `ready` immediately.
/// `Offline`/`Busy`/`Error`/`Maintenance` devices are left alone — they
/// still need a heartbeat to come back.
fn republish_loaded_online(store: Arc<crate::store::DeviceStore>, sink: &dyn EventSink) {
    for device in store.list() {
        if matches!(
            device.state,
            crate::store::DeviceState::Registering | crate::store::DeviceState::Ready
        ) {
            sink.publish_online(&device);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::store::{Device, DeviceState, DeviceStore};

    #[derive(Clone)]
    struct CountingSink(Arc<AtomicUsize>);

    impl EventSink for CountingSink {
        fn publish_online(&self, _device: &Device) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn publish_heartbeat(&self, _device: &Device) {}
        fn publish_offline(&self, _device: &Device) {}
        fn publish_task_result(&self, _result: &crate::dispatcher::TaskResult) {}
    }

    fn device(id: &str, state: DeviceState) -> Device {
        Device {
            id: id.to_string(),
            service_url: "http://127.0.0.1:9900".into(),
            capabilities: vec![],
            last_heartbeat: chrono::Utc::now(),
            registered_at: chrono::Utc::now(),
            state,
            control_preset: None,
        }
    }

    #[test]
    fn republish_only_ready_or_registering() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            DeviceStore::new(Arc::new(
                store::open_database(dir.path().join("t.redb")).unwrap(),
            ))
            .unwrap(),
        );
        store.register(device("ready", DeviceState::Ready)).unwrap();
        store
            .register(device("reg", DeviceState::Registering))
            .unwrap();
        store.register(device("off", DeviceState::Offline)).unwrap();
        store
            .register(device(
                "busy",
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
            ))
            .unwrap();

        let count = Arc::new(AtomicUsize::new(0));
        let sink = CountingSink(count.clone());
        republish_loaded_online(store, &sink);

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}

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

use crate::events::NoopSink;
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

    let store = Arc::new(
        store::DeviceStore::open(&storage_path)
            .unwrap_or_else(|e| panic!("failed to open device store at {storage_path}: {e}")),
    );
    info!(path = storage_path, "device store opened");

    let http_client = reqwest::Client::new();

    let events: events::SharedEventSink = match &cfg.mqtt {
        Some(mqtt_cfg) => {
            match mqtt::spawn(
                mqtt_cfg,
                store.clone(),
                http_client.clone(),
                cfg.device.request_timeout_ms,
            )
            .await
            {
                Ok(rt) => {
                    info!(alias = mqtt_cfg.alias, "mqtt connected");
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
        config: cfg,
        store,
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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .unwrap_or_else(|e| tracing::error!(error = %e, "server error"));
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

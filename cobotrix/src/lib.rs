use auth::AuthState;
use auth::token_manager::TokenManager;
use axum::Router;
use axum::extract::Request;
use axum::middleware;
use axum::response::IntoResponse;
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use http::{HeaderValue, Method, StatusCode, Uri};
use reqwest::Client;
use sea_orm_migration::MigratorTrait;
use std::sync::Arc;
use std::{future::Future, time::Duration};
use tokio::net::TcpListener;

use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, info_span};

use crate::config::Config;
use crate::route::{get_protected_routes, get_public_routes};
use crate::service::database::DatabaseService;
use crate::store::{Node, NodeKind, Storage};

#[cfg(feature = "webui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../assets/cobotrix/"]
struct Assets;

pub mod config;
pub mod entity;
pub mod error;
mod migration;
pub mod mqtt;
mod result;
pub mod route;
pub mod service;
mod store;

pub async fn serve<F>(cfg: Config, listener: TcpListener, signal: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    info!(
        "CoBotrix management center listening on {}",
        listener.local_addr().unwrap()
    );
    let client_req = Client::builder();
    let client_mem = Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_millis(1000));

    #[cfg(feature = "net4mqtt")]
    let (client_req, client_mem) = if let Some(proxy) = match cfg.net4mqtt.clone() {
        Some(c) => {
            let target = reqwest::Url::parse(&format!("socks5h://{}", c.listen)).unwrap();
            Some(reqwest::Proxy::custom(move |url| match url.host_str() {
                Some(host) => {
                    if host.ends_with(c.domain.as_str()) {
                        Some(target.clone())
                    } else {
                        None
                    }
                }
                None => None,
            }))
        }
        None => None,
    } {
        info!("net4mqtt proxy: {:?}", proxy);
        (client_req.proxy(proxy.clone()), client_mem.proxy(proxy))
    } else {
        (client_req, client_mem)
    };

    let store = Storage::new(client_mem.build().unwrap());
    let nodes = store.get_map_nodes_mut();
    for v in cfg.nodes.clone() {
        nodes
            .write()
            .unwrap()
            .insert(v.alias, Node::new(v.token, NodeKind::Static, v.url));
    }

    // Initialize database connection
    let database_service = DatabaseService::new(&cfg.database)
        .await
        .expect("Failed to create database service");
    migration::Migrator::up(&database_service.connection, None)
        .await
        .unwrap();

    #[cfg(feature = "net4mqtt")]
    {
        if let Some(mut c) = cfg.net4mqtt.clone() {
            c.validate();
            let (sender, mut receiver) =
                tokio::sync::mpsc::channel::<(String, String, Vec<u8>)>(10);

            let domain = c.domain.clone();

            std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(async move {
                        loop {
                            let listener = TcpListener::bind(c.listen).await.unwrap();
                            match net4mqtt::proxy::local_socks(
                                &c.mqtt_url,
                                listener,
                                ("-", &c.alias.clone()),
                                Some(c.domain.clone()),
                                Some(net4mqtt::proxy::VDataConfig {
                                    receiver: Some(sender.clone()),
                                    ..Default::default()
                                }),
                                false,
                            )
                            .await
                            {
                                Ok(_) => tracing::warn!(
                                    "net4mqtt service is end, restart net4mqtt service"
                                ),
                                Err(e) => error!("mqtt4mqtt error: {:?}", e),
                            }
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    });
            });

            std::thread::spawn(move || {
                let dns = net4mqtt::kxdns::Kxdns::new(domain);
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(async move {
                        loop {
                            match receiver.recv().await {
                                Some((agent_id, _local_id, data)) => {
                                    if data.len() > 5 {
                                        nodes.write().unwrap().insert(
                                            agent_id.clone(),
                                            Node::new(
                                                "".to_string(),
                                                NodeKind::Net4mqtt,
                                                format!("http://{}", dns.registry(&agent_id)),
                                            ),
                                        );
                                    } else {
                                        nodes.write().unwrap().remove(&agent_id);
                                    }
                                }
                                None => {
                                    error!("net4mqtt discovery receiver channel closed");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                }
                            }
                        }
                    })
            });
        }
    }

    let database = Arc::new(database_service);

    let mqtt_handle = if let Some(mqtt_cfg) = cfg.mqtt.clone() {
        let conn = Arc::new(database.connection.clone());
        match mqtt::spawn(&mqtt_cfg, conn, cfg.task.result_cache_ttl_secs).await {
            Ok(rt) => {
                info!(url = %mqtt_cfg.url, "cobotrix mqtt connected");
                Some(rt.handle)
            }
            Err(e) => {
                tracing::warn!(error = %e, "cobotrix mqtt connect failed; tasks API will return errors");
                None
            }
        }
    } else {
        None
    };

    let app_state = AppState {
        config: cfg.clone(),
        client: client_req.build().unwrap(),
        storage: store,
        database: database.clone(),
        mqtt: mqtt_handle,
    };

    // JWT verification — same HS256 secret as cobotsrv (cfg.auth).
    // Phase 1 will swap this for RS256 + JWKS pulled from cobotsrv.
    let auth_state = AuthState {
        token_manager: TokenManager::new(&cfg.auth.secret, cfg.auth.exp_seconds),
        // refresh tokens are issued/verified by cobotsrv only; cobotrix never sees them
        refresh_token_manager: TokenManager::new(&cfg.auth.secret, cfg.auth.exp_seconds),
    };

    let protected = get_protected_routes().layer(middleware::from_fn_with_state(
        auth_state.clone(),
        auth::access_middleware::<IdType>,
    ));

    let app = Router::new()
        .merge(protected)
        .merge(get_public_routes())
        .layer({
            let origins: Vec<String> = vec!["*".to_string()]; // internal, trust proxy
            let has_wildcard = origins.iter().any(|o| o == "*");
            let cors_layer = CorsLayer::new()
                .allow_credentials(true)
                .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
                .allow_methods([
                    Method::GET,
                    Method::PATCH,
                    Method::POST,
                    Method::DELETE,
                    Method::OPTIONS,
                ]);

            if has_wildcard {
                cors_layer.allow_origin(tower_http::cors::AllowOrigin::mirror_request())
            } else {
                cors_layer.allow_origin(tower_http::cors::AllowOrigin::predicate(
                    move |origin: &HeaderValue, _request_head: &http::request::Parts| {
                        if let Ok(origin_str) = origin.to_str() {
                            origins.contains(&origin_str.to_string())
                        } else {
                            false
                        }
                    },
                ))
            }
        })
        .with_state(app_state.clone())
        .layer(axum::middleware::from_fn(http_log::print_request_response))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let span = info_span!(
                    "http_request",
                    uri = ?request.uri(),
                    method = ?request.method(),
                    span_id = tracing::field::Empty,
                    target_addr = tracing::field::Empty,
                );
                span.record(
                    "span_id",
                    span.id().unwrap_or(tracing::Id::from_u64(42)).into_u64(),
                );
                span
            }),
        )
        .fallback(static_handler);

    axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await
        .unwrap_or_else(|e| error!("Application error: {e}"));
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

#[derive(Clone)]
pub struct AppState {
    config: Config,
    client: Client,
    storage: Storage,
    database: Arc<DatabaseService>,
    pub mqtt: Option<Arc<crate::mqtt::MqttHandle>>,
}

pub type IdType = i32;

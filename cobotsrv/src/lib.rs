use auth::access_middleware_rs256;
use auth::jwks::JwksClient;
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
#[folder = "../assets/cobotsrv/"]
struct Assets;

pub mod config;
pub mod enforce;
pub mod entity;
pub mod error;
pub mod mcp;
mod migration;
pub mod mqtt;
mod result;
pub mod route;
pub mod service;
mod store;

/// Capacity of the MCP audit-record queue feeding the background writer task.
const MCP_AUDIT_QUEUE_CAPACITY: usize = 1024;
/// MCP audit records older than this are purged (once per day).
const MCP_AUDIT_RETENTION_DAYS: i64 = 30;
/// TTL of the MCP token-revocation introspect cache, in seconds.
const MCP_REVOCATION_CACHE_TTL_SECS: u64 = 10;

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

    #[cfg(feature = "easytier")]
    let mut stop_handle_storage: Option<cobotnet::StopHandle> = None;
    #[cfg(feature = "easytier")]
    {
        if let Some(c) = cfg.easytier.clone() {
            let mut cobotnet_cfg = c.into_cobotnet_config();
            cobotnet_cfg.validate();

            let node = cobotnet::EasytierNode::new(cobotnet_cfg);
            match node.start() {
                Ok((stop_handle, mut receiver)) => {
                    stop_handle_storage = Some(stop_handle);
                    info!("easytier node started");

                    let store_nodes = store.get_map_nodes_mut();
                    std::thread::spawn(move || {
                        tokio::runtime::Runtime::new()
                            .unwrap()
                            .block_on(async move {
                                while let Some(event) = receiver.recv().await {
                                    match event {
                                        cobotnet::PeerEvent::Online(peer) => {
                                            let url = format!("http://{}", peer.virtual_ip);
                                            store_nodes.write().unwrap().insert(
                                                peer.name.clone(),
                                                Node::new(String::new(), NodeKind::Easytier, url),
                                            );
                                            info!(
                                                "easytier peer online: {} at {}",
                                                peer.name, peer.virtual_ip
                                            );
                                        }
                                        cobotnet::PeerEvent::Offline(name) => {
                                            store_nodes.write().unwrap().remove(&name);
                                            info!("easytier peer offline: {}", name);
                                        }
                                    }
                                }
                                tracing::warn!("easytier peer discovery channel closed");
                            });
                    });
                }
                Err(e) => {
                    error!("failed to start easytier node: {:?}", e);
                }
            }
        }
    }

    let database = Arc::new(database_service);

    #[cfg(feature = "easytier")]
    {
        let mut pool = cfg
            .easytier
            .as_ref()
            .map(|c| c.ip_pool.clone())
            .unwrap_or_default();
        // Auto-derive pool prefix from cobotsrv's own IPv4 if not explicitly set.
        let own_ipv4 = cfg.easytier.as_ref().map(|c| c.ipv4.as_str()).unwrap_or("");
        pool.prefix = pool.resolve_prefix(own_ipv4);

        crate::service::network::NetworkService::spawn_reclaim_task(database.clone(), pool);
    }

    let mqtt_handle = if let Some(mqtt_cfg) = cfg.mqtt.clone() {
        let conn = Arc::new(database.connection.clone());
        match mqtt::spawn(&mqtt_cfg, conn, cfg.task.result_cache_ttl_secs).await {
            Ok(rt) => {
                info!(url = %mqtt_cfg.url, "cobotsrv mqtt connected");
                Some(rt.handle)
            }
            Err(e) => {
                tracing::warn!(error = %e, "cobotsrv mqtt connect failed; tasks API will return errors");
                None
            }
        }
    } else {
        None
    };

    // Active MCP session registry + idle-eviction reaper (5 min TTL, 60s sweep).
    let mcp_sessions = Arc::new(crate::mcp::session::SessionRegistry::new());
    mcp_sessions.spawn_reaper(Duration::from_secs(300), Duration::from_secs(60));

    // MCP audit log: bounded queue + single background writer. Producers
    // (tools/call handlers) `try_send` and drop on overload rather than
    // spawning a task per call.
    let (mcp_audit_tx, mut mcp_audit_rx) =
        tokio::sync::mpsc::channel::<crate::mcp::audit::CallRecord>(MCP_AUDIT_QUEUE_CAPACITY);
    {
        let db = database.connection.clone();
        tokio::spawn(async move {
            while let Some(rec) = mcp_audit_rx.recv().await {
                crate::mcp::audit::record(&db, rec).await;
            }
        });
    }

    // Daily purge of MCP audit records past the retention window.
    {
        let db = database.connection.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
            loop {
                tick.tick().await;
                match crate::mcp::audit::purge_older_than(&db, MCP_AUDIT_RETENTION_DAYS).await {
                    Ok(deleted) => {
                        info!(
                            deleted,
                            retention_days = MCP_AUDIT_RETENTION_DAYS,
                            "mcp_call_log purge"
                        )
                    }
                    Err(e) => tracing::warn!(error = %e, "mcp_call_log purge failed"),
                }
            }
        });
    }

    let app_state = AppState {
        config: cfg.clone(),
        client: client_req.build().unwrap(),
        storage: store,
        database: database.clone(),
        mqtt: mqtt_handle,
        mcp_sessions,
        mcp_audit_tx,
    };

    // JWT verification (Phase 1): pull cobotusr's public key from JWKS, verify RS256.
    // Hard-fail at startup if the JWKS is unreachable — running an unauthenticated cobotsrv
    // is worse than refusing to start.
    let jwks = Arc::new(
        JwksClient::new(&cfg.auth.jwks_url, Some(cfg.auth.expected_audience.clone()))
            .await
            .expect("failed to fetch cobotusr JWKS at startup"),
    );
    jwks.spawn_refresher(cfg.auth.jwks_refresh_secs);

    // Sync required permissions to cobotusr so the admin can assign them.
    sync_permissions(&cfg).await;

    // http_log wrapper: skip body buffering for SSE (text/event-stream) responses,
    // which never end and would block http_log's collect().await indefinitely.
    async fn http_log_non_sse(req: Request, next: middleware::Next) -> axum::response::Response {
        let is_sse = req
            .headers()
            .get(http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("text/event-stream"))
            .unwrap_or(false);
        if is_sse {
            next.run(req).await
        } else {
            http_log::print_request_response(req, next)
                .await
                .into_response()
        }
    }

    // PDP-based permission enforcement. When `auth.require_permissions = true`
    // (the default), EnforceClient is injected so `require_perm!` calls
    // cobotusr's `/api/v1/enforce` at request time (cached, TTL 10 s).
    // When false the middleware layer is omitted entirely and `require_perm!`
    // falls back to JWT-embedded permissions (or allows all if absent).
    async fn enforce_injector(
        enforce: axum::extract::State<Arc<dyn auth::PermissionChecker>>,
        mut request: Request,
        next: middleware::Next,
    ) -> impl IntoResponse {
        request.extensions_mut().insert(enforce.0.clone());
        next.run(request).await
    }

    // Revocation guard for long-lived MCP tokens. Runs after JWT verification (claims
    // present) and rejects any `/mcp` request whose token `jti` is on cobotusr's denylist.
    // Non-MCP routes are skipped entirely (no cobotusr round-trip). Fails open on outage.
    async fn mcp_revocation_guard(
        rev: axum::extract::State<Arc<crate::mcp::revocation::RevocationClient>>,
        request: Request,
        next: middleware::Next,
    ) -> axum::response::Response {
        let path = request.uri().path();
        if !(path == "/mcp" || path.starts_with("/mcp/")) {
            return next.run(request).await;
        }
        let jti = request
            .extensions()
            .get::<auth::claims::AccessTokenClaims>()
            .map(|c| c.jti.clone())
            .unwrap_or_default();
        if !jti.is_empty() {
            let auth_header = request
                .headers()
                .get(http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if rev.0.is_revoked(&jti, &auth_header).await {
                return (http::StatusCode::UNAUTHORIZED, "token revoked").into_response();
            }
        }
        next.run(request).await
    }

    let revocation: Arc<crate::mcp::revocation::RevocationClient> =
        Arc::new(crate::mcp::revocation::RevocationClient::new(
            cfg.auth.cobotusr_base_url(),
            MCP_REVOCATION_CACHE_TTL_SECS,
        ));

    let mut protected = get_protected_routes();
    if cfg.auth.require_permissions {
        let enforce_client: Arc<dyn auth::PermissionChecker> =
            Arc::new(enforce::EnforceClient::new(
                cfg.auth.cobotusr_base_url(),
                cfg.auth.client_id.clone(),
                10,
            ));
        protected = protected.layer(middleware::from_fn_with_state(
            enforce_client,
            enforce_injector,
        ));
    }
    // Order matters: access middleware (outermost) injects claims, then the revocation
    // guard reads them, then the routes run.
    let protected = protected
        .layer(middleware::from_fn_with_state(
            revocation,
            mcp_revocation_guard,
        ))
        .layer(middleware::from_fn_with_state(
            jwks.clone(),
            access_middleware_rs256,
        ));

    let app = Router::new()
        .merge(protected)
        .merge(get_public_routes())
        .layer({
            let origins: Vec<String> = app_state.config.http.cors_origins.clone();
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
        .layer(axum::middleware::from_fn(http_log_non_sse))
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

    #[cfg(feature = "easytier")]
    if let Some(handle) = stop_handle_storage {
        info!("stopping easytier node");
        handle.stop();
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

/// Sync required permissions to cobotusr at startup.
/// POSTs to cobotusr's `/api/v1/application/sync-permissions` (idempotent
/// upsert). Authenticates with the dedicated confidential machine client
/// (`sync_client_id`/`sync_client_secret`) via the client_credentials grant,
/// while declaring `app_key = client_id` so permissions stay registered under
/// the SPA client's application — the app PDP/`require_perm!` checks.
async fn sync_permissions(cfg: &Config) {
    let perms = &cfg.permissions;
    if perms.is_empty() {
        return;
    }

    let cobotusr_base = cfg.auth.cobotusr_base_url();

    let (sync_client_id, sync_client_secret) = cfg.auth.sync_credentials();
    if cfg.auth.sync_client_id.is_empty() || cfg.auth.sync_client_secret.is_empty() {
        tracing::warn!(
            "[auth] sync_client_id/sync_client_secret not configured — falling back to \
             legacy client_id/client_secret; this only succeeds while the SPA client is \
             still confidential (not public_client)",
        );
    }

    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "client_id": sync_client_id,
        "client_secret": sync_client_secret,
        "app_key": cfg.auth.client_id,
        "permissions": perms.iter().map(|p| serde_json::json!({
            "name": p.name,
            "action": p.action,
            "resource_type": p.resource_type,
            "resource_pattern": p.resource_pattern,
        })).collect::<Vec<_>>(),
    });
    match client
        .post(format!(
            "{}/api/v1/application/sync-permissions",
            cobotusr_base
        ))
        .json(&body)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {
            tracing::info!("{} permissions synced to cobotusr", perms.len());
        }
        Ok(r) => {
            tracing::warn!(status = %r.status(), "permission sync: cobotusr returned error");
        }
        Err(e) => {
            tracing::warn!(error = %e, "permission sync: failed to reach cobotusr");
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    config: Config,
    client: Client,
    storage: Storage,
    database: Arc<DatabaseService>,
    pub mqtt: Option<Arc<crate::mqtt::MqttHandle>>,
    /// Active MCP session registry (Mcp-Session-Id → session info).
    pub mcp_sessions: Arc<crate::mcp::session::SessionRegistry>,
    /// Queue of pending MCP audit records; drained by a single background
    /// writer task (bounded — producers drop on overload).
    pub mcp_audit_tx: tokio::sync::mpsc::Sender<crate::mcp::audit::CallRecord>,
}

pub type IdType = i32;

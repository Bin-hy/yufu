use axum::body::Body;
use axum::extract::State;
use axum::{Router, extract::Request, middleware, response::IntoResponse};
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use http::{HeaderValue, Method, StatusCode, Uri};
use reqwest::Client;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use sea_orm_migration::MigratorTrait;
use std::sync::Arc;
use std::{future::Future, time::Duration};
use tokio::net::TcpListener;

use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, info_span};

use crate::config::Config;
use crate::entity::role::RoleEnum;
use crate::entity::user;
use crate::rbac::authority_check_middleware_rbac_abac;
use crate::route::get_no_prefix_routes;
use crate::service::authority::Service;
use crate::service::database::DatabaseService;
use crate::store::{Node, NodeKind, Storage};
use auth::claims::Claims;
use auth::{AuthState, access_middleware_with_callback};

#[cfg(feature = "webui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../cobotweb/"]
struct Assets;

pub mod config;
pub mod entity;
pub mod error;
mod migration;
pub mod rbac;
mod recording_middleware;
mod result;
pub mod route;
mod seed;
pub mod service;
mod store;
mod swagger_docs;

pub async fn serve<F>(cfg: Config, listener: TcpListener, signal: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    info!("Server listening on {}", listener.local_addr().unwrap());
    let client_req = Client::builder();
    let client_mem = Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_millis(1000));

    #[cfg(feature = "net4mqtt")]
    let (client_req, client_mem) = if let Some(proxy) = match cfg.net4mqtt.clone() {
        Some(c) => {
            // References: https://github.com/seanmonstar/reqwest/issues/899
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

    // Middleware and AppState
    let auth_state = AuthState {
        token_manager: auth::token_manager::TokenManager::new(
            &cfg.auth.secret,
            cfg.auth.exp_seconds,
        ),
        refresh_token_manager: auth::token_manager::TokenManager::new(
            &cfg.auth.secret,
            cfg.auth.refresh_exp_seconds,
        ),
    };

    let app_state = AppState {
        config: cfg.clone(),
        client: client_req.build().unwrap(),
        storage: store,
        database: Arc::new(database_service),
        auth_state: Arc::new(auth_state.clone()),
    };
    let _ = app_state.init().await;
    let app_state_for_middleware = app_state.clone();
    // routes need auth
    let sub_auth_router = route::get_auth_routes()
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            recording_middleware::recording_operation_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            authority_check_middleware_rbac_abac,
        ))
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            move |State(auth_state): State<AuthState>, req, next| {
                let auth_state = auth_state.clone();
                let app_state = app_state_for_middleware.clone(); // Arc clone

                async move {
                    access_middleware_with_callback(
                        State(auth_state),
                        req,
                        next,
                        move |claims, req| {
                            let claims = claims.clone();
                            let app_state = app_state.clone();
                            async move {
                                // 调用 access_callback 获取 Request 所有权
                                let req = access_callback(claims, req, app_state).await?;
                                Ok::<Request<Body>, (StatusCode, String)>(req)
                            }
                        },
                    )
                    .await
                }
            },
        )); // 靠下的拦截器先执行

    // routes don't need auth,public routes
    let sub_public_router = route::get_public_routes();

    let api_prefix = &cfg.http.api_prefix;
    let mut app = Router::new()
        .nest(api_prefix, sub_auth_router)
        .nest(api_prefix, sub_public_router)
        .merge(get_no_prefix_routes())
        .layer(if cfg.http.cors {
            CorsLayer::permissive()
        } else {
            let origins: Vec<String> = app_state.clone().config.auth.origins;
            origins.iter().fold(
                CorsLayer::new()
                    .allow_credentials(true)
                    .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
                    .allow_methods([
                        Method::GET,
                        Method::PATCH,
                        Method::POST,
                        Method::DELETE,
                        Method::OPTIONS,
                    ]),
                |cors, origin| cors.allow_origin(origin.parse::<HeaderValue>().unwrap()),
            )
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
    #[cfg(feature = "openapi")]
    {
        use crate::swagger_docs::doc::ApiDoc;
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        app = app.merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", ApiDoc::openapi()));
    }

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
        None => {
            // fallback to index.html
            match Assets::get("index.html") {
                Some(index) => {
                    let mime = mime_guess::from_path("index.html").first_or_octet_stream();
                    ([(http::header::CONTENT_TYPE, mime.as_ref())], index.data).into_response()
                }
                None => (StatusCode::NOT_FOUND, "not found").into_response(),
            }
        }
    }
}

#[cfg(not(feature = "webui"))]
async fn static_handler(_: Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "feature webui not enable")
}

/// 鉴权成功后回调：将用户信息注入
async fn access_callback(
    claim: Claims<IdType>,
    mut req: Request<Body>, // 拥有 Request 所有权
    app_state: AppState,
) -> Result<Request<Body>, (StatusCode, String)> {
    let db = &app_state.database.connection;

    let user = user::Entity::find_by_id(claim.id)
        .one(db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "No user id".to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "User not found".to_string()))?;
    req.extensions_mut().insert(user);

    Ok(req)
}
#[derive(Clone)]
pub struct AppState {
    config: Config,
    client: Client,
    storage: Storage,
    database: Arc<DatabaseService>,
    auth_state: Arc<AuthState>,
}
pub type IdType = i32;

impl AppState {
    pub async fn init(&self) -> result::Result<()> {
        self.ensure_sadmin_authorities().await?;
        self.init_sadmin_from_config().await?;
        Ok(())
    }
    // 初始化超级管理员权限
    async fn ensure_sadmin_authorities(&self) -> result::Result<()> {
        Service::ensure_sadmin_authorities(&self.database.connection).await?;
        Ok(())
    }

    // 初始化用户
    async fn init_sadmin_from_config(&self) -> Result<(), DbErr> {
        use crate::entity::user::ActiveModel as User;

        let db = &self.database.connection;
        let config = self.config.clone();
        let super_admin_account = config.auth.super_admin.clone();
        let sadmin_user = super_admin_account.username;
        let sadmin_password = bcrypt::hash(super_admin_account.password, bcrypt::DEFAULT_COST)
            .map_err(|e| DbErr::Custom(format!("Password hash failed: {}", e)))?;

        // 检查是否已存在
        if user::Entity::find()
            .filter(user::Column::Username.eq(&sadmin_user))
            .one(db)
            .await?
            .is_none()
        {
            User {
                id: ActiveValue::NotSet,
                username: Set(sadmin_user),
                password: Set(Some(sadmin_password)),
                nickname: Set(Some("sadmin".to_string())),
                role_id: Set(RoleEnum::Superadmin.default_id()),
                description: Set(Some("The System Admin.".to_string())),
                create_time: Set(chrono::Utc::now().naive_utc()),
                update_time: Set(chrono::Utc::now().naive_utc()),
            }
            .insert(db)
            .await?;
            info!("Initial the super admin account has created successfully!");
        } else {
            info!("Initial the super admin account has created before.");
        }
        Ok(())
    }
}

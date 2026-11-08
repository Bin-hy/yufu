use axum::body::Body;
use axum::extract::State;
use axum::{Router, extract::Request, middleware, response::IntoResponse};
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, SET_COOKIE};
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use sea_orm_migration::MigratorTrait;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info, info_span, warn};

use crate::config::Config;
use crate::entity::role::RoleEnum;
use crate::entity::user;
use crate::modules::im::service::{ConnectionManager, event_channel, spawn_event_service};
use crate::rbac::authority_check_middleware_rbac_abac;
use crate::route::get_no_prefix_routes;
use crate::service::authority::Service;
use crate::service::database::DatabaseService;
use auth::access_middleware_local_rs256;
use auth::claims::AccessTokenClaims;
use auth::token_manager::{IdTokenSigner, TokenManager};

#[cfg(feature = "webui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../assets/cobotusr/"]
struct Assets;

pub mod config;
pub mod entity;
pub mod error;
pub mod migration;
pub mod modules;
pub mod rbac;
mod recording_middleware;
mod result;
pub mod route;
mod seed;
pub mod service;
mod swagger_docs;

fn is_successful_http_exchange(status: StatusCode) -> bool {
    status.is_informational() || status.is_success()
}

pub(crate) fn redact_access_token(uri: &http::Uri) -> String {
    let Some(query) = uri.query() else {
        return uri.to_string();
    };

    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    if !pairs.iter().any(|(key, _)| key == "access_token") {
        return uri.to_string();
    }

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        let value = if key == "access_token" {
            "REDACTED"
        } else {
            value.as_ref()
        };
        serializer.append_pair(&key, value);
    }

    format!("{}?{}", uri.path(), serializer.finish())
}

fn redact_sensitive_headers(mut headers: HeaderMap) -> HeaderMap {
    headers.remove(AUTHORIZATION);
    headers.remove(COOKIE);
    headers.remove(SET_COOKIE);
    headers
}

/// Endpoints whose request/response bodies carry credentials (passwords,
/// tokens, client secrets) or large binary payloads are logged as status
/// lines only. Paths under the API prefix are derived from the configured
/// `http.api_prefix` instead of being hard-coded.
fn should_skip_body_logging(uri: &http::Uri, headers: &HeaderMap, api_prefix: &str) -> bool {
    let path = uri.path();
    matches!(path, "/oidc/token" | "/oidc/revoke" | "/oidc/introspect")
        || [
            format!("{api_prefix}/login"),
            format!("{api_prefix}/register"),
            format!("{api_prefix}/refresh"),
            format!("{api_prefix}/im/files"),
        ]
        .iter()
        .any(|skip| skip == path)
        || headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("multipart/"))
}

/// Bodies larger than this are never buffered for logging — only the status
/// line is recorded (e.g. paginated message history responses).
const MAX_LOGGED_BODY_BYTES: u64 = 256 * 1024;

fn body_exceeds_log_limit(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|len| len > MAX_LOGGED_BODY_BYTES)
}

fn log_http_exchange(method: &Method, uri: &str, status: StatusCode, duration: Duration) {
    if is_successful_http_exchange(status) {
        if duration.as_millis() > 500 {
            warn!(
                "[{} {}] [{}] {}ms",
                method,
                uri,
                status.as_u16(),
                duration.as_millis()
            );
        } else {
            info!(
                "[{} {}] [{}] {}ms",
                method,
                uri,
                status.as_u16(),
                duration.as_millis()
            );
        }
    } else {
        error!(
            "[{} {}] [{}] {}ms",
            method,
            uri,
            status.as_u16(),
            duration.as_millis()
        );
    }
}

async fn print_request_response(
    State(api_prefix): State<String>,
    req: Request,
    next: middleware::Next,
) -> std::result::Result<impl IntoResponse, (StatusCode, String)> {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = redact_access_token(req.uri());
    let skip_body_logging = should_skip_body_logging(req.uri(), req.headers(), &api_prefix)
        || body_exceeds_log_limit(req.headers());

    if skip_body_logging {
        let response = next.run(req).await;
        log_http_exchange(&method, &uri, response.status(), start.elapsed());
        return Ok(response);
    }

    let request_headers = redact_sensitive_headers(req.headers().clone());
    let (request_parts, request_body) = req.into_parts();
    let request_bytes = http_log::buffer_and_print("request", request_headers, request_body)
        .await
        .inspect_err(|err| {
            warn!("{}", err.1);
        })?;
    let request = Request::from_parts(request_parts, Body::from(request_bytes));

    let response = next.run(request).await;
    // Large responses (e.g. paginated message history) skip body buffering
    // and only record the status line.
    if body_exceeds_log_limit(response.headers()) {
        log_http_exchange(&method, &uri, response.status(), start.elapsed());
        return Ok(response);
    }
    let response_headers = redact_sensitive_headers(response.headers().clone());
    let (response_parts, response_body) = response.into_parts();
    let response_bytes = http_log::buffer_and_print("response", response_headers, response_body)
        .await
        .inspect_err(|err| {
            warn!("{}", err.1);
        })?;
    let response = axum::response::Response::from_parts(response_parts, Body::from(response_bytes));

    let duration = start.elapsed();
    let status = response.status();
    log_http_exchange(&method, &uri, status, duration);

    Ok(response)
}

pub async fn serve<F>(cfg: Config, listener: TcpListener, signal: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    serve_internal(cfg, listener, signal, None).await;
}

/// The clock-health monitor is supplied by the host. No assumed-synchronized
/// fallback is allowed for transactional credential issuance.
pub async fn serve_with_security_clock<F>(
    cfg: Config,
    listener: TcpListener,
    signal: F,
    clock: Arc<dyn auth::security::budget::Clock>,
) where
    F: Future<Output = ()> + Send + 'static,
{
    serve_internal(cfg, listener, signal, Some(clock)).await;
}

async fn serve_internal<F>(
    cfg: Config,
    listener: TcpListener,
    signal: F,
    security_clock: Option<Arc<dyn auth::security::budget::Clock>>,
) where
    F: Future<Output = ()> + Send + 'static,
{
    info!("Server listening on {}", listener.local_addr().unwrap());

    // Initialize database connection
    let database_service = DatabaseService::new(&cfg.database)
        .await
        .expect("Failed to create database service");
    migration::Migrator::up(&database_service.connection, None)
        .await
        .unwrap();

    // Refresh tokens stay HS256 / opaque-internal (cobotsrv issues + verifies them itself,
    // they never cross service boundaries). Access tokens move to RS256 via id_token_signer.
    let refresh_token_manager = TokenManager::new(&cfg.auth.secret, cfg.auth.refresh_exp_seconds);

    // Initialize IdTokenSigner. Default RS256 (load RSA from path, auto-generate if absent);
    // tests / dev can opt into HS256 via oidc.signing_algorithm to skip the RSA dance.
    let access_token_exp = cfg
        .oidc
        .access_token_expire_seconds
        .unwrap_or(cfg.auth.exp_seconds);
    let id_token_signer = if cfg.oidc.signing_algorithm.eq_ignore_ascii_case("HS256") {
        info!("OIDC signer: HS256 (HMAC) — using auth.secret as key");
        IdTokenSigner::from_hmac(&cfg.auth.secret, access_token_exp)
    } else {
        let key_path = if !cfg.oidc.rsa_private_key_path.is_empty() {
            cfg.oidc.rsa_private_key_path.clone()
        } else {
            cfg.auth.rsa_private_key_path.clone()
        };
        match tokio::fs::read(&key_path).await {
            Ok(pem) => {
                info!("Loaded RSA private key from {}", key_path);
                IdTokenSigner::from_rsa_pem(&pem, access_token_exp)
                    .expect("Failed to parse RSA private key")
            }
            Err(_) => {
                info!(
                    "RSA key not found at {}, generating new key pair...",
                    key_path
                );
                use rsa::pkcs8::EncodePrivateKey;
                let mut rng = rand::rngs::OsRng;
                let private_key =
                    rsa::RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate RSA key");
                let pem_str = private_key
                    .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
                    .expect("Failed to encode RSA key to PEM");
                if let Some(parent) = std::path::Path::new(&key_path).parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                tokio::fs::write(&key_path, pem_str.as_bytes())
                    .await
                    .expect("Failed to write RSA key file");
                info!("Generated and saved RSA key to {}", key_path);
                IdTokenSigner::from_rsa_pem(pem_str.as_bytes(), access_token_exp)
                    .expect("Failed to parse generated RSA key")
            }
        }
    };

    // IM: websocket connection manager and event channel (before database_service move)
    let im_db_conn = database_service.connection.clone();
    let ws_handle = std::sync::Arc::new(ConnectionManager::new());
    let (im_events, im_event_receiver) = event_channel();
    let _im_event_service = spawn_event_service(im_db_conn, ws_handle.clone(), im_event_receiver);

    let database = Arc::new(database_service);
    let id_token_signer = Arc::new(id_token_signer);

    let app_state = AppState {
        config: cfg.clone(),
        database: database.clone(),
        refresh_token_manager: refresh_token_manager.clone(),
        id_token_signer: id_token_signer.clone(),
        security_clock,
        im_events,
        ws_handle,
    };
    let _ = app_state.init().await;

    // Auth middleware stack (applied in reverse layer order — bottom runs first):
    //   1. access_middleware_local_rs256 — verify Bearer JWT, inject AccessTokenClaims
    //   2. load_user_middleware           — read claims.sub, load user::Model into ext
    //   3. authority_check_middleware     — RBAC/ABAC check (uses user::Model)
    //   4. recording_operation_middleware — audit log
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
            app_state.clone(),
            load_user_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            id_token_signer.clone(),
            access_middleware_local_rs256,
        ));

    // routes don't need auth, public routes
    let sub_public_router = route::get_public_routes();

    // Service-to-service routes: Bearer JWT verified, but no user load / URI-RBAC,
    // so machine callers (cobotsrv revocation introspection) always get JSON or 401.
    let sub_service_router = route::get_service_routes().layer(middleware::from_fn_with_state(
        id_token_signer.clone(),
        access_middleware_local_rs256,
    ));

    let api_prefix = &cfg.http.api_prefix;
    #[cfg_attr(not(feature = "openapi"), allow(unused_mut))]
    let mut app = Router::new()
        .nest(api_prefix, sub_auth_router)
        .nest(api_prefix, sub_public_router)
        .nest(api_prefix, sub_service_router)
        .merge(get_no_prefix_routes())
        .layer({
            let origins: Vec<String> = app_state.clone().config.auth.origins;
            let has_wildcard = origins.iter().any(|o| o == "*");
            let cors_layer = CorsLayer::new()
                .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
                .allow_methods([
                    Method::GET,
                    Method::PATCH,
                    Method::POST,
                    Method::DELETE,
                    Method::OPTIONS,
                ]);

            if has_wildcard {
                // Browser SPA requests authenticate with bearer tokens, not cookies.  A true
                // wildcard therefore avoids credential-mode restrictions while reliably
                // returning Access-Control-Allow-Origin for public OIDC endpoints.
                cors_layer.allow_origin(Any)
            } else {
                // Use predicate to match request Origin against allowed origins list
                cors_layer.allow_credentials(true).allow_origin(
                    tower_http::cors::AllowOrigin::predicate(
                        move |origin: &HeaderValue, _request_head: &http::request::Parts| {
                            if let Ok(origin_str) = origin.to_str() {
                                origins.contains(&origin_str.to_string())
                            } else {
                                false
                            }
                        },
                    ),
                )
            }
        })
        .with_state(app_state.clone())
        .layer(axum::middleware::from_fn_with_state(
            cfg.http.api_prefix.clone(),
            print_request_response,
        ))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let span = info_span!(
                    "http_request",
                    uri = %redact_access_token(request.uri()),
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

/// Middleware that loads the current user from DB based on a previously-verified
/// [`AccessTokenClaims`] in request extensions. Run AFTER `access_middleware_local_rs256`.
pub async fn load_user_middleware(
    State(app_state): State<AppState>,
    mut request: Request<Body>,
    next: middleware::Next,
) -> axum::response::Response {
    let claims = match request.extensions().get::<AccessTokenClaims>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing token claims (auth middleware not applied?)",
            )
                .into_response();
        }
    };

    let user_id = match parse_user_id(&claims.sub) {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    let user = match user::Entity::find_by_id(user_id)
        .one(&app_state.database.connection)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "User not found").into_response(),
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    request.extensions_mut().insert(user);
    next.run(request).await
}

/// Parse the JWT `sub` claim (string per OIDC spec) into our local `IdType` (i32).
/// Returns `AppError::Unauthorized` so handlers can use `?` to bail out.
pub fn parse_user_id(sub: &str) -> result::Result<IdType> {
    sub.parse::<IdType>()
        .map_err(|_| crate::error::AppError::Unauthorized(format!("invalid sub in token: {sub}")))
}

#[derive(Clone)]
pub struct AppState {
    config: Config,
    database: Arc<DatabaseService>,
    /// HS256 manager for opaque-internal refresh tokens (never crosses service boundaries).
    refresh_token_manager: TokenManager,
    /// RS256 signer for cross-service access tokens (also exposed via JWKS).
    id_token_signer: Arc<IdTokenSigner>,
    security_clock: Option<Arc<dyn auth::security::budget::Clock>>,
    pub im_events: crate::modules::im::service::ImEvents,
    pub ws_handle: crate::modules::im::service::WsHandle,
}
pub type IdType = i32;

impl AppState {
    pub fn security_clock(
        &self,
    ) -> std::result::Result<
        Arc<dyn auth::security::budget::Clock>,
        auth::security::error::SecurityError,
    > {
        self.security_clock
            .clone()
            .ok_or(auth::security::error::SecurityError::DependencyUnavailable)
    }
    pub async fn init(&self) -> result::Result<()> {
        self.ensure_sadmin_authorities().await?;
        self.init_sadmin_from_config().await?;
        self.init_oidc_bootstrap_clients().await?;
        Ok(())
    }

    /// OIDC access token expiry: oidc.access_token_expire_seconds > auth.exp_seconds
    pub fn oidc_access_token_exp(&self) -> i64 {
        self.config
            .oidc
            .access_token_expire_seconds
            .unwrap_or(self.config.auth.exp_seconds)
    }

    /// OIDC refresh token expiry: oidc.refresh_token_expire_seconds > auth.refresh_exp_seconds
    pub fn oidc_refresh_token_exp(&self) -> i64 {
        self.config
            .oidc
            .refresh_token_expire_seconds
            .unwrap_or(self.config.auth.refresh_exp_seconds)
    }

    /// Issuer URL used in access/id token `iss` claim.
    /// Priority: cfg.oidc.issuer > cfg.http.public > derived from listen address.
    pub fn issuer(&self) -> String {
        if !self.config.oidc.issuer.is_empty() {
            self.config.oidc.issuer.clone()
        } else if !self.config.http.public.is_empty() {
            self.config.http.public.clone()
        } else {
            format!("http://{}", self.config.http.listen)
        }
    }

    /// Configured upper bound (days) for long-lived MCP access tokens.
    pub fn mcp_token_max_ttl_days(&self) -> u32 {
        self.config.oidc.mcp_token_max_ttl_days
    }

    /// Mint an access token with an explicit TTL, `jti`, and pre-resolved
    /// roles/permissions. Used for long-lived MCP access tokens
    /// (`POST /api/v1/oidc/mcp-tokens`); the caller persists a revocable record keyed
    /// by the same `jti` so the token can later be revoked.
    pub fn mint_access_token(
        &self,
        sub: IdType,
        aud: &str,
        ttl_seconds: i64,
        roles: Vec<String>,
        permissions: Vec<String>,
        jti: &str,
    ) -> result::Result<String> {
        let iss = self.issuer();
        self.id_token_signer
            .sign_access_token_with_jti(
                &sub.to_string(),
                &iss,
                aud,
                "",
                ttl_seconds,
                roles,
                permissions,
                jti,
            )
            .map_err(|e| crate::error::AppError::InternalServerError(anyhow::anyhow!(e)))
    }

    /// Sign an access token for client_credentials grant (sub = client_id, no user).
    pub fn sign_access_token_for_client(
        &self,
        client_id: &str,
        aud: &str,
        scope: &str,
    ) -> result::Result<String> {
        let iss = self.issuer();
        self.id_token_signer
            .sign_access_token(
                client_id,
                &iss,
                aud,
                scope,
                self.oidc_access_token_exp(),
                vec![],
                vec![],
            )
            .map_err(|e| crate::error::AppError::InternalServerError(anyhow::anyhow!(e)))
    }

    pub fn refresh_token_manager(&self) -> &TokenManager {
        &self.refresh_token_manager
    }

    pub fn id_token_signer(&self) -> &Arc<IdTokenSigner> {
        &self.id_token_signer
    }

    // 初始化超级管理员权限
    async fn ensure_sadmin_authorities(&self) -> result::Result<()> {
        Service::ensure_sadmin_authorities(&self.database.connection).await?;
        Ok(())
    }

    /// Auto-register OIDC clients listed in `[[oidc.bootstrap_clients]]`,
    /// and pair each one with an `Application` row so it shows up in the
    /// permission system. Existing secrets are NEVER overwritten. Public
    /// clients are upgraded when configured as such, and bootstrap redirect
    /// URIs are added (without removing administrator-managed URIs) so old
    /// databases receive required callback paths on upgrade.
    async fn init_oidc_bootstrap_clients(&self) -> result::Result<()> {
        use crate::entity::{application, oauth_client};
        use rand::RngCore;
        use rand::rngs::OsRng;
        use sea_orm::Set;

        let bootstrap = &self.config.oidc.bootstrap_clients;
        if bootstrap.is_empty() {
            return Ok(());
        }

        let db = &self.database.connection;

        // Owner of bootstrap clients = the super-admin user we just seeded.
        let sadmin = user::Entity::find()
            .filter(user::Column::Username.eq(&self.config.auth.super_admin.username))
            .one(db)
            .await?
            .ok_or_else(|| {
                crate::error::AppError::InternalServerError(anyhow::anyhow!(
                    "bootstrap clients require a super-admin user, none found"
                ))
            })?;

        for entry in bootstrap {
            // Dirty-data guard (mirrors OidcService::register_client): a public
            // client proves possession with PKCE and has no secret, so the
            // client_credentials grant can never succeed for it. Rejecting at
            // startup beats a silent 401 in every downstream machine call.
            if entry.public_client && entry.grant_types.iter().any(|g| g == "client_credentials") {
                return Err(crate::error::AppError::InternalServerError(
                    anyhow::anyhow!(
                        "bootstrap client '{}' is public_client but requests the \
                     client_credentials grant — public clients cannot authenticate \
                     with a secret; use a separate confidential machine client",
                        entry.client_id
                    ),
                ));
            }

            // ── 1. OIDC client ──
            let oidc_client_id = match oauth_client::Entity::get_by_client_id(db, &entry.client_id)
                .await?
            {
                Some(existing) => {
                    let mut redirect_uris: Vec<String> = existing
                        .redirect_uris
                        .split(',')
                        .map(str::trim)
                        .filter(|uri| !uri.is_empty())
                        .map(str::to_string)
                        .collect();
                    let mut redirects_changed = false;
                    for uri in &entry.redirect_uris {
                        if !redirect_uris.contains(uri) {
                            redirect_uris.push(uri.clone());
                            redirects_changed = true;
                        }
                    }
                    let public_upgrade = entry.public_client && !existing.public_client;
                    if public_upgrade || redirects_changed {
                        let mut active: oauth_client::ActiveModel = existing.into();
                        if public_upgrade {
                            active.public_client = Set(true);
                            active.client_secret_hash = Set(String::new());
                        }
                        if redirects_changed {
                            active.redirect_uris = Set(redirect_uris.join(","));
                        }
                        active.update_time = Set(chrono::Utc::now().naive_utc());
                        let updated = active.update(db).await?;
                        info!(client_id = %entry.client_id, public_upgrade, redirects_changed, "OIDC bootstrap client upgraded");
                        updated.id
                    } else {
                        info!(client_id = %entry.client_id, "OIDC bootstrap client already exists");
                        existing.id
                    }
                }
                None => {
                    let (secret, generated) = match (&entry.client_secret, entry.public_client) {
                        (_, true) => (String::new(), false),
                        (Some(s), false) => (s.clone(), false),
                        (None, false) => {
                            let mut buf = [0u8; 32];
                            OsRng.fill_bytes(&mut buf);
                            (hex::encode(buf), true)
                        }
                    };
                    let secret_hash = if entry.public_client {
                        String::new()
                    } else {
                        bcrypt::hash(&secret, bcrypt::DEFAULT_COST).map_err(|e| {
                            crate::error::AppError::InternalServerError(anyhow::anyhow!(
                                "bcrypt hash failed for bootstrap client {}: {e}",
                                entry.client_id
                            ))
                        })?
                    };

                    let now = chrono::Utc::now().naive_utc();
                    let inserted = oauth_client::ActiveModel {
                        id: ActiveValue::NotSet,
                        client_id: Set(entry.client_id.clone()),
                        user_id: Set(sadmin.id),
                        name: Set(entry.name.clone()),
                        redirect_uris: Set(entry.redirect_uris.join(",")),
                        allowed_scopes: Set(entry.allowed_scopes.join(",")),
                        client_secret_hash: Set(secret_hash),
                        public_client: Set(entry.public_client),
                        grant_types: Set(entry.grant_types.join(",")),
                        create_time: Set(now),
                        update_time: Set(now),
                    }
                    .insert(db)
                    .await?;

                    if generated {
                        tracing::warn!(
                            client_id = %entry.client_id,
                            client_secret = %secret,
                            "OIDC bootstrap client registered with AUTO-GENERATED secret — \
                             copy this value to your client config NOW; it won't be shown again",
                        );
                    } else {
                        info!(client_id = %entry.client_id, "OIDC bootstrap client registered (using configured secret)");
                    }
                    inserted.id
                }
            };

            // ── 2. Application row (paired by app_key = client_id) ──
            //
            // Backfilled even when the oauth_client already existed, so users
            // upgrading from before Phase 2.A get the Application implicitly.
            if application::Entity::get_by_app_key(db, &entry.client_id)
                .await?
                .is_none()
            {
                let now = chrono::Utc::now().naive_utc();
                application::ActiveModel {
                    id: ActiveValue::NotSet,
                    app_key: Set(entry.client_id.clone()),
                    name: Set(entry.name.clone()),
                    description: Set(None),
                    oidc_client_id: Set(Some(oidc_client_id)),
                    create_time: Set(now),
                    update_time: Set(now),
                }
                .insert(db)
                .await?;
                info!(
                    app_key = %entry.client_id,
                    oidc_client_id,
                    "Application registered for bootstrap OIDC client"
                );
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_successful_http_exchange_accepts_websocket_switching_protocols() {
        assert!(is_successful_http_exchange(StatusCode::SWITCHING_PROTOCOLS));
        assert!(is_successful_http_exchange(StatusCode::OK));
        assert!(!is_successful_http_exchange(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn test_redact_access_token_masks_each_token_and_preserves_other_parameters() {
        let uri: http::Uri =
            "/api/v1/im/files?path=storage%2Fvideo%2Fclip.mp4&access_token=first&access_token=second"
                .parse()
                .unwrap();

        let redacted = redact_access_token(&uri);

        assert_eq!(
            redacted,
            "/api/v1/im/files?path=storage%2Fvideo%2Fclip.mp4&access_token=REDACTED&access_token=REDACTED"
        );
        assert!(!redacted.contains("first"));
        assert!(!redacted.contains("second"));
    }

    #[test]
    fn test_redact_access_token_leaves_uris_without_token_unchanged() {
        let without_query: http::Uri = "/api/v1/im/users".parse().unwrap();
        let without_token: http::Uri = "/api/v1/im/files?path=storage%2Fvideo%2Fclip.mp4"
            .parse()
            .unwrap();

        assert_eq!(
            redact_access_token(&without_query),
            without_query.to_string()
        );
        assert_eq!(
            redact_access_token(&without_token),
            without_token.to_string()
        );
    }

    #[test]
    fn test_redact_sensitive_headers_removes_credentials() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer token"));
        headers.insert(COOKIE, HeaderValue::from_static("sid=value"));
        headers.insert(SET_COOKIE, HeaderValue::from_static("sid=value"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let redacted = redact_sensitive_headers(headers);

        assert!(!redacted.contains_key(AUTHORIZATION));
        assert!(!redacted.contains_key(COOKIE));
        assert!(!redacted.contains_key(SET_COOKIE));
        assert_eq!(
            redacted.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static("application/json"))
        );
    }

    #[test]
    fn test_should_skip_body_logging_for_oidc_token_endpoints() {
        let headers = HeaderMap::new();
        let mut multipart_headers = HeaderMap::new();
        multipart_headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=test"),
        );

        assert!(should_skip_body_logging(
            &"/oidc/token".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(should_skip_body_logging(
            &"/oidc/revoke".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(should_skip_body_logging(
            &"/api/v1/im/files".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(!should_skip_body_logging(
            &"/something-im/files".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(!should_skip_body_logging(
            &"/api/v1/im/users".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(should_skip_body_logging(
            &"/api/v1/im/messages".parse().unwrap(),
            &multipart_headers,
            "/api/v1"
        ));
    }

    #[test]
    fn test_should_skip_body_logging_for_credential_endpoints() {
        let headers = HeaderMap::new();

        // OIDC introspection carries token + client_secret in the form body;
        // legacy auth endpoints carry plaintext passwords / access tokens.
        assert!(should_skip_body_logging(
            &"/oidc/introspect".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        for path in ["/api/v1/login", "/api/v1/register", "/api/v1/refresh"] {
            assert!(should_skip_body_logging(
                &path.parse().unwrap(),
                &headers,
                "/api/v1"
            ));
        }
        assert!(!should_skip_body_logging(
            &"/api/v1/users".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
    }

    #[test]
    fn test_should_skip_body_logging_follows_configured_api_prefix() {
        let headers = HeaderMap::new();

        assert!(should_skip_body_logging(
            &"/custom/login".parse().unwrap(),
            &headers,
            "/custom"
        ));
        assert!(should_skip_body_logging(
            &"/custom/im/files".parse().unwrap(),
            &headers,
            "/custom"
        ));
        assert!(!should_skip_body_logging(
            &"/api/v1/login".parse().unwrap(),
            &headers,
            "/custom"
        ));
    }

    #[test]
    fn test_body_exceeds_log_limit_checks_content_length() {
        let mut over_limit = HeaderMap::new();
        over_limit.insert(
            CONTENT_LENGTH,
            HeaderValue::from_static("262145"), // 256 KiB + 1
        );
        assert!(body_exceeds_log_limit(&over_limit));

        let mut at_limit = HeaderMap::new();
        at_limit.insert(CONTENT_LENGTH, HeaderValue::from_static("262144")); // 256 KiB
        assert!(!body_exceeds_log_limit(&at_limit));

        let mut invalid = HeaderMap::new();
        invalid.insert(CONTENT_LENGTH, HeaderValue::from_static("not-a-number"));
        assert!(!body_exceeds_log_limit(&invalid));

        assert!(!body_exceeds_log_limit(&HeaderMap::new()));
    }
}

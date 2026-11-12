use axum::body::Body;
use axum::extract::State;
use axum::{Router, extract::Request, middleware, response::IntoResponse};
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, SET_COOKIE};
use http::{HeaderMap, HeaderValue, Method, StatusCode};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use sea_orm_migration::MigratorTrait;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, info_span, warn};

use crate::config::Config;
use crate::entity::role::RoleEnum;
use crate::entity::user;
use crate::rbac::authority_check_middleware_rbac_abac;
use crate::route::get_no_prefix_routes;
use crate::service::authority::Service;
use crate::service::database::DatabaseService;
use auth::access_middleware_local_rs256;
use auth::claims::AccessTokenClaims;
use auth::token_manager::{IdTokenSigner, TokenManager};

pub mod clock;
pub mod config;
mod config_identity;
pub mod domain;
pub mod entity;
pub mod error;
pub mod migration;
pub mod rbac;
mod recording_middleware;
mod result;
pub mod route;
mod seed;
pub mod service;
mod swagger_docs;
mod webui;

fn deployment_cors(cfg: &Config) -> CorsLayer {
    let origins = cfg.auth.origins.clone();
    let wildcard = origins.iter().any(|o| o == "*");
    CorsLayer::new()
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            ACCEPT,
            http::HeaderName::from_static("x-csrf-token"),
        ])
        .allow_methods([
            Method::GET,
            Method::PATCH,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_credentials(!wildcard)
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin: &HeaderValue, head: &http::request::Parts| {
                !route::identity_cors::is_identity_path(head.uri.path())
                    && origin
                        .to_str()
                        .is_ok_and(|o| wildcard || origins.iter().any(|allowed| allowed == o))
            },
        ))
}

fn is_successful_http_exchange(status: StatusCode) -> bool {
    status.is_informational() || status.is_success()
}

pub(crate) fn redact_access_token(uri: &http::Uri) -> String {
    let Some(query) = uri.query() else {
        return uri.to_string();
    };

    // Identity endpoints carry action proofs, PKCE, nonce and personal data.
    // Preserve only the path for both access logs and historical recordings.
    if uri.path().split('/').any(|segment| {
        matches!(
            segment,
            "identity" | "account" | "developer" | "admin" | "oidc"
        )
    }) {
        return uri.path().to_owned();
    }

    fn sensitive(key: &str) -> bool {
        matches!(
            key,
            "access_token"
                | "refresh_token"
                | "id_token"
                | "id_token_hint"
                | "logout_token"
                | "token"
                | "code"
                | "code_verifier"
                | "client_secret"
                | "password"
                | "old_password"
                | "new_password"
                | "secret"
                | "invitation"
                | "reset_token"
                | "action_secret"
        )
    }

    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    if !pairs.iter().any(|(key, _)| sensitive(key)) {
        return uri.to_string();
    }

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        let value = if sensitive(&key) {
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
    headers.remove("x-csrf-token");
    headers
}

/// Endpoints whose request/response bodies carry credentials (passwords,
/// tokens, client secrets) or large binary payloads are logged as status
/// lines only. Paths under the API prefix are derived from the configured
/// `http.api_prefix` instead of being hard-coded.
fn should_skip_body_logging(uri: &http::Uri, headers: &HeaderMap, api_prefix: &str) -> bool {
    let path = uri.path();
    if ["identity", "account", "developer", "admin"]
        .iter()
        .any(|group| {
            let prefix = format!("{api_prefix}/{group}");
            path == prefix || path.starts_with(&format!("{prefix}/"))
        })
        || (path.split('/').any(|segment| segment == "oidc") && !path.ends_with("/jwks"))
    {
        return true;
    }
    matches!(
        path,
        "/oidc/token" | "/oidc/revoke" | "/oidc/introspect" | "/oidc/logout"
    ) || [
        format!("{api_prefix}/login"),
        format!("{api_prefix}/register"),
        format!("{api_prefix}/refresh"),
        format!("{api_prefix}/csrf"),
        format!("{api_prefix}/logout"),
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
    if let Err(error) = cfg.validate_identity_policy() {
        error!(reason = %error, "invalid identity configuration; server not started");
        return;
    }
    if cfg.auth.allow_insecure_loopback_cookies
        && !listener
            .local_addr()
            .is_ok_and(|address| address.ip().is_loopback())
    {
        error!("insecure development cookies require an actual loopback listener");
        return;
    }
    info!("Server listening on {}", listener.local_addr().unwrap());

    let id_token_signer = match cfg.load_identity_signer().await {
        Ok(signer) => signer,
        Err(error) => {
            error!(reason = %error, "identity signer unavailable; server not started");
            return;
        }
    };

    // Initialize database connection
    let database_service = DatabaseService::new(&cfg.database)
        .await
        .expect("Failed to create database service");
    migration::Migrator::up(&database_service.connection, None)
        .await
        .unwrap();

    // Legacy refresh compatibility remains local to the authentication center (
    // they never cross service boundaries). Access tokens move to RS256 via id_token_signer.
    let refresh_token_manager = TokenManager::new(&cfg.auth.secret, cfg.auth.refresh_exp_seconds);

    let database = Arc::new(database_service);
    let id_token_signer = Arc::new(id_token_signer);

    let app_state = AppState {
        config: cfg.clone(),
        database: database.clone(),
        refresh_token_manager: refresh_token_manager.clone(),
        id_token_signer: id_token_signer.clone(),
        security_clock,
    };
    if app_state.init().await.is_err() {
        error!("identity initialization failed; server not started");
        return;
    }

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
            app_state.clone(),
            service::identity::legacy_governance::request_context,
        ))
        .layer(middleware::from_fn_with_state(
            id_token_signer.clone(),
            access_middleware_local_rs256,
        ));

    // routes don't need auth, public routes
    let sub_public_router = route::get_public_routes();

    // Service-to-service routes: Bearer JWT verified, but no user load / URI-RBAC,
    // so external machine callers always get JSON or 401.
    let sub_service_router = route::get_service_routes().layer(middleware::from_fn_with_state(
        id_token_signer.clone(),
        access_middleware_local_rs256,
    ));

    let api_prefix = &cfg.http.api_prefix;
    let legacy_routes = Router::new()
        .nest(api_prefix, sub_auth_router)
        .nest(api_prefix, sub_public_router)
        .nest(api_prefix, sub_service_router)
        .nest(api_prefix, route::get_revocation_routes())
        .layer(deployment_cors(&cfg));
    #[cfg_attr(not(feature = "openapi"), allow(unused_mut))]
    let mut app = Router::new()
        .merge(legacy_routes)
        .nest(
            api_prefix,
            Router::new()
                .route(
                    "/oidc/userinfo",
                    axum::routing::get(route::identity_userinfo::userinfo)
                        .post(route::identity_userinfo::userinfo),
                )
                .layer(middleware::from_fn_with_state(
                    app_state.clone(),
                    route::identity_browser::request_context,
                )),
        )
        .nest(
            api_prefix,
            route::identity_browser::get_routes(app_state.clone()),
        )
        .merge(get_no_prefix_routes().layer(middleware::from_fn_with_state(
            app_state.clone(),
            route::identity_browser::request_context,
        )))
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
        .fallback_service(webui::fallback(cfg.http.api_prefix.clone()));
    #[cfg(feature = "openapi")]
    {
        use crate::swagger_docs::doc::ApiDoc;
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        app = app.merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", ApiDoc::openapi()));
    }

    let workers = if cfg.identity.background_workers_enabled {
        match app_state.security_clock() {
            Ok(clock) => {
                use service::identity::{
                    logout_dispatcher::LogoutDispatcher, logout_transport::HttpsLogoutTransport,
                    repository::PgIdentityRepository, workers::IdentityWorkers,
                };
                let repository = Arc::new(PgIdentityRepository::new(
                    database.connection.clone(),
                    clock.clone(),
                ));
                let transport = Arc::new(HttpsLogoutTransport::new(
                    cfg.identity.clone(),
                    clock.clone(),
                ));
                match LogoutDispatcher::new(
                    repository.clone(),
                    clock.clone(),
                    id_token_signer,
                    cfg.clone(),
                    transport,
                ) {
                    Ok(dispatcher) => Some(IdentityWorkers::start(
                        Arc::new(dispatcher),
                        repository,
                        clock,
                        cfg.identity.clone(),
                    )),
                    Err(reason) => {
                        error!(
                            reason = reason.code(),
                            "identity workers unavailable; server not started"
                        );
                        return;
                    }
                }
            }
            Err(_) => {
                warn!("identity workers require the host security clock");
                None
            }
        }
    } else {
        None
    };
    let shutdown = async move {
        signal.await;
        if let Some(workers) = workers {
            workers.shutdown().await;
        }
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .unwrap_or_else(|e| error!("Application error: {e}"));
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
        crate::service::identity::account::initialize_provider(
            &self.database.connection,
            &self.issuer(),
            u64::try_from(chrono::Utc::now().timestamp_millis()).map_err(|_| {
                crate::domain::identity::error::IdentityError::DependencyUnavailable
            })?,
        )
        .await?;
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
        self.config.issuer()
    }

    /// Configured upper bound (days) for long-lived MCP access tokens.
    pub fn mcp_token_max_ttl_days(&self) -> u32 {
        self.config.oidc.mcp_token_max_ttl_days
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

    /// Historical bootstrap entries are compatibility references. Trust changes
    /// require an authenticated, explicit administrator operation.
    async fn init_oidc_bootstrap_clients(&self) -> result::Result<()> {
        for entry in &self.config.oidc.bootstrap_clients {
            if crate::entity::oauth_client::Entity::get_by_client_id(
                &self.database.connection,
                &entry.client_id,
            )
            .await?
            .is_some()
            {
                info!(client_id = %entry.client_id, "Historical bootstrap configuration preserved without changes");
            } else {
                tracing::warn!(client_id = %entry.client_id, "Bootstrap entry requires explicit governed application registration; no client was created");
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
                ..Default::default()
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
        let uri: http::Uri = "/api/v1/data?format=json&access_token=first&access_token=second"
            .parse()
            .unwrap();

        let redacted = redact_access_token(&uri);

        assert_eq!(
            redacted,
            "/api/v1/data?format=json&access_token=REDACTED&access_token=REDACTED"
        );
        assert!(!redacted.contains("first"));
        assert!(!redacted.contains("second"));
    }

    #[test]
    fn test_redact_access_token_leaves_uris_without_token_unchanged() {
        let without_query: http::Uri = "/api/v1/users".parse().unwrap();
        let without_token: http::Uri = "/api/v1/users?format=json".parse().unwrap();

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
    fn test_identity_protocol_logs_exclude_queries_bodies_and_credentials() {
        for path in [
            "/oidc/authorize",
            "/oidc/token",
            "/oidc/logout",
            "/api/v1/oidc/userinfo",
            "/custom/identity/login",
            "/custom/identity/invitations/accept",
            "/custom/identity/password-resets/complete",
            "/custom/developer/apps/7/secret-reset",
            "/custom/account/profile",
            "/custom/admin/invitations",
        ] {
            let uri: http::Uri = format!("{path}?code=secret-code&state=secret-state&nonce=secret-nonce&client_secret=secret-client&action_secret=secret-action&email=secret-person").parse().unwrap();
            assert_eq!(redact_access_token(&uri), path);
            assert!(
                should_skip_body_logging(&uri, &HeaderMap::new(), "/custom"),
                "{path}"
            );
        }
        let uri: http::Uri =
            "/public?%61ccess_token=secret&password=secret&code_verifier=secret&format=json"
                .parse()
                .unwrap();
        let redacted = redact_access_token(&uri);
        assert!(!redacted.contains("secret"));
        assert!(redacted.contains("format=json"));
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert(COOKIE, HeaderValue::from_static("flow=secret"));
        headers.insert("x-csrf-token", HeaderValue::from_static("secret"));
        assert!(redact_sensitive_headers(headers).is_empty());
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
            &"/api/v1/csrf".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(!should_skip_body_logging(
            &"/other/csrf".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(!should_skip_body_logging(
            &"/api/v1/users".parse().unwrap(),
            &headers,
            "/api/v1"
        ));
        assert!(should_skip_body_logging(
            &"/api/v1/test-upload".parse().unwrap(),
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
            &"/custom/csrf".parse().unwrap(),
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

    #[test]
    fn security_t25_csrf_and_logout_evidence_is_excluded_from_generic_logs() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-csrf-token",
            HeaderValue::from_static("signed-test-proof"),
        );
        assert!(!redact_sensitive_headers(headers.clone()).contains_key("x-csrf-token"));
        for path in ["/custom/csrf", "/custom/logout", "/oidc/logout"] {
            assert!(
                should_skip_body_logging(&path.parse().unwrap(), &headers, "/custom"),
                "{path}"
            );
        }
    }

    #[tokio::test]
    async fn test_invalid_http_development_cookie_policy_stops_before_database_init() {
        for wildcard_listener in [false, true] {
            let mut config = Config::default();
            config.auth.allow_insecure_loopback_cookies = true;
            config.database.url = "must-not-open-database".into();
            config.http.listen = "127.0.0.1:4000".parse().unwrap();
            if !wildcard_listener {
                config.oidc.issuer = "https://remote.example.test".into();
            }
            let listener = TcpListener::bind(if wildcard_listener {
                "0.0.0.0:0"
            } else {
                "127.0.0.1:0"
            })
            .await
            .unwrap();
            tokio::time::timeout(
                Duration::from_millis(100),
                serve(config, listener, std::future::pending()),
            )
            .await
            .unwrap();
        }
    }
}

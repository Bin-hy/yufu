use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::route::AppState;
use crate::service::network::NetworkService;

// ── Public endpoints (register_token auth) ────────────────────────────

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/easytier/config", get(get_config))
        .route("/api/v1/easytier/heartbeat", post(heartbeat))
}

// ── Admin endpoints (JWT auth) ────────────────────────────────────────

pub fn get_protected_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/easytier/admin/devices", get(list_devices))
        .route("/api/v1/easytier/admin/scrap", post(scrap_device))
        .route("/api/v1/easytier/admin/reclaim", post(reclaim))
}

// ── Query params ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ConfigQuery {
    robot_sn: String,
}

#[derive(Deserialize)]
struct ScrapQuery {
    robot_sn: String,
}

/// Response shape for config distribution.
#[derive(Serialize)]
struct ConfigResponse {
    network_name: String,
    network_secret: String,
    ipv4: String,
    listeners: Vec<String>,
    peers: Vec<String>,
    instance_name: String,
    hostname: String,
}

// ── Handlers ──────────────────────────────────────────────────────────

async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ConfigQuery>,
) -> impl IntoResponse {
    // Require easytier to be configured on this cobotsrv.
    let easytier_cfg = match &state.config.easytier {
        Some(cfg) => cfg,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "easytier not configured on this cobotsrv",
            )
                .into_response();
        }
    };

    // Validate register_token
    if !easytier_cfg.register_token.is_empty() {
        let token = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if token != easytier_cfg.register_token {
            return (StatusCode::UNAUTHORIZED, "invalid register token").into_response();
        }
    }

    let db = &state.database.connection;
    let mut pool = easytier_cfg.ip_pool.clone();
    pool.prefix = pool.resolve_prefix(&easytier_cfg.ipv4);

    match NetworkService::register_or_get_config(db, &q.robot_sn, &pool).await {
        Ok(record) => {
            let ip = record.virtual_ip.unwrap_or_default();

            let resp = ConfigResponse {
                network_name: easytier_cfg.network_name.clone(),
                network_secret: easytier_cfg.network_secret.clone(),
                ipv4: ip.clone(),
                listeners: easytier_cfg.listeners.clone(),
                peers: easytier_cfg.peers.clone(),
                instance_name: q.robot_sn.clone(),
                hostname: q.robot_sn,
            };

            tracing::info!(
                robot_sn = record.robot_sn,
                ip = ip,
                "easytier config distributed"
            );
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, "easytier config distribution failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("config distribution failed: {:?}", e),
            )
                .into_response()
        }
    }
}

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ConfigQuery>,
) -> impl IntoResponse {
    let easytier_cfg = match &state.config.easytier {
        Some(cfg) => cfg,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "easytier not configured on this cobotsrv",
            )
                .into_response();
        }
    };

    if !easytier_cfg.register_token.is_empty() {
        let token = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if token != easytier_cfg.register_token {
            return (StatusCode::UNAUTHORIZED, "invalid register token").into_response();
        }
    }

    NetworkService::heartbeat(&q.robot_sn);
    (StatusCode::OK, "ok").into_response()
}

async fn list_devices(State(state): State<AppState>) -> impl IntoResponse {
    match NetworkService::list_devices(&state.database.connection).await {
        Ok(devices) => (StatusCode::OK, Json(devices)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to list devices: {:?}", e),
        )
            .into_response(),
    }
}

async fn scrap_device(
    State(state): State<AppState>,
    Query(q): Query<ScrapQuery>,
) -> impl IntoResponse {
    match NetworkService::scrap_device(&state.database.connection, &q.robot_sn).await {
        Ok(()) => (StatusCode::OK, "device scrapped").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to scrap device: {:?}", e),
        )
            .into_response(),
    }
}

async fn reclaim(State(state): State<AppState>) -> impl IntoResponse {
    match NetworkService::reclaim_expired(&state.database.connection).await {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({ "reclaimed": count })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("reclaim failed: {:?}", e),
        )
            .into_response(),
    }
}

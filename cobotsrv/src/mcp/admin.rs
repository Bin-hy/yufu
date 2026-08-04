//! Admin/observability endpoints for the MCP service (status, sessions, audit).
//!
//! Mounted under `/mcp/admin/*` and guarded by `require_perm!("mcp:admin")` —
//! separate from the `mcp:access` permission that guards the MCP protocol
//! endpoint itself, so ordinary MCP users cannot read other users' audit
//! logs and sessions. These power the cobotsrv web console's MCP panel.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::entity::mcp_call_log::McpCallLogVO;
use crate::entity::{PageSelectQuery, PageSelectQueryResult};
use crate::result::Result;
use crate::route::tool::{ToolListResponse, aggregate_tools};
use crate::service::device::DeviceService;
use auth::require_perm;

use super::audit::{self, LogFilter};
use super::protocol::PROTOCOL_VERSION;
use super::session::SessionVO;

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/mcp/admin/info", get(info))
        .route("/mcp/admin/sessions", get(sessions))
        .route("/mcp/admin/logs", get(logs))
        .route("/mcp/admin/tools", get(tools))
        .layer(require_perm!("mcp:admin"))
}

#[derive(Serialize)]
struct McpInfo {
    protocol_version: &'static str,
    capabilities_tools_list_changed: bool,
    /// Audience clients must target (== this server's expected_audience). The web
    /// console uses this to mint a correctly-scoped access token.
    audience: String,
    /// Runtime settings (read-only; edit the [mcp] config and restart).
    call_timeout_ms: u64,
    online_tools: usize,
    online_devices: usize,
    calls_today: u64,
    success_today: u64,
}

/// GET /mcp/admin/info — server status + read-only settings + today's stats.
async fn info(State(state): State<AppState>) -> Result<Json<McpInfo>> {
    let db = &state.database.connection;
    let online = DeviceService::list_filtered(db, None, Some("ready"), None).await?;
    let online_tools = crate::route::tool::aggregate_tools(&online).len();

    let (calls_today, success_today) = audit::today_stats(db).await?;

    Ok(Json(McpInfo {
        protocol_version: PROTOCOL_VERSION,
        capabilities_tools_list_changed: false,
        audience: state.config.auth.expected_audience.clone(),
        call_timeout_ms: state.config.mcp.call_timeout_ms,
        online_tools,
        online_devices: online.len(),
        calls_today,
        success_today,
    }))
}

/// GET /mcp/admin/sessions — active MCP sessions.
async fn sessions(State(state): State<AppState>) -> Json<Vec<SessionVO>> {
    Json(state.mcp_sessions.list())
}

/// GET /mcp/admin/tools — same aggregated tool list as `GET /tools`, so the
/// web console's MCP panel doesn't need the `device:read` permission.
async fn tools(State(state): State<AppState>) -> Result<Json<ToolListResponse>> {
    let devices = DeviceService::list(&state.database.connection).await?;
    Ok(Json(ToolListResponse {
        tools: aggregate_tools(&devices),
    }))
}

#[derive(Deserialize)]
struct LogQuery {
    page_size: Option<u64>,
    page_number: Option<u64>,
    subject: Option<String>,
    tool: Option<String>,
    /// "ok" | "error" | omitted (all)
    result: Option<String>,
}

/// GET /mcp/admin/logs — paginated, filterable audit log.
async fn logs(
    State(state): State<AppState>,
    Query(q): Query<LogQuery>,
) -> Result<Json<PageSelectQueryResult<McpCallLogVO>>> {
    let page = PageSelectQuery {
        page_size: q.page_size.unwrap_or(20),
        page_number: q.page_number.unwrap_or(1),
    };
    let filter = LogFilter {
        subject: q.subject.filter(|s| !s.is_empty()),
        tool: q.tool.filter(|s| !s.is_empty()),
        is_error: match q.result.as_deref() {
            Some("error") => Some(true),
            Some("ok") => Some(false),
            _ => None,
        },
    };
    let res = audit::query(&state.database.connection, &page, &filter).await?;
    Ok(Json(res))
}

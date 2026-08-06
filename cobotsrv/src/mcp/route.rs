use std::time::Instant;

use auth::claims::AccessTokenClaims;
use auth::require_perm;
use axum::Extension;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header::HeaderValue},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::Value;

use crate::AppState;

use super::audit::CallRecord;
use super::session::{TouchResult, Transport};
use super::{
    handler,
    protocol::{JsonRpcRequest, JsonRpcResponse, error_codes},
    sse,
};

const SESSION_HEADER: &str = "Mcp-Session-Id";

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/mcp", get(sse_get).post(handle_post))
        .layer(require_perm!("mcp:access"))
}

/// GET /mcp — SSE stream. Refreshes the session if the client supplies one.
/// An unknown or foreign session id is rejected with 404 (same rule as POST).
async fn sse_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<AccessTokenClaims>,
) -> Response {
    if let Some(sid) = header_str(&headers, SESSION_HEADER) {
        match state.mcp_sessions.touch(&sid, &claims.sub, Transport::Sse) {
            TouchResult::Ok => {}
            TouchResult::Unknown | TouchResult::SubjectMismatch => {
                return StatusCode::NOT_FOUND.into_response();
            }
        }
    }
    sse::sse_handler().await.into_response()
}

async fn handle_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<AccessTokenClaims>,
    Json(body): Json<Value>,
) -> Response {
    let subject = claims.sub;

    // JSON-RPC 2.0 batch requests (arrays) are not supported.
    if body.is_array() {
        return Json(JsonRpcResponse::err(
            None,
            error_codes::INVALID_REQUEST,
            "batch requests are not supported",
        ))
        .into_response();
    }

    let req = match serde_json::from_value::<JsonRpcRequest>(body) {
        Ok(req) => req,
        Err(e) => {
            return Json(JsonRpcResponse::err(
                None,
                error_codes::INVALID_REQUEST,
                format!("invalid request: {e}"),
            ))
            .into_response();
        }
    };

    // Session lifecycle, per the MCP Streamable HTTP spec:
    // - Sessions are only created by an `initialize` *request* (one with a
    //   JSON-RPC `id`); initialize *notifications* never mint a session.
    // - A non-`initialize` request carrying an unknown `Mcp-Session-Id`
    //   is rejected with HTTP 404.
    // - A session id owned by a different subject is also rejected with 404
    //   (not 403) so the response doesn't reveal that the id exists.
    // The server-assigned id is returned via the Mcp-Session-Id response
    // header; every subsequent request carrying it refreshes `last_seen`.
    let mut minted_session: Option<String> = None;
    if let Some(sid) = header_str(&headers, SESSION_HEADER) {
        match state.mcp_sessions.touch(&sid, &subject, Transport::Http) {
            TouchResult::Ok => {}
            TouchResult::Unknown => {
                if req.method == "initialize" && req.id.is_some() {
                    // Client supplied its own id at initialize — honor it.
                    if !state.mcp_sessions.register(&sid, &subject, Transport::Http) {
                        return StatusCode::SERVICE_UNAVAILABLE.into_response();
                    }
                } else if req.method != "initialize" {
                    return StatusCode::NOT_FOUND.into_response();
                }
                // An initialize *notification* with an unknown id registers
                // nothing and falls through to the 202 ack below.
            }
            TouchResult::SubjectMismatch => {
                return StatusCode::NOT_FOUND.into_response();
            }
        }
    } else if req.method == "initialize" && req.id.is_some() {
        let id = uuid::Uuid::new_v4().to_string();
        if !state.mcp_sessions.register(&id, &subject, Transport::Http) {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        minted_session = Some(id);
    }

    // A request without an `id` is a JSON-RPC notification: per the spec the
    // server MUST NOT send a response. MCP clients emit `notifications/initialized`
    // (and others) this way; we accept and acknowledge with 202, no body.
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    // Capture audit fields before `dispatch` consumes `req`.
    let method = req.method.clone();
    // String ids stay unquoted; numbers fall back to their JSON form.
    let request_id = req.id.as_ref().map(|v| {
        v.as_str()
            .map(String::from)
            .unwrap_or_else(|| v.to_string())
    });
    let tool_name = if method == "tools/call" {
        req.params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from)
    } else {
        None
    };

    let started = Instant::now();
    let resp = handler::dispatch(&state, req).await;
    let duration_ms = started.elapsed().as_millis() as i64;

    // Audit `tools/call` invocations (fire-and-forget, never blocks the response).
    if method == "tools/call" {
        // Extract error message: protocol error or tool-level error content.
        let error_msg = resp.error.as_ref().map(|e| e.message.clone()).or_else(|| {
            // Check for tool-level isError flag
            if resp
                .result
                .as_ref()
                .and_then(|r| r.get("isError"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                resp.result
                    .as_ref()
                    .and_then(|r| r.get("content"))
                    .and_then(|c| c.get(0))
                    .and_then(|b| b.get("text"))
                    .and_then(|t| t.as_str())
                    .map(String::from)
            } else {
                None
            }
        });
        let rec = CallRecord {
            subject,
            method,
            tool_name,
            error_msg,
            duration_ms,
            request_id,
        };
        // Bounded channel feeding a single background writer task (started in
        // lib.rs). When full we drop the record — audit loss under overload is
        // acceptable, unbounded per-call task churn is not.
        if let Err(e) = state.mcp_audit_tx.try_send(rec) {
            tracing::warn!(error = %e, "mcp audit queue full/closed; dropping audit record");
        }
    }

    let mut response = Json(resp).into_response();
    if let Some(sid) = minted_session
        && let Ok(val) = HeaderValue::from_str(&sid)
    {
        response.headers_mut().insert(SESSION_HEADER, val);
    }
    response
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

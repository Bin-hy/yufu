use reqwest::StatusCode;
use serde_json::json;

use super::common::{insert_test_device, sign_test_token, start};

// ── 1. 无 token → 401 ────────────────────────────────────────────────────────

#[tokio::test]
async fn mcp_requires_auth() {
    let ctx = start().await;

    let resp = reqwest::Client::new()
        .post(format!("{}/mcp", ctx.base))
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── 2. initialize → serverInfo + capabilities ─────────────────────────────────

#[tokio::test]
async fn mcp_initialize_returns_capabilities() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "0.1"}
            }
        }))
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected error: {body}");
    let result = &body["result"];
    assert_eq!(result["protocolVersion"], "2025-03-26");
    assert_eq!(result["serverInfo"]["name"], "cobotrix-mcp");
    assert!(result["capabilities"]["tools"].is_object());
}

// ── 3. ping → {} ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn mcp_ping() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"2.0","id":2,"method":"ping"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected error: {body}");
    assert!(body["result"].is_object());
}

// ── 4. tools/list 空库 → {tools:[]} ──────────────────────────────────────────

#[tokio::test]
async fn mcp_tools_list_empty() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected error: {body}");
    let tools = body["result"]["tools"].as_array().unwrap();
    assert!(
        tools.is_empty(),
        "expected empty tools list, got: {tools:?}"
    );
}

// ── 5. tools/list 有在线设备 → McpTool[] with inputSchema ────────────────────

#[tokio::test]
async fn mcp_tools_list_with_online_device() {
    let ctx = start().await;
    insert_test_device(&ctx.db, "cobotlet-test", "device-01", "move", true).await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"2.0","id":4,"method":"tools/list"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected error: {body}");
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1, "expected 1 tool, got: {tools:?}");

    let tool = &tools[0];
    assert_eq!(tool["name"], "move");
    // MCP spec: inputSchema must be an object
    assert_eq!(tool["inputSchema"]["type"], "object");
    // Confirm field name is camelCase "inputSchema", not snake_case
    assert!(
        tool.get("input_schema").is_none(),
        "should use inputSchema not input_schema"
    );
}

// ── 6. tools/list 离线设备不暴露 ──────────────────────────────────────────────

#[tokio::test]
async fn mcp_tools_list_excludes_offline_devices() {
    let ctx = start().await;
    insert_test_device(&ctx.db, "cobotlet-off", "device-02", "grip", false).await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"2.0","id":5,"method":"tools/list"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    let tools = body["result"]["tools"].as_array().unwrap();
    assert!(
        tools.is_empty(),
        "offline device tools should not appear, got: {tools:?}"
    );
}

// ── 7. tools/call 工具不存在 → isError:true ──────────────────────────────────

#[tokio::test]
async fn mcp_tools_call_tool_not_found() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {"name": "nonexistent_tool", "arguments": {}}
        }))
        .await;

    assert_eq!(status, StatusCode::OK);
    // tools/call "not found" is a tool-level error (isError:true), not a protocol error
    assert!(
        body.get("error").is_none(),
        "should be tool-level error: {body}"
    );
    assert_eq!(body["result"]["isError"], true);
    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("nonexistent_tool"),
        "error message should mention the tool name: {text}"
    );
}

// ── 8. tools/call 有在线设备但 MQTT 未配置 → JSON-RPC InternalError ───────────

#[tokio::test]
async fn mcp_tools_call_no_mqtt() {
    let ctx = start().await;
    // Insert an online device so we get past the "no provider" check
    insert_test_device(&ctx.db, "cobotlet-mqtt", "device-03", "move", true).await;

    let (status, body) = ctx
        .mcp_post(json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {"name": "move", "arguments": {"x": 1.0}}
        }))
        .await;

    assert_eq!(status, StatusCode::OK);
    // MQTT missing → protocol-level InternalError
    let error = body.get("error").expect("expected protocol error: {body}");
    assert_eq!(error["code"], -32603);
    assert!(
        error["message"].as_str().unwrap().contains("mqtt"),
        "error should mention mqtt: {error}"
    );
}

// ── 9. 未知方法 → MethodNotFound -32601 ──────────────────────────────────────

#[tokio::test]
async fn mcp_unknown_method() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"2.0","id":8,"method":"unknown/method"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    let error = body.get("error").expect("expected error: {body}");
    assert_eq!(error["code"], -32601);
}

// ── 10. tools/call 无 params → InvalidParams -32602 ──────────────────────────

#[tokio::test]
async fn mcp_tools_call_missing_params() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"2.0","id":9,"method":"tools/call"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    let error = body.get("error").expect("expected error: {body}");
    assert_eq!(error["code"], -32602);
}

// ── 11. jsonrpc 版本非 "2.0" → InvalidRequest -32600 ────────────────────────

#[tokio::test]
async fn mcp_jsonrpc_invalid_version() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!({"jsonrpc":"1.0","id":10,"method":"ping"}))
        .await;

    assert_eq!(status, StatusCode::OK);
    let error = body.get("error").expect("expected error: {body}");
    assert_eq!(error["code"], -32600, "expected INVALID_REQUEST: {error}");
    assert!(
        error["message"].as_str().unwrap().contains("2.0"),
        "error should mention 2.0: {error}"
    );
}

// ── 12. 批量请求 → InvalidRequest -32600 ─────────────────────────────────────

#[tokio::test]
async fn mcp_batch_request_rejected() {
    let ctx = start().await;

    let (status, body) = ctx
        .mcp_post(json!([
            {"jsonrpc":"2.0","id":1,"method":"ping"},
            {"jsonrpc":"2.0","id":2,"method":"ping"}
        ]))
        .await;

    assert_eq!(status, StatusCode::OK);
    let error = body.get("error").expect("expected error: {body}");
    assert_eq!(error["code"], -32600, "expected INVALID_REQUEST: {error}");
    assert!(
        error["message"].as_str().unwrap().contains("batch"),
        "error should mention batch: {error}"
    );
}

// ── 13. 通知（无 id）→ 202 Accepted，无响应体 ────────────────────────────────
//
// JSON-RPC notifications (requests without `id`, e.g. `notifications/initialized`)
// MUST NOT receive a response. We acknowledge with 202 and an empty body, rather
// than replying with a (spec-violating) error response.

#[tokio::test]
async fn mcp_notification_gets_no_response() {
    let ctx = start().await;

    let resp = reqwest::Client::new()
        .post(format!("{}/mcp", ctx.base))
        .header("Authorization", format!("Bearer {}", ctx.token))
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = resp.text().await.unwrap();
    assert!(
        body.is_empty(),
        "notification must get an empty body, got: {body}"
    );
}

// ── 14. GET /mcp → 200 text/event-stream ─────────────────────────────────────
//
// SSE connections never close the body, so we read HTTP response headers via a
// raw TCP stream instead of reqwest (whose timeout covers the full body).

#[tokio::test]
async fn mcp_sse_endpoint_alive() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let ctx = start().await;

    let addr = ctx.base.strip_prefix("http://").unwrap().to_string();
    let mut stream = tokio::net::TcpStream::connect(&addr).await.unwrap();

    let req = format!(
        "GET /mcp HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n",
        token = ctx.token,
    );
    stream.write_all(req.as_bytes()).await.unwrap();

    // Read until we have the full response header section (ends with \r\n\r\n).
    let mut raw = Vec::<u8>::new();
    let mut tmp = [0u8; 1024];
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let n = stream.read(&mut tmp).await.expect("read failed");
            raw.extend_from_slice(&tmp[..n]);
            if raw.windows(4).any(|w| w == b"\r\n\r\n") || n == 0 {
                break;
            }
        }
    })
    .await
    .expect("SSE endpoint did not send headers within 5 seconds");

    let headers = String::from_utf8_lossy(&raw);
    assert!(
        headers.starts_with("HTTP/1.1 200"),
        "expected HTTP 200, got: {headers}"
    );
    assert!(
        headers.contains("text/event-stream"),
        "expected text/event-stream content-type, got: {headers}"
    );
}

// ── 15. admin: GET /mcp/admin/info ────────────────────────────────────────────────

#[tokio::test]
async fn mcp_admin_info_returns_status() {
    let ctx = start().await;
    insert_test_device(&ctx.db, "cobotlet-0", "dev-1", "do_thing", true).await;

    let (status, body) = ctx.get_authed("/mcp/admin/info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["protocol_version"], "2025-03-26");
    assert_eq!(body["online_tools"], 1);
    assert_eq!(body["online_devices"], 1);
}

#[tokio::test]
async fn mcp_admin_info_requires_auth() {
    let ctx = start().await;
    let resp = reqwest::Client::new()
        .get(format!("{}/mcp/admin/info", ctx.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── 16. admin: GET /mcp/admin/logs records a tools/call ───────────────────────────

#[tokio::test]
async fn mcp_admin_logs_records_tools_call() {
    let ctx = start().await;

    // A tools/call for a non-existent tool still produces an audited record.
    let (status, _) = ctx
        .mcp_post(json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"ghost_tool","arguments":{}}
        }))
        .await;
    assert_eq!(status, StatusCode::OK);

    // Audit write is fire-and-forget (spawned); poll until it lands.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let row = loop {
        let (s, body) = ctx
            .get_authed("/mcp/admin/logs?page_size=10&page_number=1")
            .await;
        assert_eq!(s, StatusCode::OK);
        let list = body["data_list"].as_array().cloned().unwrap_or_default();
        if let Some(r) = list.into_iter().find(|r| r["tool_name"] == "ghost_tool") {
            break r;
        }
        if std::time::Instant::now() >= deadline {
            panic!("audit record for tools/call did not appear within 5s");
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };

    assert_eq!(row["method"], "tools/call");
    assert_eq!(row["subject"], "1"); // sub from the test token
    assert!(
        row["error_msg"].is_string(),
        "error_msg should be present for failed call"
    ); // no provider for ghost_tool
}

// ── 17. admin: GET /mcp/admin/sessions tracks an initialize ───────────────────────

#[tokio::test]
async fn mcp_admin_sessions_tracks_initialize() {
    let ctx = start().await;

    let (status, _) = ctx
        .mcp_post(json!({
            "jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-03-26","capabilities":{}}
        }))
        .await;
    assert_eq!(status, StatusCode::OK);

    let (s, body) = ctx.get_authed("/mcp/admin/sessions").await;
    assert_eq!(s, StatusCode::OK);
    let sessions = body.as_array().cloned().unwrap_or_default();
    assert_eq!(sessions.len(), 1, "expected one active session: {body}");
    assert_eq!(sessions[0]["transport"], "http");
}

// ── 18. 有效 JWT 但无 mcp:access → POST /mcp 403 ─────────────────────────────

#[tokio::test]
async fn mcp_requires_mcp_access_permission() {
    let ctx = start().await;
    let token = sign_test_token(&ctx.signing_pem, &["task:execute", "device:read"]);

    // 403 body is plain text ("Missing permission: ..."), so don't parse JSON.
    let resp = reqwest::Client::new()
        .post(format!("{}/mcp", ctx.base))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── 19. mcp:access 但无 mcp:admin → /mcp/admin/* 403, POST /mcp 仍可用 ──────

#[tokio::test]
async fn mcp_admin_requires_mcp_admin_permission() {
    let ctx = start().await;
    let token = sign_test_token(&ctx.signing_pem, &["mcp:access"]);

    let (status, _) = ctx.get_authed_with(&token, "/mcp/admin/info").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The same token still works on the MCP protocol endpoint.
    let (status, body) = ctx
        .mcp_post_with(
            &token,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {"name": "test-client", "version": "0.1"}
                }
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected error: {body}");
}

// ── 20. 未知 Mcp-Session-Id 的非 initialize 请求 → 404 ───────────────────────

#[tokio::test]
async fn mcp_unknown_session_id_returns_404() {
    let ctx = start().await;

    let resp = reqwest::Client::new()
        .post(format!("{}/mcp", ctx.base))
        .header("Authorization", format!("Bearer {}", ctx.token))
        .header("Mcp-Session-Id", uuid::Uuid::new_v4().to_string())
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

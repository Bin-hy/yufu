use reqwest::StatusCode;
use serde_json::json;

use super::common::{insert_test_device, start};

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
    assert!(tools.is_empty(), "expected empty tools list, got: {tools:?}");
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
    assert!(tool.get("input_schema").is_none(), "should use inputSchema not input_schema");
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
    assert!(body.get("error").is_none(), "should be tool-level error: {body}");
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

// ── 12. jsonrpc 版本非 "2.0" → InvalidRequest -32600 ────────────────────────

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

// ── 13. 批量请求 → InvalidRequest -32600 ─────────────────────────────────────

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

// ── 11. GET /mcp → 200 text/event-stream ─────────────────────────────────────
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

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: &str = "2025-03-26";

// ─── JSON-RPC 2.0 ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

impl JsonRpcResponse {
    pub fn ok(id: Option<serde_json::Value>, result: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    pub fn err(id: Option<serde_json::Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// ─── MCP initialize ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: serde_json::Value,
    pub client_info: Option<Implementation>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: &'static str,
    pub capabilities: ServerCapabilities,
    pub server_info: Implementation,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    pub list_changed: bool,
}

// ─── MCP tools ───────────────────────────────────────────────────────────────

/// A tool descriptor in MCP format. `inputSchema` must be a JSON Schema object
/// (at minimum `{"type":"object"}`).
#[derive(Debug, Serialize)]
pub struct McpTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    pub tools: Vec<McpTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

/// Result of a tools/call request. `content` mirrors the MCP spec content block
/// array; `isError` indicates whether the call failed at the tool level (as
/// opposed to a protocol error).
#[derive(Debug, Serialize)]
pub struct CallToolResult {
    pub content: Vec<serde_json::Value>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

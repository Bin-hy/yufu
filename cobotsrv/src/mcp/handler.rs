use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entity::device::Capability;
use crate::mqtt::Task;
use crate::service::device::DeviceService;

use super::protocol::{
    CallToolParams, CallToolResult, Implementation, InitializeParams, InitializeResult,
    JsonRpcRequest, JsonRpcResponse, ListToolsResult, McpTool, PROTOCOL_VERSION,
    ServerCapabilities, ToolsCapability, error_codes,
};

pub struct DispatchResult {
    pub response: JsonRpcResponse,
    pub device_id: Option<String>,
    pub cobotlet_alias: Option<String>,
}

pub async fn dispatch(state: &AppState, req: JsonRpcRequest) -> DispatchResult {
    if req.jsonrpc != "2.0" {
        return DispatchResult {
            response: JsonRpcResponse::err(
                req.id,
                error_codes::INVALID_REQUEST,
                format!("jsonrpc must be \"2.0\", got \"{}\"", req.jsonrpc),
            ),
            device_id: None,
            cobotlet_alias: None,
        };
    }
    match req.method.as_str() {
        "initialize" => DispatchResult {
            response: handle_initialize(req.id, req.params),
            device_id: None,
            cobotlet_alias: None,
        },
        "ping" => DispatchResult {
            response: JsonRpcResponse::ok(req.id, json!({})),
            device_id: None,
            cobotlet_alias: None,
        },
        "tools/list" => DispatchResult {
            response: handle_tools_list(state, req.id).await,
            device_id: None,
            cobotlet_alias: None,
        },
        "tools/call" => handle_tools_call(state, req.id, req.params).await,
        other => DispatchResult {
            response: JsonRpcResponse::err(
                req.id,
                error_codes::METHOD_NOT_FOUND,
                format!("method not found: {other}"),
            ),
            device_id: None,
            cobotlet_alias: None,
        },
    }
}

fn handle_initialize(
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> JsonRpcResponse {
    let init_params = params.and_then(|p| serde_json::from_value::<InitializeParams>(p).ok());

    if let Some(ref p) = init_params
        && p.protocol_version != PROTOCOL_VERSION
    {
        tracing::warn!(
            client_version = %p.protocol_version,
            server_version = PROTOCOL_VERSION,
            "mcp: client protocol version differs from server"
        );
    }

    let client_info = init_params.and_then(|p| p.client_info);
    tracing::info!(
        client = ?client_info.as_ref().map(|c| &c.name),
        "mcp: initialize"
    );

    JsonRpcResponse::ok(
        id,
        InitializeResult {
            protocol_version: PROTOCOL_VERSION,
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false, // Phase 2
                },
            },
            server_info: Implementation {
                name: "cobotrix-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        },
    )
}

async fn handle_tools_list(state: &AppState, id: Option<serde_json::Value>) -> JsonRpcResponse {
    let devices =
        match DeviceService::list_filtered(&state.database.connection, None, None, None).await {
            Ok(d) => d,
            Err(e) => {
                return JsonRpcResponse::err(id, error_codes::INTERNAL_ERROR, e.to_string());
            }
        };

    let mut seen: std::collections::HashMap<String, McpTool> = std::collections::HashMap::new();
    for device in &devices {
        if device.state != "ready" {
            continue;
        }
        let caps: Vec<Capability> =
            serde_json::from_value(device.capabilities.clone()).unwrap_or_default();
        for cap in caps {
            seen.entry(cap.name.clone()).or_insert_with(|| McpTool {
                name: cap.name.clone(),
                description: cap.description.clone(),
                // MCP spec: inputSchema must be a JSON Schema object
                input_schema: cap
                    .input_schema
                    .unwrap_or_else(|| json!({"type": "object"})),
            });
        }
    }

    let mut tools: Vec<McpTool> = seen.into_values().collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));

    JsonRpcResponse::ok(
        id,
        ListToolsResult {
            tools,
            next_cursor: None,
        },
    )
}

async fn handle_tools_call(
    state: &AppState,
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> DispatchResult {
    let Some(params) = params else {
        return DispatchResult {
            response: JsonRpcResponse::err(
                id,
                error_codes::INVALID_PARAMS,
                "params required for tools/call",
            ),
            device_id: None,
            cobotlet_alias: None,
        };
    };

    let mut call_params: CallToolParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return DispatchResult {
                response: JsonRpcResponse::err(id, error_codes::INVALID_PARAMS, e.to_string()),
                device_id: None,
                cobotlet_alias: None,
            };
        }
    };

    // Extract _meta.cobotlet_alias from arguments to allow optional cobotlet targeting.
    // _meta is stripped before forwarding so device services don't see it.
    let cobotlet_alias_hint = call_params
        .arguments
        .as_mut()
        .and_then(|args| args.as_object_mut())
        .and_then(|obj| obj.remove("_meta"))
        .and_then(|meta| {
            meta.get("cobotlet_alias")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    let devices = match DeviceService::list_filtered(
        &state.database.connection,
        cobotlet_alias_hint.as_deref(),
        None,
        Some(&call_params.name),
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            return DispatchResult {
                response: JsonRpcResponse::err(id, error_codes::INTERNAL_ERROR, e.to_string()),
                device_id: None,
                cobotlet_alias: None,
            };
        }
    };

    let device = match devices.iter().find(|d| d.state == "ready") {
        Some(d) => d,
        None => {
            let msg = match &cobotlet_alias_hint {
                Some(alias) => format!(
                    "cobotlet '{}' is offline or does not provide tool '{}'",
                    alias, call_params.name
                ),
                None => format!("no online provider for tool '{}'", call_params.name),
            };
            return DispatchResult {
                response: JsonRpcResponse::ok(
                    id,
                    CallToolResult {
                        content: vec![json!({"type": "text", "text": msg})],
                        is_error: true,
                    },
                ),
                device_id: None,
                cobotlet_alias: cobotlet_alias_hint,
            };
        }
    };

    let Some(mqtt) = &state.mqtt else {
        return DispatchResult {
            response: JsonRpcResponse::err(
                id,
                error_codes::INTERNAL_ERROR,
                "mqtt not configured on this server",
            ),
            device_id: Some(device.device_id.clone()),
            cobotlet_alias: Some(device.cobotlet_alias.clone()),
        };
    };

    // list_filtered already confirmed this device has the requested capability.
    let device_id = Some(device.device_id.clone());

    let timeout_ms = state.config.mcp.call_timeout_ms;
    let task = Task {
        id: Uuid::new_v4().to_string(),
        name: call_params.name.clone(),
        device_id,
        headers: Default::default(),
        input: call_params.arguments,
        timeout_ms: Some(timeout_ms),
    };

    tracing::info!(
        tool = %call_params.name,
        cobotlet = %device.cobotlet_alias,
        "mcp: tools/call dispatch"
    );

    let selected_device_id = device.device_id.clone();
    let selected_alias = device.cobotlet_alias.clone();

    match mqtt
        .submit(
            &device.cobotlet_alias,
            task,
            Duration::from_millis(timeout_ms),
        )
        .await
    {
        Ok(result) => {
            let content: Vec<serde_json::Value> = result
                .content
                .iter()
                .map(|b| json!({"type": b.r#type, "text": b.text}))
                .collect();
            DispatchResult {
                response: JsonRpcResponse::ok(
                    id,
                    CallToolResult {
                        content,
                        is_error: result.is_error,
                    },
                ),
                device_id: Some(selected_device_id),
                cobotlet_alias: Some(selected_alias),
            }
        }
        Err(e) => DispatchResult {
            response: JsonRpcResponse::err(id, error_codes::INTERNAL_ERROR, e.to_string()),
            device_id: Some(selected_device_id),
            cobotlet_alias: Some(selected_alias),
        },
    }
}

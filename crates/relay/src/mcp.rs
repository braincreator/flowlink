// MCP Server — JSON-RPC endpoint for AI model interaction with agents.
// POST /mcp — accepts MCP protocol requests (initialize, tools/list, tools/call).

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::server::AppState;

// ═══════════════════════════════════════════════
// MCP Protocol Types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct McpRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Serialize)]
pub struct McpResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Serialize, Clone)]
struct McpError {
    code: i32,
    message: String,
}

// ═══════════════════════════════════════════════
// Tool Definitions
// ═══════════════════════════════════════════════

fn mcp_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "flowlink_agents",
            "description": "List all connected flowlink agents. Returns ID, hostname, OS, arch, connection status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["all", "online"],
                        "default": "online",
                        "description": "Filter by status"
                    }
                }
            }
        }),
        json!({
            "name": "flowlink_exec",
            "description": "Execute a shell command on a remote machine via a flowlink agent. Returns stdout, stderr, exit code.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "command"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID (from flowlink_agents) or label (hostname)" },
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "timeout": { "type": "integer", "default": 120, "description": "Timeout in seconds" },
                    "workdir": { "type": "string", "description": "Working directory (optional)" }
                }
            }
        }),
        json!({
            "name": "flowlink_read",
            "description": "Read a file from a remote machine via a flowlink agent.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "path"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "path": { "type": "string", "description": "File path" }
                }
            }
        }),
        json!({
            "name": "flowlink_write",
            "description": "Write a file to a remote machine via a flowlink agent.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "path", "content"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "path": { "type": "string", "description": "File path" },
                    "content": { "type": "string", "description": "File content" }
                }
            }
        }),
        json!({
            "name": "flowlink_list",
            "description": "List files/directories on a remote machine.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "path"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "path": { "type": "string", "description": "Directory path" }
                }
            }
        }),
        json!({
            "name": "flowlink_sysinfo",
            "description": "Get system information from a remote machine (CPU, RAM, OS, disk, network).",
            "inputSchema": {
                "type": "object",
                "required": ["agent"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" }
                }
            }
        }),
    ]
}

// ═══════════════════════════════════════════════
// Handler
// ═══════════════════════════════════════════════

pub async fn handle_mcp(
    State(state): State<AppState>,
    Json(req): Json<McpRequest>,
) -> axum::response::Response {
    match req.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "flowlink-relay", "version": "0.1.0" }
            });
            mcp_ok(req.id, result).into_response()
        }

        "notifications/initialized" => {
            axum::http::StatusCode::NO_CONTENT.into_response()
        }

        "tools/list" => {
            mcp_ok(req.id, json!({ "tools": mcp_tools() })).into_response()
        }

        "tools/call" => handle_tools_call(state, req).await,

        _ => mcp_err(req.id, -32601, format!("method not found: {}", req.method)).into_response(),
    }
}

async fn handle_tools_call(state: AppState, req: McpRequest) -> axum::response::Response {
    let params = match req.params {
        Some(Value::Object(map)) => map,
        _ => return mcp_err(req.id, -32602, "invalid params").into_response(),
    };

    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return mcp_err(req.id, -32602, "missing tool name").into_response(),
    };

    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "flowlink_agents" => mcp_agents(&state, req.id, &args),
        "flowlink_exec" => mcp_exec(&state, req.id, &args).await,
        "flowlink_read" => mcp_read(&state, req.id, &args).await,
        "flowlink_write" => mcp_write(&state, req.id, &args).await,
        "flowlink_list" => mcp_list(&state, req.id, &args).await,
        "flowlink_sysinfo" => mcp_sysinfo(&state, req.id, &args).await,
        _ => mcp_err(req.id, -32602, format!("unknown tool: {name}")).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Tool Implementations
// ═══════════════════════════════════════════════

fn mcp_agents(state: &AppState, id: Option<Value>, _args: &Value) -> axum::response::Response {
    let agents = state.pool.list();
    if agents.is_empty() {
        return mcp_ok(id, json!({
            "content": [{ "type": "text", "text": "No connected flowlink agents." }]
        })).into_response();
    }

    let mut text = format!("Connected agents: {}\n\n", agents.len());
    for a in &agents {
        let online = chrono::Utc::now().timestamp() - a.last_heartbeat < 120;
        let status = if online { "🟢" } else { "🔴" };
        text.push_str(&format!(
            "{} {} ({}, {}/{}) — last heartbeat: {}s ago\n",
            status, a.hostname, a.agent_id, a.os, a.arch,
            chrono::Utc::now().timestamp() - a.last_heartbeat
        ));
    }

    mcp_ok(id, json!({ "content": [{ "type": "text", "text": text }] })).into_response()
}

async fn mcp_exec(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };
    let command = match get_arg(args, "command") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "command: required").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let timeout: i32 = args.get("timeout").and_then(|v| v.as_i64()).unwrap_or(120) as i32;
    let workdir = args.get("workdir").and_then(|v| v.as_str()).map(String::from);

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::ExecRequest)
        .with_agent_id(&resolved)
        .with_payload(flowlink_core::ExecRequestPayload {
            command,
            shell: None,
            env: None,
            dir: workdir,
            timeout_sec: timeout,
            request_id: flowlink_core::request_id(),
        });

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => mcp_ok(id, json!({
            "content": [{ "type": "text", "text": format!("Command sent to agent {}", resolved) }]
        })).into_response(),
        Err(e) => mcp_err(id, -32603, format!("agent error: {e}")).into_response(),
    }
}

async fn mcp_read(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let (agent_id, path) = match (get_arg(args, "agent"), get_arg(args, "path")) {
        (Some(a), Some(p)) => (a, p),
        _ => return mcp_err(id, -32602, "agent and path: required").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::FileRead)
        .with_agent_id(&resolved)
        .with_payload(serde_json::json!({ "path": path, "request_id": flowlink_core::request_id() }));

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => mcp_ok(id, json!({
            "content": [{ "type": "text", "text": format!("Read request sent: {path}") }]
        })).into_response(),
        Err(e) => mcp_err(id, -32603, e.to_string()).into_response(),
    }
}

async fn mcp_write(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let (agent_id, path) = match (get_arg(args, "agent"), get_arg(args, "path")) {
        (Some(a), Some(p)) => (a, p),
        _ => return mcp_err(id, -32602, "agent, path, content: required").into_response(),
    };
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::FileWrite)
        .with_agent_id(&resolved)
        .with_payload(serde_json::json!({
            "path": path,
            "content": content,
            "encoding": "utf8",
            "request_id": flowlink_core::request_id()
        }));

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => mcp_ok(id, json!({
            "content": [{ "type": "text", "text": format!("✅ File written: {path}") }]
        })).into_response(),
        Err(e) => mcp_err(id, -32603, e.to_string()).into_response(),
    }
}

async fn mcp_list(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let (agent_id, path) = match (get_arg(args, "agent"), get_arg(args, "path")) {
        (Some(a), Some(p)) => (a, p),
        _ => return mcp_err(id, -32602, "agent and path: required").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::FileList)
        .with_agent_id(&resolved)
        .with_payload(serde_json::json!({ "path": path, "request_id": flowlink_core::request_id() }));

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => mcp_ok(id, json!({
            "content": [{ "type": "text", "text": format!("List request sent: {path}") }]
        })).into_response(),
        Err(e) => mcp_err(id, -32603, e.to_string()).into_response(),
    }
}

async fn mcp_sysinfo(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::SysInfo)
        .with_agent_id(&resolved)
        .with_payload(serde_json::json!({ "request_id": flowlink_core::request_id() }));

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => mcp_ok(id, json!({
            "content": [{ "type": "text", "text": format!("Sysinfo request sent to {resolved}") }]
        })).into_response(),
        Err(e) => mcp_err(id, -32603, e.to_string()).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

fn get_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn resolve_agent(pool: &crate::pool::AgentPool, selector: &str) -> Option<String> {
    // Try by ID first
    if pool.get(selector).is_some() {
        return Some(selector.to_string());
    }
    // Try by hostname
    for agent in pool.list() {
        if agent.hostname == selector {
            return Some(agent.agent_id);
        }
    }
    None
}

fn mcp_ok(id: Option<Value>, result: Value) -> Json<McpResponse> {
    Json(McpResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    })
}

fn mcp_err(id: Option<Value>, code: i32, message: impl Into<String>) -> Json<McpResponse> {
    let message = message.into();
    Json(McpResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(McpError { code, message }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp_req(method: &str, id: Option<Value>, params: Option<Value>) -> McpRequest {
        McpRequest { jsonrpc: "2.0".into(), id, method: method.into(), params }
    }

    #[test]
    fn test_mcp_request_deserialize() {
        let json = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"});
        let req: McpRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(Value::Number(1.into())));
    }

    #[test]
    fn test_mcp_request_missing_params_defaults() {
        let json = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list"});
        let req: McpRequest = serde_json::from_value(json).unwrap();
        assert!(req.params.is_none());
    }

    #[test]
    fn test_mcp_tools_list_content() {
        let tools = mcp_tools();
        assert!(!tools.is_empty());
        let names: Vec<_> = tools.iter().filter_map(|t| t.get("name").and_then(|n| n.as_str())).collect();
        assert!(names.contains(&"flowlink_agents"));
        assert!(names.contains(&"flowlink_exec"));
    }

    #[test]
    fn test_mcp_ok_response() {
        let resp = mcp_ok(Some(Value::Number(1.into())), json!({"test": true}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_mcp_err_response() {
        let resp = mcp_err(Some(Value::Number(1.into())), -32601, "method not found");
        assert!(resp.result.is_none());
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "method not found");
    }

    #[test]
    fn test_get_arg() {
        let args = json!({"agent": "a1", "command": "ls"});
        assert_eq!(get_arg(&args, "agent"), Some("a1".into()));
        assert_eq!(get_arg(&args, "missing"), None);
    }

    #[test]
    fn test_mcp_request_with_params() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "flowlink_agents", "arguments": {}}
        });
        let req: McpRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.method, "tools/call");
        assert!(req.params.is_some());
    }
}

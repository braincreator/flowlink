//! MCP Server — JSON-RPC endpoint for AI model interaction with agents.
//! POST /mcp — accepts MCP protocol requests (initialize, tools/list, tools/call).
//!
//! This is the standalone crate version — uses `McpState` instead of relay's `AppState`.

use axum::{
    extract::{State, Extension},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

// ═══════════════════════════════════════════════
// MCP Protocol Types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct McpRequest {
    #[allow(dead_code)]
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
// State
// ═══════════════════════════════════════════════

/// MCP API state — self-contained, no dependency on relay.
#[derive(Clone)]
pub struct McpState {
    /// Database pool (optional — graceful degradation without)
    pub db: Option<Arc<flowlink_db::DbPool>>,
    /// Billing engine for plan gating
    pub billing: Option<Arc<flowlink_billing::BillingEngine>>,
}

// ═══════════════════════════════════════════════
// API Key types
// ═══════════════════════════════════════════════

/// API key identity — extracted from validated API key
#[derive(Debug, Clone)]
pub struct KeyIdentity {
    pub key_id: uuid::Uuid,
    pub account_id: String,
    pub org_id: String,
    pub scopes: Vec<String>,
}

impl KeyIdentity {
    /// Check if key has required scope
    pub fn can_call(&self, scope: &str) -> bool {
        self.scopes.contains(&"*".to_string()) || self.scopes.contains(&scope.to_string())
    }
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
        json!({
            "name": "flowlink_kill",
            "description": "Emergency kill switch — immediately disconnect an agent from the relay.",
            "inputSchema": {
                "type": "object",
                "required": ["agent"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label to kill" },
                    "reason": { "type": "string", "description": "Reason for killing the agent (audited)" }
                }
            }
        }),
        json!({
            "name": "flowlink_deregister",
            "description": "Permanently remove an agent — disconnects WS, removes from pool and database.",
            "inputSchema": {
                "type": "object",
                "required": ["agent"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID to deregister" },
                    "reason": { "type": "string", "description": "Reason for deregistration (audited)" }
                }
            }
        }),
        json!({
            "name": "flowlink_health",
            "description": "Health check for a connected agent.",
            "inputSchema": {
                "type": "object",
                "required": ["agent"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" }
                }
            }
        }),
        json!({
            "name": "flowlink_config_update",
            "description": "Update an agent's configuration remotely.",
            "inputSchema": {
                "type": "object",
                "required": ["agent"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "read_only": { "type": "boolean", "description": "Set agent to read-only mode" },
                    "label": { "type": "string", "description": "Update agent label" },
                    "work_dir": { "type": "string", "description": "Set agent working directory" },
                    "approval_mode": { "type": "string", "enum": ["auto", "soft_ask", "hard_ask"], "description": "Set approval mode" },
                    "approval_timeout_sec": { "type": "integer", "description": "Seconds before auto-rejecting pending approvals" }
                }
            }
        }),
        json!({
            "name": "flowlink_approve",
            "description": "Approve or reject a pending command from an agent.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "action"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "action": { "type": "string", "enum": ["list", "approve", "reject", "approve_always"], "description": "Action to perform" },
                    "request_id": { "type": "string", "description": "Approval request ID" },
                    "reason": { "type": "string", "description": "Reason for the decision" },
                    "approver": { "type": "string", "description": "Who is making this decision" }
                }
            }
        }),
        json!({
            "name": "flowlink_policy",
            "description": "Dynamically manage agent policy rules at runtime.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "action"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "action": { "type": "string", "enum": ["list", "add_allow", "add_deny", "remove"], "description": "Policy action" },
                    "pattern": { "type": "string", "description": "Glob pattern for the rule" }
                }
            }
        }),
    ]
}

// ═══════════════════════════════════════════════
// Handler
// ═══════════════════════════════════════════════

pub async fn handle_mcp(
    Extension(state): Extension<std::sync::Arc<McpState>>,
    headers: HeaderMap,
    Json(req): Json<McpRequest>,
) -> axum::response::Response {
    process_mcp_http(&state, headers, req).await
}

/// Internal MCP processor (shared between HTTP and Streamable HTTP transports)
pub async fn process_mcp_http(
    state: &McpState,
    headers: HeaderMap,
    req: McpRequest,
) -> axum::response::Response {
    // ── Public methods (no auth required) ──
    let public_methods = ["initialize", "notifications/initialized", "tools/list"];
    let is_public = public_methods.contains(&req.method.as_str());

    // ── API Key Auth (required for non-public methods) ──
    let identity: Option<KeyIdentity> = if is_public {
        None
    } else {
        match extract_api_key(&headers) {
            Some(key) => {
                match &state.db {
                    Some(db) => {
                        // Validate API key against database
                        // In relay, this uses ApiKeyRepo::validate from relay/src/api_keys.rs
                        // In standalone mode, we do a direct DB query
                        match validate_api_key(db.pool(), &key).await {
                            Some(id) => Some(id),
                            None => {
                                log::warn!("MCP auth failed: invalid API key prefix={}", &key[..12.min(key.len())]);
                                return mcp_err(req.id, -32001, "Unauthorized: invalid API key").into_response();
                            }
                        }
                    }
                    None => {
                        log::warn!("MCP auth skipped: no DB configured");
                        // Without DB, create a wildcard identity for development
                        Some(KeyIdentity {
                            key_id: uuid::Uuid::nil(),
                            account_id: "standalone".into(),
                            org_id: "standalone".into(),
                            scopes: vec!["*".into()],
                        })
                    }
                }
            }
            None => {
                log::warn!("MCP request without API key — rejected");
                return mcp_err(req.id, -32001, "Unauthorized: API key required. Use Authorization: Bearer flk_... or x-api-key header").into_response();
            }
        }
    };

    match req.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2025-03-26",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "flowlink-relay", "version": "0.2.0" }
            });
            mcp_ok(req.id, result).into_response()
        }

        "notifications/initialized" => {
            axum::http::StatusCode::NO_CONTENT.into_response()
        }

        "tools/list" => {
            mcp_ok(req.id, json!({ "tools": mcp_tools() })).into_response()
        }

        "tools/call" => {
            if identity.is_none() {
                return mcp_err(req.id, -32001, "Unauthorized: API key required for tool calls").into_response();
            }
            handle_tools_call(req, identity).await
        }

        _ => mcp_err(req.id, -32601, format!("method not found: {}", req.method)).into_response(),
    }
}

async fn handle_tools_call(req: McpRequest, identity: Option<KeyIdentity>) -> axum::response::Response {
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
        "flowlink_agents" => {
            // In standalone mode, return empty agents list
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "No connected flowlink agents (standalone mode)." }]
            })).into_response()
        }
        "flowlink_exec" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_exec") { return mcp_err(req.id, -32002, "Forbidden: missing agents:write scope").into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "Command execution requires relay connection." }]
            })).into_response()
        }
        "flowlink_read" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_read") { return mcp_err(req.id, -32002, "Forbidden: missing agents:read scope").into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "File read requires relay connection." }]
            })).into_response()
        }
        "flowlink_write" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_write") { return mcp_err(req.id, -32002, "Forbidden: missing agents:write scope").into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "File write requires relay connection." }]
            })).into_response()
        }
        "flowlink_list" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_list") { return mcp_err(req.id, -32002, "Forbidden: missing agents:read scope").into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "Directory listing requires relay connection." }]
            })).into_response()
        }
        "flowlink_sysinfo" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_sysinfo") { return mcp_err(req.id, -32002, "Forbidden: missing system:read scope").into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "System info requires relay connection." }]
            })).into_response()
        }
        "flowlink_kill" | "flowlink_deregister" => {
            if let Some(ref id) = identity { if !id.can_call(name) { return mcp_err(req.id, -32002, "Forbidden: missing agents:admin scope").into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": format!("{} requires relay connection.", name) }]
            })).into_response()
        }
        "flowlink_health" => {
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": "Health check requires relay connection." }]
            })).into_response()
        }
        "flowlink_config_update" | "flowlink_approve" | "flowlink_policy" => {
            let scope = name.replace("flowlink_", "");
            if let Some(ref id) = identity { if !id.can_call(name) { return mcp_err(req.id, -32002, &format!("Forbidden: missing {} scope", scope)).into_response(); } }
            mcp_ok(req.id, json!({
                "content": [{ "type": "text", "text": format!("{} requires relay connection.", name) }]
            })).into_response()
        }
        _ => mcp_err(req.id, -32602, format!("unknown tool: {name}")).into_response(),
    }
}

// ═══════════════════════════════════════════════
// API Key extraction
// ═══════════════════════════════════════════════

fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // 1. x-api-key header
    if let Some(v) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let key = v.trim();
        if key.starts_with("flk_") {
            return Some(key.to_string());
        }
    }
    // 2. Authorization: Bearer flk_...
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let parts: Vec<&str> = v.splitn(2, ' ').collect();
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case("bearer") {
            let key = parts[1].trim();
            if key.starts_with("flk_") {
                return Some(key.to_string());
            }
        }
    }
    None
}

/// Validate API key against the database.
/// In relay, this is `ApiKeyRepo::validate` from relay's api_keys module.
/// Here we implement a direct DB query for standalone operation.
async fn validate_api_key(pool: &sqlx::PgPool, key: &str) -> Option<KeyIdentity> {
    use sha2::{Digest, Sha256};

    // Hash the key to compare with stored hash
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash = hex::encode(hasher.finalize());

    let row = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, Option<Vec<String>>)>(
        "SELECT id, org_id, account_id, scopes FROM api_keys WHERE key_hash = $1 AND revoked_at IS NULL"
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .ok()??;

    Some(KeyIdentity {
        key_id: row.0,
        org_id: row.1.to_string(),
        account_id: row.2,
        scopes: row.3.unwrap_or_default(),
    })
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

pub fn mcp_ok(id: Option<Value>, result: Value) -> Json<McpResponse> {
    Json(McpResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    })
}

pub fn mcp_err(id: Option<Value>, code: i32, message: impl Into<String>) -> Json<McpResponse> {
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
    fn test_key_identity_scopes() {
        let id = KeyIdentity {
            key_id: uuid::Uuid::new_v4(),
            account_id: "test".into(),
            org_id: "org".into(),
            scopes: vec!["*".into()],
        };
        assert!(id.can_call("flowlink_exec"));
        assert!(id.can_call("anything"));

        let id2 = KeyIdentity {
            key_id: uuid::Uuid::new_v4(),
            account_id: "test".into(),
            org_id: "org".into(),
            scopes: vec!["flowlink_exec".into()],
        };
        assert!(id2.can_call("flowlink_exec"));
        assert!(!id2.can_call("flowlink_read"));
    }

    #[tokio::test]
    async fn test_initialize() {
        let state = McpState { db: None, billing: None };
        let req = mcp_req("initialize", Some(json!(1)), None);
        let resp = process_mcp_http(&state, HeaderMap::new(), req).await;
        // Should return 200 with initialize response
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_tools_list() {
        let state = McpState { db: None, billing: None };
        let req = mcp_req("tools/list", Some(json!(2)), None);
        let resp = process_mcp_http(&state, HeaderMap::new(), req).await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }
}

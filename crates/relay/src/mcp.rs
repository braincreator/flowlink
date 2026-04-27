// MCP Server — JSON-RPC endpoint for AI model interaction with agents.
// POST /mcp — accepts MCP protocol requests (initialize, tools/list, tools/call).

use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api_keys::{ApiKeyRepo, KeyIdentity};
use crate::approval::ApprovalDecision;
use crate::server::AppState;

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
            "description": "Emergency kill switch — immediately disconnect an agent from the relay. Use when an agent is compromised or running unauthorized commands. Agent record is preserved.",
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
            "description": "Permanently remove an agent — disconnects WS, removes from pool and database. The agent will no longer be able to reconnect. Requires explicit confirmation.",
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
            "description": "Health check for a connected agent — returns connection latency, heartbeat age, and system load.",
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
            "description": "Update an agent's configuration remotely (read_only, label, work_dir). Agent will apply changes immediately.",
            "inputSchema": {
                "type": "object",
                "required": ["agent"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "read_only": { "type": "boolean", "description": "Set agent to read-only mode" },
                    "label": { "type": "string", "description": "Update agent label" },
                    "work_dir": { "type": "string", "description": "Set agent working directory" },
                    "approval_mode": { "type": "string", "enum": ["auto", "soft_ask", "hard_ask"], "description": "Set approval mode: auto=execute all, soft_ask=approve medium/high risk, hard_ask=approve all commands" },
                    "approval_timeout_sec": { "type": "integer", "description": "Seconds before auto-rejecting pending approvals. 0=no timeout, default=300" }
                }
            }
        }),
        json!({
            "name": "flowlink_approve",
            "description": "Approve or reject a pending command from an agent. Use 'list' to see pending approvals, 'approve' to allow, 'reject' to deny, 'approve_always' to add a permanent allow rule.",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "action"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "action": { "type": "string", "enum": ["list", "approve", "reject", "approve_always"], "description": "Action to perform" },
                    "request_id": { "type": "string", "description": "Approval request ID (required for approve/reject/approve_always)" },
                    "reason": { "type": "string", "description": "Reason for the decision (optional, for audit)" },
                    "approver": { "type": "string", "description": "Who is making this decision (user ID or name, for audit trail)" }
                }
            }
        }),
        json!({
            "name": "flowlink_policy",
            "description": "Dynamically manage agent policy rules at runtime. Add/remove allow/deny patterns without restart. Patterns use glob syntax (* = wildcard).",
            "inputSchema": {
                "type": "object",
                "required": ["agent", "action"],
                "properties": {
                    "agent": { "type": "string", "description": "Agent ID or label" },
                    "action": { "type": "string", "enum": ["list", "add_allow", "add_deny", "remove"], "description": "Policy action" },
                    "pattern": { "type": "string", "description": "Glob pattern for the rule (required for add/remove). Examples: 'docker *', 'npm *', 'sudo apt *'" }
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
    headers: HeaderMap,
    Json(req): Json<McpRequest>,
) -> axum::response::Response {
    process_mcp_http(&state, headers, req).await
}

/// Internal MCP processor (shared between HTTP and Streamable HTTP transports)
pub async fn process_mcp_http(
    state: &AppState,
    headers: HeaderMap,
    req: McpRequest,
) -> axum::response::Response {
    // ── Public methods (no auth required) ──
    let public_methods = ["initialize", "notifications/initialized", "tools/list"];
    let is_public = public_methods.contains(&req.method.as_str());

    // ── API Key Auth (required for non-public methods) ──
    let identity = if is_public {
        // No auth needed, but check if key provided for optional rate limiting
        None
    } else {
        match extract_api_key(&headers) {
            Some(key) => {
                match &state.db {
                    Some(db) => {
                        match ApiKeyRepo::validate(db.pool(), &key).await {
                            Ok(Some(id)) => {
                                // Per-key rate limiting
                                if !state.key_rate_limiter.check(&id.key_id.to_string()).await {
                                    log::warn!("MCP rate limited: key_id={}", id.key_id);
                                    return mcp_err(req.id, -32029, "Rate limit exceeded: max 100 requests per minute per key").into_response();
                                }
                                Some(id)
                            }
                            Ok(None) => {
                                log::warn!("MCP auth failed: invalid API key prefix={}", &key[..12.min(key.len())]);
                                return mcp_err(req.id, -32001, "Unauthorized: invalid API key").into_response();
                            }
                            Err(e) => {
                                log::error!("MCP auth error: {e}");
                                // DB error — allow through for now (graceful degradation)
                                None
                            }
                        }
                    }
                    None => {
                        log::warn!("MCP auth skipped: no DB configured");
                        None
                    }
                }
            }
            None => {
                log::warn!("MCP request without API key — rejected");
                return mcp_err(req.id, -32001, "Unauthorized: API key required. Use Authorization: Bearer flk_... or x-api-key header").into_response();
            }
        }
    };

    // Resolve plan for feature/limit enforcement
    let plan = identity.as_ref().and_then(|id| {
        state.billing.as_ref().and_then(|billing| {
            let acc = billing.get_or_create_account(&id.account_id);
            billing.plans().get(&acc.plan_id)
        })
    });

    match req.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2024-11-05",
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
            // Enforce auth for tool calls
            if identity.is_none() {
                return mcp_err(req.id, -32001, "Unauthorized: API key required for tool calls. Use Authorization: Bearer flk_... or x-api-key header").into_response();
            }
            handle_tools_call(state.clone(), req, identity, plan).await
        }

        _ => mcp_err(req.id, -32601, format!("method not found: {}", req.method)).into_response(),
    }
}

async fn handle_tools_call(state: AppState, req: McpRequest, identity: Option<KeyIdentity>, plan: Option<flowlink_billing::plans::Plan>) -> axum::response::Response {
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
        "flowlink_exec" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_exec") { return mcp_err(req.id, -32002, "Forbidden: missing agents:write scope").into_response(); } }
            mcp_exec(&state, req.id, &args, identity.as_ref()).await
        }
        "flowlink_read" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_read") { return mcp_err(req.id, -32002, "Forbidden: missing agents:read scope").into_response(); } }
            mcp_read(&state, req.id, &args).await
        }
        "flowlink_write" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_write") { return mcp_err(req.id, -32002, "Forbidden: missing agents:write scope").into_response(); } }
            mcp_write(&state, req.id, &args).await
        }
        "flowlink_list" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_list") { return mcp_err(req.id, -32002, "Forbidden: missing agents:read scope").into_response(); } }
            mcp_list(&state, req.id, &args).await
        }
        "flowlink_sysinfo" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_sysinfo") { return mcp_err(req.id, -32002, "Forbidden: missing system:read scope").into_response(); } }
            mcp_sysinfo(&state, req.id, &args).await
        }
        "flowlink_kill" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_kill") { return mcp_err(req.id, -32002, "Forbidden: missing agents:admin scope").into_response(); } }
            mcp_kill(&state, req.id, &args).await
        }
        "flowlink_deregister" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_deregister") { return mcp_err(req.id, -32002, "Forbidden: missing agents:admin scope").into_response(); } }
            mcp_deregister(&state, req.id, &args).await
        }
        "flowlink_health" => mcp_health(&state, req.id, &args),
        "flowlink_config_update" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_config_update") { return mcp_err(req.id, -32002, "Forbidden: missing system:write scope").into_response(); } }
            mcp_config_update(&state, req.id, &args).await
        }
        "flowlink_approve" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_approve") { return mcp_err(req.id, -32002, "Forbidden: missing approvals:write scope").into_response(); } }
            if let Err(e) = crate::plan_gate::require_feature(&plan, "approval", Some(crate::plan_gate::feature_min_tier("approval"))) {
                return mcp_err(req.id, -32003, &format!("Plan gate: {} (upgrade: {})", e.message, e.upgrade_url.as_deref().unwrap_or("/pricing"))).into_response();
            }
            mcp_approve(&state, req.id, &args, identity.as_ref()).await
        }
        "flowlink_policy" => {
            if let Some(ref id) = identity { if !id.can_call("flowlink_policy") { return mcp_err(req.id, -32002, "Forbidden: missing policy:read scope").into_response(); } }
            if let Err(e) = crate::plan_gate::require_feature(&plan, "policy_engine", Some(crate::plan_gate::feature_min_tier("policy_engine"))) {
                return mcp_err(req.id, -32003, &format!("Plan gate: {} (upgrade: {})", e.message, e.upgrade_url.as_deref().unwrap_or("/pricing"))).into_response();
            }
            mcp_policy(&state, req.id, &args).await
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

async fn mcp_exec(state: &AppState, id: Option<Value>, args: &Value, identity: Option<&KeyIdentity>) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };
    let command = match get_arg(args, "command") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "command: required").into_response(),
    };
    if command.len() > 8192 {
        return mcp_err(id, -32602, "command too long (max 8192 chars)").into_response();
    }

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    // Audit log with identity
    if let Some(id) = identity {
        log::info!("MCP exec: agent={resolved} cmd={command:.80} caller={} org={}", id.account_id, id.org_id);
    }

    let timeout: i32 = args.get("timeout").and_then(|v| v.as_i64()).unwrap_or(120) as i32;
    let timeout = timeout.clamp(1, 600); // max 10 minutes
    let workdir = args.get("workdir").and_then(|v| v.as_str()).map(String::from);

    // Secret injection: check mappings and inject env vars before sending to agent
    let (injected_env, secrets_injected, _requires_approval, resolved_command) =
        if let (Some(db), Some(ident)) = (&state.db, identity) {
            let agent_labels = state.pool.get(&resolved).map(|a| a.labels).unwrap_or_default();
            crate::secret_mappings_api::inject_for_exec(
                db.pool(),
                Some(&ident.org_id),
                &command,
                &agent_labels,
                &resolved,
            ).await
        } else {
            (std::collections::HashMap::new(), vec![], vec![], command.clone())
        };

    if !secrets_injected.is_empty() {
        log::info!("🔐 Secret injection: {} secrets for agent {}", secrets_injected.len(), resolved);
    }

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::ExecRequest)
        .with_agent_id(&resolved)
        .with_payload(flowlink_core::ExecRequestPayload {
            command: resolved_command,
            shell: None,
            env: if injected_env.is_empty() { None } else { Some(injected_env) },
            dir: workdir,
            timeout_sec: timeout,
            request_id: flowlink_core::request_id(),
        });

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => mcp_ok(id, json!({
            "content": [{ "type": "text", "text": format!("Command sent to agent {}", resolved) }]
        })).into_response(),
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
    }
}

async fn mcp_read(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let (agent_id, path) = match (get_arg(args, "agent"), get_arg(args, "path")) {
        (Some(a), Some(p)) => (a, p),
        _ => return mcp_err(id, -32602, "agent and path: required").into_response(),
    };
    // Validate path: reject traversal
    if path.contains("..") || path.starts_with('/') {
        return mcp_err(id, -32602, "path must be relative and not contain '..'").into_response();
    }
    if path.len() > 4096 {
        return mcp_err(id, -32602, "path too long (max 4096 chars)").into_response();
    }

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
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
    }
}

async fn mcp_write(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let (agent_id, path) = match (get_arg(args, "agent"), get_arg(args, "path")) {
        (Some(a), Some(p)) => (a, p),
        _ => return mcp_err(id, -32602, "agent, path, content: required").into_response(),
    };
    // Validate path: reject traversal
    if path.contains("..") || path.starts_with('/') {
        return mcp_err(id, -32602, "path must be relative and not contain '..'").into_response();
    }
    if path.len() > 4096 {
        return mcp_err(id, -32602, "path too long (max 4096 chars)").into_response();
    }
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.len() > 1024 * 1024 {
        return mcp_err(id, -32602, "content too large (max 1MB)").into_response();
    }

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
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
    }
}

async fn mcp_list(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let (agent_id, path) = match (get_arg(args, "agent"), get_arg(args, "path")) {
        (Some(a), Some(p)) => (a, p),
        _ => return mcp_err(id, -32602, "agent and path: required").into_response(),
    };
    if path.contains("..") {
        return mcp_err(id, -32602, "path must not contain '..'").into_response();
    }

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
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
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
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
    }
}

// ═══════════════════════════════════════════════
// Kill Switch / Health / Config Update
// ═══════════════════════════════════════════════

async fn mcp_kill(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };
    let reason = get_arg(args, "reason").unwrap_or_else(|| "No reason provided".into());

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    // Send disconnect message and remove from pool
    let msg = flowlink_core::Message::new(flowlink_core::MessageType::Disconnect)
        .with_agent_id(&resolved)
        .with_priority(flowlink_core::Priority::System)
        .with_payload(serde_json::json!({
            "reason": reason,
            "killed": true,
            "timestamp": chrono::Utc::now().timestamp()
        }));

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => {
            state.pool.set_offline(&resolved);
            state.handler.remove_sender(&resolved);
            mcp_ok(id, json!({
                "content": [{ "type": "text", "text": format!("🛑 Agent {} disconnected. Reason: {}. Agent record preserved (use deregister to remove).", resolved, reason) }]
            })).into_response()
        }
        Err(e) => {
            state.pool.unregister(&resolved);
            state.handler.remove_sender(&resolved);
            mcp_ok(id, json!({
                "content": [{ "type": "text", "text": format!("🛑 Agent {} force-disconnected (send failed: {}). Reason: {}", resolved, e, reason) }]
            })).into_response()
        }
    }
}

fn mcp_health(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let agent = match state.pool.get(&resolved) {
        Some(a) => a,
        None => return mcp_err(id, -32603, "agent info not in pool").into_response(),
    };

    let now = chrono::Utc::now().timestamp();
    let heartbeat_age = now - agent.last_heartbeat;
    let connected_seconds = now - agent.connected_at;
    let online = heartbeat_age < 120;

    let status = if online { "🟢 HEALTHY" } else { "🔴 UNHEALTHY" };
    let mut text = format!(
        "{} Agent: {} ({})\n  Heartbeat: {}s ago\n  Connected: {}s ago\n  OS/Arch: {}/{}",
        status, agent.hostname, agent.agent_id,
        heartbeat_age, connected_seconds, agent.os, agent.arch
    );

    if !agent.labels.is_empty() {
        text.push_str(&format!("\n  Labels: {}", agent.labels.join(", ")));
    }
    if !agent.capabilities.is_empty() {
        text.push_str(&format!("\n  Capabilities: {}", agent.capabilities.join(", ")));
    }

    let mut health_data = json!({
        "agent_id": agent.agent_id,
        "hostname": agent.hostname,
        "online": online,
        "heartbeat_age_sec": heartbeat_age,
        "connected_sec": connected_seconds,
        "os": agent.os,
        "arch": agent.arch,
    });

    if !online {
        health_data["warning"] = json!("Heartbeat stale — agent may be disconnected");
    }

    mcp_ok(id, json!({
        "content": [
            { "type": "text", "text": text },
            { "type": "text", "text": serde_json::to_string_pretty(&health_data).unwrap_or_default() }
        ]
    })).into_response()
}

async fn mcp_config_update(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    // Build ConfigUpdate payload with only provided fields
    let mut payload = serde_json::json!({
        "request_id": flowlink_core::request_id()
    });

    if let Some(ro) = args.get("read_only").and_then(|v| v.as_bool()) {
        payload["read_only"] = json!(ro);
    }
    if let Some(label) = args.get("label").and_then(|v| v.as_str()) {
        payload["label"] = json!(label);
    }
    if let Some(workdir) = args.get("work_dir").and_then(|v| v.as_str()) {
        payload["work_dir"] = json!(workdir);
    }
    if let Some(mode) = args.get("approval_mode").and_then(|v| v.as_str()) {
        payload["approval_mode"] = json!(mode);
    }
    if let Some(timeout) = args.get("approval_timeout_sec").and_then(|v| v.as_u64()) {
        payload["approval_timeout_sec"] = json!(timeout);
    }

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::ConfigUpdate)
        .with_agent_id(&resolved)
        .with_priority(flowlink_core::Priority::System)
        .with_payload(payload);

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => {
            let changes: Vec<String> = [
                args.get("read_only").map(|v| format!("read_only={}", v.as_bool().unwrap_or(false))),
                args.get("label").map(|v| format!("label={}", v.as_str().unwrap_or(""))),
                args.get("work_dir").map(|v| format!("work_dir={}", v.as_str().unwrap_or(""))),
                args.get("approval_mode").map(|v| format!("approval_mode={}", v.as_str().unwrap_or(""))),
                args.get("approval_timeout_sec").map(|v| format!("approval_timeout_sec={}", v)),
            ].into_iter().flatten().collect();

            if changes.is_empty() {
                mcp_err(id, -32602, "No fields to update. Provide at least one of: read_only, label, work_dir, approval_mode").into_response()
            } else {
                mcp_ok(id, json!({
                    "content": [{ "type": "text", "text": format!("✅ Config update sent to agent {}. Changes: {}", resolved, changes.join(", ")) }]
                })).into_response()
            }
        }
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
    }
}

// ═══════════════════════════════════════════════
// Approve + Policy Management
// ═══════════════════════════════════════════════

async fn mcp_deregister(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };
    let reason = get_arg(args, "reason").unwrap_or_else(|| "No reason provided".into());

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    // Disconnect WS if online
    let _ = state.handler.send_to_agent(&resolved,
        flowlink_core::Message::new(flowlink_core::MessageType::Disconnect)
            .with_agent_id(&resolved)
            .with_priority(flowlink_core::Priority::System)
            .with_payload(serde_json::json!({"reason": reason, "timestamp": chrono::Utc::now().timestamp()}))
    ).await;

    state.handler.remove_sender(&resolved);
    state.pool.deregister(&resolved);

    // Remove from DB
    if let Some(db) = state.db.as_ref() {
        let _ = sqlx::query("DELETE FROM agents WHERE agent_id = $1")
            .bind(&resolved)
            .execute(db.write_pool()).await;
    }

    log::info!("Agent deregistered via MCP: {resolved} (reason: {reason})");
    mcp_ok(id, json!({
        "content": [{ "type": "text", "text": format!("🗑️ Agent {} permanently deregistered. Reason: {}. DB record deleted.", resolved, reason) }]
    })).into_response()
}

async fn mcp_approve(state: &AppState, id: Option<Value>, args: &Value, identity: Option<&KeyIdentity>) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };
    let action = match get_arg(args, "action") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "action: required (list/approve/reject/approve_always)").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    match action.as_str() {
        "list" => {
            let pending = state.approvals.list_pending();
            let agent_pending: Vec<_> = pending.iter().filter(|r| r.agent_id == resolved).collect();

            if agent_pending.is_empty() {
                mcp_ok(id, json!({
                    "content": [{ "type": "text", "text": "📋 No pending approvals for this agent." }]
                })).into_response()
            } else {
                let lines: Vec<String> = agent_pending.iter().map(|r| {
                    format!("  📋 {} | {} | {} | {}s ago",
                        r.id, r.command, r.risk_level,
                        chrono::Utc::now().timestamp() - r.created_at)
                }).collect();

                mcp_ok(id, json!({
                    "content": [{
                        "type": "text",
                        "text": format!("📋 Pending approvals ({}):\n{}", agent_pending.len(), lines.join("\n"))
                    }]
                })).into_response()
            }
        }
        "approve" | "reject" => {
            let request_id = match get_arg(args, "request_id") {
                Some(v) => v,
                None => return mcp_err(id, -32602, "request_id: required for approve/reject").into_response(),
            };

            let decision = if action == "approve" {
                ApprovalDecision::Approved
            } else {
                ApprovalDecision::Rejected
            };

            let reason = get_arg(args, "reason").unwrap_or_default();
            let approver = if let Some(id) = identity {
                format!("mcp:{} ({})", id.account_id, id.key_id)
            } else {
                get_arg(args, "approver").unwrap_or_else(|| "mcp:unknown".into())
            };

            if state.approvals.resolve(&request_id, decision.clone()) {
                // Log the decision
                log::info!(
                    "Approval {}: request={} agent={} approver={:?} reason={:?}",
                    action, request_id, resolved, approver, reason
                );

                // Update DB audit log with identity
                if let Some(ref db) = state.db {
                    let _ = sqlx::query(
                        "UPDATE approval_log SET status = $1, approver = $2, reason = $3, resolved_at = NOW(),
                         org_id = $5, approver_account_id = $6, api_key_id = $7 WHERE id = $4"
                    )
                    .bind(&action)
                    .bind(&approver)
                    .bind(&reason)
                    .bind(&request_id)
                    .bind(identity.map(|i| i.org_id))
                    .bind(identity.map(|i| i.account_id.as_str()))
                    .bind(identity.map(|i| i.key_id))
                    .execute(db.write_pool())
                    .await;
                }

                let emoji = if action == "approve" { "✅" } else { "❌" };
                let text = if reason.is_empty() {
                    format!("{} Approval {} for request {} by {}", emoji, action, request_id, approver)
                } else {
                    format!("{} Approval {} for request {} by {}. Reason: {}", emoji, action, request_id, approver, reason)
                };

                // Also send decision to agent via WS
                let msg_type = if action == "approve" {
                    flowlink_core::MessageType::ExecApprove
                } else {
                    flowlink_core::MessageType::ExecReject
                };
                let _ = state.handler.send_to_agent(&resolved,
                    flowlink_core::Message::new(msg_type)
                        .with_agent_id(&resolved)
                        .with_payload(json!({
                            "request_id": request_id,
                            "decision": action,
                            "reason": reason,
                            "approved": action == "approve",
                        }))
                ).await;

                mcp_ok(id, json!({
                    "content": [{ "type": "text", "text": text }]
                })).into_response()
            } else {
                mcp_err(id, -32603, format!("request {} not found or already resolved", request_id)).into_response()
            }
        }
        "approve_always" => {
            let request_id = match get_arg(args, "request_id") {
                Some(v) => v,
                None => return mcp_err(id, -32602, "request_id: required for approve_always").into_response(),
            };

            // First, get the command from the pending approval
            let pending = state.approvals.list_pending();
            let req = pending.iter().find(|r| r.id == request_id);
            let command = match req {
                Some(r) => r.command.clone(),
                None => return mcp_err(id, -32603, format!("request {} not found", request_id)).into_response(),
            };

            // 1. Approve the pending request
            let _ = state.approvals.resolve(&request_id, ApprovalDecision::Approved);

            // 2. Notify agent
            let _ = state.handler.send_to_agent(&resolved,
                flowlink_core::Message::new(flowlink_core::MessageType::ExecApprove)
                    .with_agent_id(&resolved)
                    .with_payload(json!({
                        "request_id": request_id,
                        "decision": "approve_always",
                    }))
            ).await;

            // 3. Add permanent allow rule via PolicyUpdate
            let _ = state.handler.send_to_agent(&resolved,
                flowlink_core::Message::new(flowlink_core::MessageType::PolicyUpdate)
                    .with_agent_id(&resolved)
                    .with_priority(flowlink_core::Priority::System)
                    .with_payload(json!({
                        "action": "add_allow",
                        "pattern": command,
                    }))
            ).await;

            mcp_ok(id, json!({
                "content": [{ "type": "text", "text": format!("✅ Approved + added permanent allow rule: '{}'", command) }]
            })).into_response()
        }
        other => mcp_err(id, -32602, format!("unknown action '{}'. Use: list, approve, reject, approve_always", other)).into_response(),
    }
}

async fn mcp_policy(state: &AppState, id: Option<Value>, args: &Value) -> axum::response::Response {
    let agent_id = match get_arg(args, "agent") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "agent: required").into_response(),
    };
    let action = match get_arg(args, "action") {
        Some(v) => v,
        None => return mcp_err(id, -32602, "action: required (list/add_allow/add_deny/remove)").into_response(),
    };

    let resolved = match resolve_agent(&state.pool, &agent_id) {
        Some(id) => id,
        None => return mcp_err(id, -32602, format!("agent not found: {agent_id}")).into_response(),
    };

    let payload = match action.as_str() {
        "list" => json!({ "action": "list" }),
        "add_allow" | "add_deny" | "remove" => {
            let pattern = match get_arg(args, "pattern") {
                Some(v) => v,
                None => return mcp_err(id, -32602, "pattern: required for add_allow/add_deny/remove").into_response(),
            };
            json!({ "action": action, "pattern": pattern })
        }
        other => return mcp_err(id, -32602, format!("unknown action '{}'", other)).into_response(),
    };

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::PolicyUpdate)
        .with_agent_id(&resolved)
        .with_priority(flowlink_core::Priority::System)
        .with_payload(payload);

    match state.handler.send_to_agent(&resolved, msg).await {
        Ok(()) => {
            let action_text = match action.as_str() {
                "list" => "Policy rules requested".to_string(),
                "add_allow" => format!("Allow rule added: {}", args.get("pattern").and_then(|v| v.as_str()).unwrap_or("")),
                "add_deny" => format!("Deny rule added: {}", args.get("pattern").and_then(|v| v.as_str()).unwrap_or("")),
                "remove" => format!("Rule removed: {}", args.get("pattern").and_then(|v| v.as_str()).unwrap_or("")),
                _ => "Unknown".to_string(),
            };
            mcp_ok(id, json!({
                "content": [{ "type": "text", "text": format!("🔒 {}", action_text) }]
            })).into_response()
        }
        Err(_e) => mcp_err(id, -32603, "agent error").into_response(),
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

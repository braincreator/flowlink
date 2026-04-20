use axum::{
    extract::{Path, Query, State, ws::{WebSocket, WebSocketUpgrade, Message as AxumMsg}},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use log::info;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
// StreamExt comes from futures_util (re-exported via axum)

use crate::approval::{ApprovalDecision, ApprovalQueue};
use crate::config_reload::ConfigReloader;
use crate::devices::DeviceManager;
use crate::eventbus::EventBus;
use crate::handler::RelayHandler;
use crate::llm::{LlmProxy, LlmRequest};
use crate::metrics::Metrics;
use axum::middleware;
use crate::middleware::{rate_limit_layer, request_id_middleware, logging_middleware, cors_layer};
use crate::ratelimit::RateLimiter;
use crate::pool::{AgentInfo, AgentPool};
use crate::registry::Registry;
use flowlink_core::ShieldAlertPayload;
use crate::audit::{AuditStore, AuditFilter, SiemFormat};

// ═══════════════════════════════════════════════
// Shared State
// ═══════════════════════════════════════════════

#[derive(Clone)]
pub struct AppState {
    pub start_time: std::time::Instant,
    pub pool: Arc<AgentPool>,
    pub approvals: Arc<ApprovalQueue>,
    pub eventbus: Arc<EventBus>,
    pub handler: Arc<RelayHandler>,
    pub registry: Arc<Registry>,
    pub device_manager: Arc<DeviceManager>,
    pub llm_proxy: Option<Arc<LlmProxy>>,
    pub shield_alerts: Arc<ShieldAlertManager>,
    pub audit_store: Arc<AuditStore>,
    pub metrics: Arc<Metrics>,
    pub billing: Option<Arc<flowlink_billing::BillingEngine>>,
    pub db: Option<Arc<flowlink_db::DbPool>>,
    pub config_reloader: Option<Arc<ConfigReloader>>,
    pub e2ee: Arc<crate::e2ee::E2eeSessionManager>,
    pub usage_tracker: Arc<crate::billing_middleware::UsageTracker>,
    pub rate_limiter: Arc<RateLimiter>,
    pub control_plane: crate::control_plane::ControlPlaneState,
    pub tochka: Option<Arc<flowlink_billing::tochka::TochkaClient>>,
    pub auth: Arc<crate::auth::AuthManager>,
    pub auth_engine: Option<Arc<crate::auth::AuthEngine>>,
    pub email_service: Option<Arc<crate::email::EmailService>>,
    pub email_queue: std::sync::OnceLock<Arc<crate::email_queue::EmailQueue>>,
    pub tg_bot: std::sync::OnceLock<teloxide::Bot>,
    pub notification_router: std::sync::OnceLock<std::sync::Arc<crate::notifications::NotificationRouter>>,
    pub notification_store: Option<Arc<crate::preferences_api::NotificationStore>>,
    pub rbac: Arc<crate::rbac_manager::RbacManager>,
    pub auth_rate_limiter: Arc<crate::auth_rate_limiter::AuthRateLimiter>,
    pub tiered_rate_limiter: Arc<crate::rate_limiter::TieredRateLimiter>,
}

// ═══════════════════════════════════════════════
// Response types
// ═══════════════════════════════════════════════

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
    db: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limits: Option<crate::rate_limiter::RateLimitStats>,
}

#[derive(Serialize)]
struct SimpleResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Deserialize)]
struct ExecBody {
    command: String,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    env: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    dir: Option<String>,
    #[serde(default = "default_timeout")]
    timeout_sec: i32,
}

fn default_timeout() -> i32 { 60 }

#[derive(Deserialize)]
struct WsParams {
    token: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct SseParams {
    token: Option<String>,
    #[serde(default = "default_channels")]
    channels: String,
}

fn default_channels() -> String { "all".into() }

// ═══════════════════════════════════════════════
// Shield Alert Manager
// ═══════════════════════════════════════════════

use dashmap::DashMap;

/// Tracks active shield alerts and manages resolution.
/// Shared state accessible from WS handler and HTTP routes.
pub struct ShieldAlertManager {
    alerts: DashMap<String, ShieldAlertEntry>,
    stats: std::sync::atomic::AtomicU64, // total received
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ShieldAlertEntry {
    pub alert_id: String,
    pub pid: u32,
    pub uid: u32,
    pub username: String,
    pub command: String,
    pub rule_name: String,
    pub action: String,
    pub snapshot: Option<String>,
    pub timestamp: i64,
    pub agent_id: Option<String>,
    pub resolved: bool,
    pub approved: Option<bool>,
}

impl Default for ShieldAlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShieldAlertManager {
    pub fn new() -> Self {
        Self {
            alerts: DashMap::new(),
            stats: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Record an incoming shield alert from an agent or shield guard.
    pub fn add(&self, entry: ShieldAlertEntry) {
        self.stats.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.alerts.insert(entry.alert_id.clone(), entry);
    }

    /// Resolve an alert by PID (approve or reject).
    pub fn resolve_by_pid(&self, pid: u32, approved: bool) -> bool {
        for mut entry in self.alerts.iter_mut() {
            if entry.value().pid == pid && !entry.value().resolved {
                entry.value_mut().resolved = true;
                entry.value_mut().approved = Some(approved);
                return true;
            }
        }
        false
    }

    /// List all active (unresolved) alerts.
    pub fn list_active(&self) -> Vec<ShieldAlertEntry> {
        self.alerts.iter()
            .filter(|e| !e.value().resolved)
            .map(|e| e.value().clone())
            .collect()
    }

    /// List all alerts (including resolved).
    pub fn list_all(&self) -> Vec<ShieldAlertEntry> {
        self.alerts.iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get stats.
    pub fn stats(&self) -> (u64, u64, u64) {
        let total = self.stats.load(std::sync::atomic::Ordering::Relaxed);
        let pending: u64 = self.alerts.iter().filter(|e| !e.value().resolved).count() as u64;
        let resolved = total - pending;
        (total, pending, resolved)
    }
}

// ═══════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let db_status = match &state.db {
        Some(pool) => match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool.pool()).await {
            Ok(_) => "ok".to_string(),
            Err(e) => {
                log::error!("Health DB check failed: {}", e);
                "error".to_string()
            }
        },
        None => "disabled".to_string(),
    };
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        db: db_status,
        timestamp: chrono::Utc::now().to_rfc3339(),
        rate_limits: Some(state.tiered_rate_limiter.stats()),
    })
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn list_agents(State(state): State<AppState>) -> Json<Vec<AgentInfo>> {
    Json(state.pool.list())
}

/// GET /api/account/info — Returns current account info from DB
async fn account_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Extract account_id from JWT
    let token = match headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "missing authorization"}))).into_response(),
    };

    let account_id = match &state.auth_engine {
        Some(engine) => match engine.validate_access_token(token) {
            Ok(claims) => claims.account_id,
            Err(_) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response(),
        },
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "auth not configured"}))).into_response(),
    };

    // Query account from DB
    if let Some(ref db) = state.db {
        match flowlink_db::accounts::AccountRepo::get(db.pool(), &account_id).await {
            Ok(Some(acc)) => {
                let servers_count = state.pool.count();
                return (StatusCode::OK, Json(serde_json::json!({
                    "active": acc.active,
                    "plan_name": acc.plan_id,
                    "plan_id": acc.plan_id,
                    "servers_count": servers_count,
                    "user": {
                        "id": acc.account_id,
                        "name": "",
                        "email": acc.email.unwrap_or_default()
                    },
                    "created_at": acc.created_at.timestamp(),
                    "last_login": acc.last_login.map(|t| t.timestamp()).unwrap_or(0),
                    "deletion_requested_at": acc.deletion_requested_at.map(|t| t.to_rfc3339()),
                    "deleted_at": acc.deleted_at.map(|t| t.to_rfc3339()),
                }))).into_response();
            }
            Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "account not found"}))).into_response(),
            Err(e) => {
                log::warn!("account_info DB error: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
            }
        }
    }

    // Fallback: return basic info from JWT claims
    (StatusCode::OK, Json(serde_json::json!({
        "active": true,
        "plan_name": "free",
        "plan_id": "free",
        "servers_count": state.pool.count(),
        "user": { "id": account_id, "name": "", "email": "" },
        "created_at": 0,
        "last_login": 0
    }))).into_response()
}

/// GET /api/account/settings
async fn account_get_settings(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({
        "name": "",
        "email": "",
        "notifications": {
            "push_enabled": true,
            "email_frequency": "immediate"
        }
    }))).into_response()
}

/// PUT /api/account/settings — persist settings to DB
async fn account_update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Extract account_id from JWT
    let token = match headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "missing authorization"}))).into_response(),
    };

    let account_id = match &state.auth_engine {
        Some(engine) => match engine.validate_access_token(token) {
            Ok(claims) => claims.account_id,
            Err(_) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response(),
        },
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "auth not configured"}))).into_response(),
    };

    // Update email if provided
    if let Some(email) = body.get("email").and_then(|v| v.as_str()) {
        if let Some(ref db) = state.db {
            if let Err(e) = sqlx::query("UPDATE accounts SET email = $1, updated_at = NOW() WHERE account_id = $2")
                .bind(email).bind(&account_id)
                .execute(db.pool()).await
            {
                log::warn!("Failed to update account settings: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
            }
        }
    }

    // Update notification preferences via NotificationStore
    if let (Some(notifications), Some(ref store)) = (body.get("notifications"), &state.notification_store) {
        if let Some(push_enabled) = notifications.get("push_enabled").and_then(|v| v.as_bool()) {
            let _kind = if push_enabled { "push_enabled" } else { "push_disabled" };
            store.add(&account_id, "settings", "Notification Settings", &format!("Push notifications {}", if push_enabled { "enabled" } else { "disabled" })).await;
        }
    }

    (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
}

async fn list_approvals(
    State(state): State<AppState>,
) -> Json<Vec<crate::approval::ApprovalRequest>> {
    Json(state.approvals.list_pending())
}

async fn approve_approval(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<SimpleResponse> {
    let ok = state.approvals.resolve(&id, ApprovalDecision::Approved);
    Json(SimpleResponse {
        ok,
        message: if ok { Some("Approved".into()) } else { Some("Not found".into()) },
    })
}

async fn reject_approval(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<SimpleResponse> {
    let ok = state.approvals.resolve(&id, ApprovalDecision::Rejected);
    Json(SimpleResponse {
        ok,
        message: if ok { Some("Rejected".into()) } else { Some("Not found".into()) },
    })
}

async fn list_clients(State(state): State<AppState>) -> Json<Vec<crate::registry::RegisteredClient>> {
    Json(state.registry.list_clients())
}

async fn exec_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<ExecBody>,
) -> impl IntoResponse {
    let msg = flowlink_core::Message::new(flowlink_core::MessageType::ExecRequest)
        .with_agent_id(&agent_id)
        .with_payload(flowlink_core::ExecRequestPayload {
            command: body.command,
            shell: body.shell,
            env: body.env,
            dir: body.dir,
            timeout_sec: body.timeout_sec,
            request_id: flowlink_core::request_id(),
        });

    match state.handler.send_to_agent(&agent_id, msg).await {
        Ok(()) => Json(SimpleResponse { ok: true, message: Some("Sent".into()) }).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(SimpleResponse { ok: false, message: Some(e.to_string()) }),
        ).into_response(),
    }
}

/// SSE endpoint — streams events from EventBus.
/// Auth via `?token=<agent_id>` query param.
async fn sse_events(
    State(state): State<AppState>,
    Query(params): Query<SseParams>,
) -> impl IntoResponse {
    // Validate token
    let _agent_id = match &params.token {
        Some(token) => match state.handler.auth.validate_token(token) {
            Some(c) if c.active => c.client_id.clone(),
            _ => return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(SimpleResponse { ok: false, message: Some("Invalid or inactive token".into()) }),
            ).into_response(),
        },
        None => return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(SimpleResponse { ok: false, message: Some("Missing token param".into()) }),
        ).into_response(),
    };

    let channels: Vec<String> = if params.channels == "all" {
        vec![
            "heartbeat".into(), "exec_done".into(), "exec_output".into(),
            "approval_request".into(), "shield_alert".into(),
            "agent_disconnect".into(), "sysinfo".into(),
        ]
    } else {
        params.channels.split(',').map(|s| s.trim().to_string()).collect()
    };

    let eventbus = state.eventbus.clone();
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(256);

    // Spawn forwarders for each channel
    for ch in channels {
        let eventbus = eventbus.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut sub = eventbus.subscribe(&ch);
            while let Ok(data) = sub.recv().await {
                if tx.send(Event::default().data(data)).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(tx); // drop our copy so the channel closes when all forwarders die

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(Ok::<Event, Infallible>);

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

// ═══════════════════════════════════════════════
// WebSocket upgrade (axum native)
// ═══════════════════════════════════════════════

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = match params.token {
        Some(t) => t,
        None => return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(SimpleResponse { ok: false, message: Some("Missing token param".into()) }),
        ).into_response(),
    };

    let agent_id = match params.agent_id {
        Some(id) => id,
        None => return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(SimpleResponse { ok: false, message: Some("Missing agent_id param".into()) }),
        ).into_response(),
    };

    // Validate token
    let client = match state.handler.auth.validate_token(&token) {
        Some(c) if c.active => c,
        _ => return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(SimpleResponse { ok: false, message: Some("Invalid or inactive token".into()) }),
        ).into_response(),
    };

    // ── Billing: check host limit before allowing connection ──
    let client_id = client.client_id.clone();
    if let Some(billing_engine) = &state.billing {
        let account_billing = billing_engine.get_or_create_account(&client_id);
        let check = billing_engine.check_and_track(
            &account_billing,
            flowlink_billing::usage::UsageOperation::AgentConnect,
        );
        if !check.allowed {
            let reason = check.reason.unwrap_or_else(|| "Plan limit exceeded".into());
            info!("WS upgrade denied for agent {} (client {}): {}", agent_id, client_id, reason);
            return (
                axum::http::StatusCode::FORBIDDEN,
                Json(SimpleResponse { ok: false, message: Some(reason) }),
            ).into_response();
        }
    }

    info!("WS upgrade for agent {} (client {})", agent_id, client_id);

    ws.on_upgrade(move |socket| handle_ws(socket, agent_id, client_id, state))
}

async fn handle_ws(socket: WebSocket, agent_id: String, client_id: String, state: AppState) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AxumMsg>(256);

    // Register sender
    // Each WS connection gets a unique ID to prevent stale sender removal
    static CONNECTION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let conn_id = CONNECTION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    state.handler.register_sender(agent_id.clone(), (tx.clone(), conn_id));

    let read_tx = tx.clone();

    // Register in pool
    state.pool.register(crate::pool::AgentInfo {
        agent_id: agent_id.clone(),
        hostname: String::new(),
        os: String::new(),
        arch: String::new(),
        connected_at: chrono::Utc::now().timestamp(),
        last_heartbeat: chrono::Utc::now().timestamp(),
        labels: vec![],
        capabilities: vec![],
        online: true,
    });

    // Notify via Telegram
    if let Some(tg_bot) = state.tg_bot.get() {
        let state_arc = Arc::new(state.clone());
        let agent_id_clone = agent_id.clone();
        let bot = tg_bot.clone();
        tokio::spawn(async move {
            crate::tgbot::notifications::agent_connected(&bot, &state_arc, &agent_id_clone, "unknown").await;
        });
    }

    // Send Connected ack with relay's E2EE public key
    let connected = flowlink_core::Message::new(flowlink_core::MessageType::Connected)
        .with_agent_id(&agent_id)
        .with_payload(flowlink_core::ConnectedPayload {
            agent_id: agent_id.clone(),
            relay_id: "relay-0".into(),
            heartbeat_interval_sec: 30,
            server_time: chrono::Utc::now().timestamp(),
            relay_public_key: Some(state.e2ee.relay_public_key().to_string()),
            relay_key_id: Some(state.e2ee.relay_key_id().to_string()),
        });
    if let Ok(json) = serde_json::to_string(&connected) {
        let _ = ws_sink.send(AxumMsg::Text(json.into())).await;
    }

    // Push DB policies to agent on connect
    if let Some(ref db) = state.db {
        if let Ok(rules) = crate::policy_db::load_agent_rules(db.write_pool(), &agent_id).await {
            if !rules.is_empty() {
                let denies: Vec<String> = rules.iter().filter(|r| r.action == "deny").map(|r| r.pattern.clone()).collect();
                let allows: Vec<String> = rules.iter().filter(|r| r.action == "allow").map(|r| r.pattern.clone()).collect();
                let push_msg = flowlink_core::Message::new(flowlink_core::MessageType::PolicyUpdate)
                    .with_agent_id(&agent_id)
                    .with_priority(flowlink_core::Priority::System)
                    .with_payload(serde_json::json!({"action": "replace_all", "denies": denies, "allows": allows, "source": "db"}));
                if let Ok(json) = serde_json::to_string(&push_msg) {
                    let _ = ws_sink.send(AxumMsg::Text(json.into())).await;
                    log::info!("Pushed {} rules to agent {} from DB", rules.len(), agent_id);
                }
            }
        }
    }

    let aid = agent_id.clone();
    let pool = state.pool.clone();
    let eventbus = state.eventbus.clone();

    // Read loop
    let read_task = async {
        while let Some(msg) = futures_util::StreamExt::next(&mut ws_stream).await {
            match msg {
                Ok(AxumMsg::Text(text)) => {
                    let text_str: String = text.to_string();
                    // Try E2EE decryption first; fall back to plaintext
                    let effective_text = if let Some(decrypted) = state.e2ee.decrypt_from_agent(&text_str) {
                        String::from_utf8(decrypted).unwrap_or(text_str.clone())
                    } else {
                        text_str.clone()
                    };
                    if let Ok(msg) = serde_json::from_str::<flowlink_core::Message>(&effective_text) {
                        // Track every incoming message as an API request
                        state.usage_tracker.record_request(&aid).await;

                        match msg.msg_type {
                            flowlink_core::MessageType::Connect => {
                                // Register agent's public key for E2EE if provided
                                if let Some(payload) = &msg.payload {
                                    if let Ok(connect) = serde_json::from_value::<flowlink_core::ConnectPayload>(payload.clone()) {
                                        if let Some(pk) = &connect.public_key {
                                            state.e2ee.register_agent_key(&aid, pk).await;
                                            log::info!("Agent {}: E2EE public key registered", aid);
                                        }
                                    }
                                }
                            }
                            flowlink_core::MessageType::Heartbeat => {
                                pool.update_heartbeat(&aid);
                                eventbus.publish("heartbeat", &text_str);
                            }
                            flowlink_core::MessageType::ExecDone => {
                                state.usage_tracker.record_command(&aid).await;
                                eventbus.publish("exec_done", &text_str);
                            }
                            flowlink_core::MessageType::ExecOutput => {
                                eventbus.publish("exec_output", &text_str);
                            }
                            flowlink_core::MessageType::NeedsApproval => {
                                eventbus.publish("approval_request", &text_str);
                            }
                            flowlink_core::MessageType::ShieldAlert => {
                                eventbus.publish("shield_alert", &text_str);
                                // Store in shield alert manager
                                if let Some(payload) = msg.payload.clone() {
                                    if let Ok(alert) = serde_json::from_value::<ShieldAlertPayload>(payload) {
                                        let entry = ShieldAlertEntry {
                                            alert_id: alert.alert_id,
                                            pid: alert.pid,
                                            uid: alert.uid,
                                            username: alert.username,
                                            command: alert.command,
                                            rule_name: alert.rule_name,
                                            action: alert.action,
                                            snapshot: alert.snapshot,
                                            timestamp: alert.timestamp,
                                            agent_id: Some(aid.clone()),
                                            resolved: false,
                                            approved: None,
                                        };
                                        state.shield_alerts.add(entry);
                                    }
                                }
                            }
                            flowlink_core::MessageType::SysInfo => {
                                eventbus.publish("sysinfo", &text_str);
                            }
                            flowlink_core::MessageType::ConfigAck => {
                                eventbus.publish("config_ack", &text_str);
                                log::info!("Agent {aid}: config acknowledged");
                            }
                            flowlink_core::MessageType::LlmResponse => {
                                // Extract token usage from LLM response payload
                                if let Some(ref payload) = msg.payload {
                                    let (tokens_in, tokens_out) =
                                        crate::billing_middleware::extract_tokens_from_payload(payload);
                                    if tokens_in > 0 || tokens_out > 0 {
                                        state.usage_tracker.record_tokens(&aid, tokens_in, tokens_out).await;
                                    }
                                }
                            }
                            flowlink_core::MessageType::Disconnect => break,
                            other => {
                                log::info!("Agent {aid}: {:?}", other);
                            }
                        }
                    }
                }
                Ok(AxumMsg::Ping(data)) => {
                    let _ = read_tx.send(AxumMsg::Pong(data)).await;
                }
                Ok(AxumMsg::Close(_)) => break,
                Err(e) => {
                    log::error!("Agent {aid} WS error: {e}");
                    break;
                }
                _ => {}
            }
        }
    };

    // Write loop — forward queued messages
    let write_task = async {
        while let Some(msg) = rx.recv().await {
            if ws_sink.send(msg).await.is_err() {
                break;
            }
        }
    };

    // Ping loop — send WS Ping every 20s to detect dead connections
    let ping_tx = tx.clone();
    let ping_task = async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(20));
        interval.tick().await; // skip first
        loop {
            interval.tick().await;
            if ping_tx.send(AxumMsg::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = read_task => {},
        _ = write_task => {},
        _ = ping_task => {},
    }

    // Only remove sender if it hasn't been replaced by a new connection
    // (agent may have reconnected before this WS handler finished cleanup)
    state.handler.remove_sender_if_stale(&aid, conn_id);
    pool.set_offline(&aid);
    state.e2ee.remove_agent_key(&aid).await;
    // Notify via Telegram
    if let Some(tg_bot) = state.tg_bot.get() {
        let state_arc = Arc::new(state.clone());
        let agent_id_clone = aid.clone();
        let bot = tg_bot.clone();
        tokio::spawn(async move {
            crate::tgbot::notifications::agent_disconnected(&bot, &state_arc, &agent_id_clone, "unknown").await;
        });
    }
    // Release billing usage counter for this host
    if let Some(billing_engine) = &state.billing {
        billing_engine.usage().release_agent(&client_id);
    }
    eventbus.publish("agent_disconnect", &serde_json::to_string(&serde_json::json!({"agent_id": aid})).unwrap_or_default());
}

// ═══════════════════════════════════════════════
// Shield Alert Routes
// ═══════════════════════════════════════════════

async fn shield_list_alerts(State(state): State<AppState>) -> Json<Vec<ShieldAlertEntry>> {
    Json(state.shield_alerts.list_active())
}

async fn shield_approve(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Json<SimpleResponse> {
    let ok = state.shield_alerts.resolve_by_pid(pid, true);
    Json(SimpleResponse {
        ok,
        message: if ok { Some(format!("PID {} approved", pid)) } else { Some("No active alert for this PID".into()) },
    })
}

async fn shield_reject(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Json<SimpleResponse> {
    let ok = state.shield_alerts.resolve_by_pid(pid, false);
    Json(SimpleResponse {
        ok,
        message: if ok { Some(format!("PID {} rejected", pid)) } else { Some("No active alert for this PID".into()) },
    })
}

async fn shield_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let (total, pending, resolved) = state.shield_alerts.stats();
    Json(serde_json::json!({
        "total_received": total,
        "pending": pending,
        "resolved": resolved,
    }))
}

/// Receive a shield alert from an external source (e.g., standalone Shield Guard).
#[derive(Deserialize, serde::Serialize)]
struct IngestAlertBody {
    alert_id: Option<String>,
    pid: u32,
    uid: Option<u32>,
    username: Option<String>,
    command: String,
    rule_name: String,
    action: String,
    snapshot: Option<String>,
    timestamp: Option<i64>,
    agent_id: Option<String>,
}

async fn shield_ingest_alert(
    State(state): State<AppState>,
    Json(body): Json<IngestAlertBody>,
) -> Json<SimpleResponse> {
    let entry = ShieldAlertEntry {
        alert_id: body.alert_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        pid: body.pid,
        uid: body.uid.unwrap_or(0),
        username: body.username.clone().unwrap_or_default(),
        command: body.command.clone(),
        rule_name: body.rule_name.clone(),
        action: body.action.clone(),
        snapshot: body.snapshot.clone(),
        timestamp: body.timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp()),
        agent_id: body.agent_id.clone(),
        resolved: false,
        approved: None,
    };
    state.shield_alerts.add(entry);
    state.eventbus.publish("shield_alert", &serde_json::to_string(&body).unwrap_or_default());
    Json(SimpleResponse { ok: true, message: Some("Alert recorded".into()) })
}

/// Receive resolution notification from Shield Guard.
#[derive(Deserialize, serde::Serialize)]
struct ResolveAlertBody {
    pid: u32,
    approved: bool,
}

async fn shield_resolve(
    State(state): State<AppState>,
    Json(body): Json<ResolveAlertBody>,
) -> Json<SimpleResponse> {
    let ok = state.shield_alerts.resolve_by_pid(body.pid, body.approved);
    Json(SimpleResponse {
        ok,
        message: if ok { Some("Resolved".into()) } else { Some("No active alert for this PID".into()) },
    })
}

// ═══════════════════════════════════════════════
// LLM Proxy Handlers
// ═══════════════════════════════════════════════

async fn llm_chat(
    State(state): State<AppState>,
    Json(body): Json<LlmRequest>,
) -> impl IntoResponse {
    let proxy = match &state.llm_proxy {
        Some(p) => p,
        None => return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "LLM proxy not configured" })),
        ).into_response(),
    };

    match proxy.complete(body).await {
        Ok(resp) => {
            // Track token usage from the HTTP LLM endpoint
            let tokens_in = resp.usage.prompt_tokens as u64;
            let tokens_out = resp.usage.completion_tokens as u64;
            // Use a generic agent id for HTTP API calls
            state.usage_tracker.record_tokens("_api_http", tokens_in, tokens_out).await;
            Json(resp).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn llm_backends(State(state): State<AppState>) -> Json<serde_json::Value> {
    match &state.llm_proxy {
        Some(proxy) => {
            let models = proxy.list_models().await;
            Json(serde_json::json!({ "backends": models }))
        }
        None => Json(serde_json::json!({ "backends": [] })),
    }
}

async fn llm_health(State(state): State<AppState>) -> Json<serde_json::Value> {
    match &state.llm_proxy {
        Some(proxy) => {
            let health = proxy.check_health().await;
            let map: std::collections::HashMap<String, String> = health.into_iter().collect();
            Json(serde_json::json!({ "health": map }))
        }
        None => Json(serde_json::json!({ "health": {} })),
    }
}

// ═══════════════════════════════════════════════
// Audit Channel Handlers
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
struct AuditQueryParams {
    agent_id: Option<String>,
    event_type: Option<String>,
    since: Option<u64>,
    until: Option<u64>,
    min_risk_score: Option<u8>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct AuditExportParams {
    format: Option<String>,
    agent_id: Option<String>,
    event_type: Option<String>,
    limit: Option<usize>,
}

async fn audit_query(State(state): State<AppState>, Query(params): Query<AuditQueryParams>) -> Json<Vec<flowlink_core::channels::AuditEvent>> {
    let filter = AuditFilter {
        agent_id: params.agent_id,
        event_type: params.event_type,
        since: params.since,
        until: params.until,
        min_risk_score: params.min_risk_score,
        limit: params.limit,
    };
    Json(state.audit_store.query(&filter))
}

async fn audit_stats_handler(State(state): State<AppState>) -> Json<crate::audit::AuditStats> {
    Json(state.audit_store.stats())
}

async fn audit_export(State(state): State<AppState>, Query(params): Query<AuditExportParams>) -> impl IntoResponse {
    let filter = AuditFilter {
        agent_id: params.agent_id,
        event_type: params.event_type,
        since: None,
        until: None,
        min_risk_score: None,
        limit: params.limit,
    };
    let format = match params.format.as_deref() {
        Some("cef") => SiemFormat::Cef,
        Some("leef") => SiemFormat::Leef,
        _ => SiemFormat::Json,
    };
    let body = state.audit_store.export_siem(&format, &filter);
    let content_type = match format {
        SiemFormat::Cef => "text/plain",
        SiemFormat::Leef => "text/plain",
        SiemFormat::Json => "application/json",
    };
    ([(axum::http::header::CONTENT_TYPE, content_type)], body).into_response()
}

async fn audit_ingest(State(state): State<AppState>, Json(event): Json<flowlink_core::channels::AuditEvent>) -> Json<SimpleResponse> {
    match state.audit_store.record(event) {
        Ok(()) => Json(SimpleResponse { ok: true, message: None }),
        Err(e) => Json(SimpleResponse { ok: false, message: Some(e.to_string()) }),
    }
}

async fn canary_alert_handler(State(state): State<AppState>, Json(alert): Json<serde_json::Value>) -> Json<SimpleResponse> {
    let event = flowlink_core::channels::AuditEvent::new(
        alert.get("agent_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        flowlink_core::channels::AuditEventType::CanaryTriggered {
            path: alert.get("token_path").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            accessor: alert.get("accessor").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            access_type: alert.get("access_type").and_then(|v| v.as_str()).unwrap_or("read").to_string(),
        },
    );
    match state.audit_store.record(event) {
        Ok(()) => Json(SimpleResponse { ok: true, message: None }),
        Err(e) => Json(SimpleResponse { ok: false, message: Some(e.to_string()) }),
    }
}

// ═══════════════════════════════════════════════
// Config Reload Endpoints
// ═══════════════════════════════════════════════

/// Reload relay config from disk and broadcast to all agents.
async fn config_reload(State(state): State<AppState>) -> impl IntoResponse {
    let reloader = match &state.config_reloader {
        Some(r) => r,
        None => return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(SimpleResponse { ok: false, message: Some("Config hot-reload not enabled (no config path)".into()) }),
        ).into_response(),
    };

    match reloader.reload().await {
        Ok(result) => Json(result).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleResponse { ok: false, message: Some(format!("Reload failed: {e}")) }),
        ).into_response(),
    }
}

/// Push current config to a specific agent.
async fn config_push_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let reloader = match &state.config_reloader {
        Some(r) => r,
        None => return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(SimpleResponse { ok: false, message: Some("Config hot-reload not enabled".into()) }),
        ).into_response(),
    };

    let result = reloader.push_to_agent(&agent_id).await.unwrap_or_else(|e| crate::config_reload::PushResult {
        ok: false,
        message: e.to_string(),
        pushed_to: vec![],
        failed: vec![(agent_id.clone(), e.to_string())],
        timestamp: chrono::Utc::now().timestamp(),
    });
    Json(result).into_response()
}

/// Get current relay config (read-only).
async fn config_get(State(state): State<AppState>) -> impl IntoResponse {
    let reloader = match &state.config_reloader {
        Some(r) => r,
        None => return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(SimpleResponse { ok: false, message: Some("Config hot-reload not enabled".into()) }),
        ).into_response(),
    };

    let config = reloader.get_config().await;
    // Mask sensitive fields
    let masked = serde_json::json!({
        "client_name": config.client_name,
        "http_addr": config.http_addr.to_string(),
        "wss_addr": config.wss_addr.to_string(),
        "llm_enabled": config.llm.enabled,
        "llm_backends": config.llm.backends.len(),
        "billing_enabled": config.billing.enabled,
        "registry_data_path": config.registry.data_path,
        "registry_max_hosts": config.registry.max_hosts,
        "database_primary": config.database.primary.as_ref().map(|_| "***"),
        "database_replicas": config.database.replicas.len(),
        "reload_count": reloader.reload_count(),
    });
    Json(masked).into_response()
}

// ═══════════════════════════════════════════════
// Router Builder
// ═══════════════════════════════════════════════

// ── Admin account management ──

async fn admin_list_accounts(State(state): State<AppState>, Query(params): Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    let mut query = String::from(
        "SELECT account_id, plan_id, active, email, tg_id, totp_enabled, last_login, created_at FROM accounts WHERE 1=1"
    );
    if let Some(plan) = params.get("plan") { query.push_str(&format!(" AND plan_id = '{}'", plan)); }
    if let Some(active) = params.get("active") { query.push_str(&format!(" AND active = {}", active)); }
    if let Some(search) = params.get("search") { query.push_str(&format!(" AND (email ILIKE '%{}%' OR account_id ILIKE '%{}%')", search, search)); }
    if let Some(from) = params.get("from") { query.push_str(&format!(" AND created_at >= '{}'", from)); }
    if let Some(to) = params.get("to") { query.push_str(&format!(" AND created_at <= '{}'", to)); }
    query.push_str(" ORDER BY created_at DESC LIMIT 100");
    match sqlx::query_as::<_, flowlink_db::accounts::AccountRow>(&query)
        .fetch_all(db.pool())
        .await
    {
        Ok(accounts) => {
            let data: Vec<_> = accounts.into_iter().map(|a| serde_json::json!({
                "account_id": a.account_id, "plan_id": a.plan_id, "active": a.active,
                "email": a.email, "tg_id": a.tg_id, "totp_enabled": a.totp_enabled,
                "last_login": a.last_login.map(|d| d.to_rfc3339()), "created_at": a.created_at.to_rfc3339(),
            })).collect();
            (StatusCode::OK, Json(serde_json::json!({ "accounts": data }))).into_response()
        }
        Err(e) => {
            log::error!("Admin list accounts failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

async fn admin_change_plan(State(state): State<AppState>, Path(account_id): Path<String>, Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let plan_id = match body.get("plan_id").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "plan_id required"}))).into_response(),
    };
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No DB"}))).into_response(),
    };
    match flowlink_db::accounts::AccountRepo::update_plan(db.pool(), &account_id, &plan_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "plan_id": plan_id}))).into_response(),
        Err(e) => {
            log::error!("Admin change plan: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

async fn admin_toggle_active(State(state): State<AppState>, Path(account_id): Path<String>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No DB"}))).into_response(),
    };
    match flowlink_db::accounts::AccountRepo::get(db.pool(), &account_id).await {
        Ok(Some(acc)) => {
            match flowlink_db::accounts::AccountRepo::set_active(db.pool(), &account_id, !acc.active).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "active": !acc.active}))).into_response(),
                Err(e) => {
                    log::error!("Admin toggle: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))).into_response(),
        Err(e) => {
            log::error!("Admin get: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

// ── Admin Analytics ──

async fn admin_dashboard_stats(State(state): State<AppState>, Query(params): Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No DB"}))).into_response(),
    };
    let pool = db.pool();

    // Parse date range filters
    let date_from = params.get("from").cloned();
    let date_to = params.get("to").cloned();

    // Date filter clause
    let date_clause = match (&date_from, &date_to) {
        (Some(f), Some(t)) => format!("WHERE created_at >= '{}' AND created_at <= '{}'", f, t),
        (Some(f), None) => format!("WHERE created_at >= '{}'", f),
        (None, Some(t)) => format!("WHERE created_at <= '{}'", t),
        _ => String::new(),
    };

    // Total users
    let total_users: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM accounts {}", date_clause))
        .fetch_one(pool).await.unwrap_or(0);

    // Active users
    let active_users: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM accounts WHERE active = true {}", if date_clause.is_empty() { String::new() } else { "AND ".to_string() + &date_clause.replace("WHERE ", "") }))
        .fetch_one(pool).await.unwrap_or(0);

    // Users with 2FA
    let users_2fa: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE totp_enabled = true")
        .fetch_one(pool).await.unwrap_or(0);

    // Total paid orders
    let total_revenue_kop: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_kopecks), 0) FROM orders WHERE status = 'paid'"
    ).fetch_one(pool).await.unwrap_or(0);

    // MRR = sum of active subscriptions monthly amounts
    let mrr_kop: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_kopecks), 0) FROM subscriptions WHERE status = 'active' AND period = 'monthly'"
    ).fetch_one(pool).await.unwrap_or(0);

    // ARR = MRR * 12
    let arr_kop: i64 = mrr_kop * 12;

    // Active subscriptions
    let active_subs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subscriptions WHERE status = 'active'"
    ).fetch_one(pool).await.unwrap_or(0);

    // Churned (cancelled) subscriptions this month
    let churned_subs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subscriptions WHERE status = 'cancelled' AND cancelled_at >= date_trunc('month', NOW())"
    ).fetch_one(pool).await.unwrap_or(0);

    // Plan distribution
    let plan_dist: Vec<(String, i64)> = sqlx::query_as(
        "SELECT plan_id, COUNT(*) as cnt FROM accounts GROUP BY plan_id ORDER BY cnt DESC"
    ).fetch_all(pool).await.unwrap_or_default();

    // New users per day (last 30 days)
    let new_users_chart: Vec<(String, i64)> = sqlx::query_as(
        "SELECT DATE(created_at) as d, COUNT(*) as cnt FROM accounts WHERE created_at >= NOW() - INTERVAL '30 days' GROUP BY d ORDER BY d"
    ).fetch_all(pool).await.unwrap_or_default();

    // Revenue per month (last 12 months)
    let revenue_chart: Vec<(String, i64)> = sqlx::query_as(
        "SELECT to_char(created_at, 'YYYY-MM') as m, SUM(amount_kopecks) as total FROM orders WHERE status = 'paid' AND created_at >= NOW() - INTERVAL '12 months' GROUP BY m ORDER BY m"
    ).fetch_all(pool).await.unwrap_or_default();

    (StatusCode::OK, Json(serde_json::json!({
        "total_users": total_users,
        "active_users": active_users,
        "users_2fa": users_2fa,
        "total_revenue_rub": total_revenue_kop as f64 / 100.0,
        "mrr_rub": mrr_kop as f64 / 100.0,
        "arr_rub": arr_kop as f64 / 100.0,
        "active_subscriptions": active_subs,
        "churned_this_month": churned_subs,
        "plan_distribution": plan_dist.iter().map(|(p, c)| serde_json::json!({"plan": p, "count": c})).collect::<Vec<_>>(),
        "new_users_chart": new_users_chart.iter().map(|(d, c)| serde_json::json!({"date": d, "count": c})).collect::<Vec<_>>(),
        "revenue_chart": revenue_chart.iter().map(|(m, v)| serde_json::json!({"month": m, "revenue_rub": *v as f64 / 100.0})).collect::<Vec<_>>(),
    }))).into_response()
}

async fn admin_list_plans(State(state): State<AppState>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    match flowlink_db::plans::DbPlan::list_all(db.pool()).await {
        Ok(plans) => (StatusCode::OK, Json(serde_json::json!({ "plans": plans }))).into_response(),
        Err(e) => {
            log::error!("Admin list plans failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct CreatePlanBody {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    tier: i32,
    price_kopecks: i64,
    annual_price_kopecks: Option<i64>,
    period: String,
    currency: String,
    limits: Option<serde_json::Value>,
    features: Option<Vec<String>>,
    #[serde(default = "default_true")]
    is_active: bool,
    sort_order: i32,
    #[serde(default)]
    trial_days: i32,
}

fn default_true() -> bool { true }

async fn admin_create_plan(State(state): State<AppState>, Json(body): Json<CreatePlanBody>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    let limits: flowlink_db::plans::PlanLimits = body.limits
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let features = body.features.unwrap_or_default();
    let plan = flowlink_db::plans::DbPlan {
        id: body.id,
        name: body.name,
        description: body.description,
        tier: body.tier,
        price_kopecks: body.price_kopecks,
        annual_price_kopecks: body.annual_price_kopecks,
        period: body.period,
        currency: body.currency,
        limits,
        features,
        is_active: body.is_active,
        sort_order: body.sort_order,
        trial_days: body.trial_days,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    match flowlink_db::plans::DbPlan::upsert(db.pool(), &plan).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "id": plan.id}))).into_response(),
        Err(e) => {
            log::error!("Admin create plan: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

async fn admin_update_plan(State(state): State<AppState>, Path(id): Path<String>, Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    // Fetch existing plan, merge fields, then upsert
    let existing = match flowlink_db::plans::DbPlan::get_by_id(db.pool(), &id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Plan not found"}))).into_response(),
        Err(e) => {
            log::error!("Admin update plan: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response();
        }
    };
    let mut plan = existing;
    if let Some(name) = body.get("name").and_then(|v| v.as_str()) { plan.name = name.to_string(); }
    if let Some(desc) = body.get("description").and_then(|v| v.as_str()) { plan.description = desc.to_string(); }
    if let Some(tier) = body.get("tier").and_then(|v| v.as_i64()) { plan.tier = tier as i32; }
    if let Some(price) = body.get("price_kopecks").and_then(|v| v.as_i64()) { plan.price_kopecks = price; }
    if let Some(price) = body.get("annual_price_kopecks").and_then(|v| v.as_i64()) { plan.annual_price_kopecks = Some(price); }
    if let Some(period) = body.get("period").and_then(|v| v.as_str()) { plan.period = period.to_string(); }
    if let Some(currency) = body.get("currency").and_then(|v| v.as_str()) { plan.currency = currency.to_string(); }
    if let Some(limits) = body.get("limits") {
        if let Ok(l) = serde_json::from_value(limits.clone()) { plan.limits = l; }
    }
    if let Some(features) = body.get("features") {
        if let Ok(f) = serde_json::from_value(features.clone()) { plan.features = f; }
    }
    if let Some(active) = body.get("is_active").and_then(|v| v.as_bool()) { plan.is_active = active; }
    if let Some(sort) = body.get("sort_order").and_then(|v| v.as_i64()) { plan.sort_order = sort as i32; }
    if let Some(trial) = body.get("trial_days").and_then(|v| v.as_i64()) { plan.trial_days = trial as i32; }
    match flowlink_db::plans::DbPlan::upsert(db.pool(), &plan).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => {
            log::error!("Admin update plan: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

async fn admin_delete_plan(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    match flowlink_db::plans::DbPlan::set_active(db.pool(), &id, false).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Plan not found"}))).into_response(),
        Err(e) => {
            log::error!("Admin delete plan: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

async fn admin_list_subscriptions(State(state): State<AppState>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    match sqlx::query_as::<_, flowlink_db::subscriptions::SubscriptionRow>(
        r#"SELECT s.id, s.account_id, s.plan_id, s.status, s.period, s.amount_kopecks,
                  s.tochka_subscription_id, s.payment_method, s.started_at, s.expires_at,
                  s.trial_ends_at, s.next_billing_at, s.cancelled_at, s.created_at, s.updated_at
           FROM subscriptions s
           ORDER BY s.created_at DESC LIMIT 200"#
    )
    .fetch_all(db.pool())
    .await
    {
        Ok(rows) => {
            // Fetch emails for all accounts in one query
            let account_ids: Vec<&str> = rows.iter().map(|r| r.account_id.as_str()).collect();
            let mut emails: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            if !account_ids.is_empty() {
                if let Ok(accs) = sqlx::query_as::<_, (String, String)>(
                    "SELECT account_id, COALESCE(email, '') FROM accounts WHERE account_id = ANY($1)"
                ).bind(&account_ids).fetch_all(db.pool()).await {
                    for (aid, email) in accs { emails.insert(aid, email); }
                }
            }
            let data: Vec<_> = rows.into_iter().map(|r| serde_json::json!({
                "id": r.id, "account_id": r.account_id,
                "email": emails.get(&r.account_id).cloned().unwrap_or_default(),
                "plan_id": r.plan_id, "status": r.status, "period": r.period,
                "amount_kopecks": r.amount_kopecks,
                "tochka_subscription_id": r.tochka_subscription_id,
                "started_at": r.started_at.to_rfc3339(),
                "expires_at": r.expires_at.map(|d| d.to_rfc3339()),
                "created_at": r.created_at.to_rfc3339(),
            })).collect();
            (StatusCode::OK, Json(serde_json::json!({ "subscriptions": data }))).into_response()
        }
        Err(e) => {
            log::error!("Admin list subscriptions: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

async fn admin_list_orders(State(state): State<AppState>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "No database"}))).into_response(),
    };
    match sqlx::query_as::<_, flowlink_db::orders::OrderRow>(
        r#"SELECT id, account_id, invoice_id, amount_kopecks, description,
                  status, payment_method, tochka_payment_id, payment_url,
                  plan_id, paid_at, failed_at, created_at
           FROM orders
           ORDER BY created_at DESC LIMIT 200"#
    )
    .fetch_all(db.pool())
    .await
    {
        Ok(rows) => {
            let account_ids: Vec<&str> = rows.iter().map(|r| r.account_id.as_str()).collect();
            let mut emails: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            if !account_ids.is_empty() {
                if let Ok(accs) = sqlx::query_as::<_, (String, String)>(
                    "SELECT account_id, COALESCE(email, '') FROM accounts WHERE account_id = ANY($1)"
                ).bind(&account_ids).fetch_all(db.pool()).await {
                    for (aid, email) in accs { emails.insert(aid, email); }
                }
            }
            let data: Vec<_> = rows.into_iter().map(|r| serde_json::json!({
                "id": r.id, "account_id": r.account_id,
                "email": emails.get(&r.account_id).cloned().unwrap_or_default(),
                "plan_id": r.plan_id, "status": r.status, "amount_kopecks": r.amount_kopecks,
                "description": r.description, "payment_method": r.payment_method,
                "tochka_payment_id": r.tochka_payment_id, "paid_at": r.paid_at.map(|d| d.to_rfc3339()),
                "created_at": r.created_at.to_rfc3339(),
            })).collect();
            (StatusCode::OK, Json(serde_json::json!({ "orders": data }))).into_response()
        }
        Err(e) => {
            log::error!("Admin list orders: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "DB error"}))).into_response()
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    let rate_limiter = state.rate_limiter.clone();

    // ── Public routes (no JWT auth required) ──
    let public_routes = Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(health))
        // Playground — public, no auth
        .route("/api/playground/scan", axum::routing::post(crate::playground::playground_scan))
        // Auth endpoints
        .route("/api/auth/email/send-code", axum::routing::post(crate::email_auth::send_code))
            .route_layer(axum::middleware::from_fn_with_state(state.clone(), crate::auth_rate_middleware::email_auth_rate_limit))
        .route("/api/auth/email/verify", axum::routing::post(crate::email_auth::verify_code))
            .route_layer(axum::middleware::from_fn_with_state(state.clone(), crate::auth_rate_middleware::email_auth_rate_limit))
        // OAuth URL generation (frontend never sees client_secret)
        .route("/api/auth/oauth-url", axum::routing::get(crate::auth_oauth::oauth_url))
        // OAuth callbacks
        .route("/api/auth/vk/callback", axum::routing::get(crate::auth_oauth::vk_callback))
        .route("/api/auth/yandex/callback", axum::routing::get(crate::auth_oauth::yandex_callback))
        .route("/api/auth/github/callback", axum::routing::get(crate::auth_oauth::github_callback))
        .route("/api/auth/refresh", axum::routing::post(crate::auth_oauth::refresh_token))
        // 2FA (public: setup done authed, but complete is public for temp_token flow)
        .route("/api/auth/2fa/complete", axum::routing::post(crate::auth_2fa::complete_2fa))
        // Auth providers listing
        .route("/api/auth/providers", axum::routing::get(crate::auth_oauth::list_providers))
        // Public plans
        .route("/api/plans", axum::routing::get(crate::billing_api::public_plans))
        // Billing webhook (external, needs no auth)
        .route("/api/billing/webhook/tochka", axum::routing::post(crate::billing_api::tochka_webhook))
        // Billing expiry check (cron-callable, internal)
        .route("/api/billing/check-expiry", axum::routing::post(crate::billing_api::check_expiry))
        // GDPR deletion cleanup (cron-callable, internal)
        .route("/api/billing/cleanup-expired-deletions", axum::routing::post(crate::account_deletion_api::cleanup_expired_deletions))
        // Shield ingest (external agent reporting)
        .route("/api/shield/ingest", post(shield_ingest_alert))
        // Audit ingest (external)
        .route("/api/audit/event", post(audit_ingest))
        .route("/api/shield/canary", post(canary_alert_handler))
        // Control Plane
        .route("/api/v1/signup", axum::routing::post(crate::control_plane::signup))
        .route("/api/v1/heartbeat", axum::routing::post(crate::control_plane::heartbeat))
        // Metrics
        .route("/metrics", axum::routing::get(crate::metrics::metrics_handler))
        // WS (has its own token validation)
        .route("/ws", get(ws_upgrade))
        // MCP (has its own validation)
        .route("/mcp", axum::routing::post(crate::mcp::handle_mcp))
        // SSE (has its own token validation)
        .route("/api/events", get(sse_events));

    // ── Protected routes (require JWT auth) ──
    let protected_routes = Router::new()
        // Agents
        .route("/api/agents", get(list_agents))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}/approve", post(approve_approval))
        .route("/api/approvals/{id}/reject", post(reject_approval))
        .route("/api/exec/{agent_id}", post(exec_agent))
        // Devices
        .route("/api/devices/pair", axum::routing::post(crate::devices::pair_device))
        .route("/api/devices/confirm", axum::routing::post(crate::devices::confirm_pairing))
        .route("/api/devices", axum::routing::get(crate::devices::list_devices))
        .route("/api/devices/{id}", axum::routing::delete(crate::devices::remove_device))
        .route("/api/devices/{id}/trust", axum::routing::get(crate::devices::get_device_trust))
        // LLM
        .route("/api/llm", post(llm_chat))
        // LLM admin routes in admin_routes
        // Shield alerts (user-facing, stats moved to admin)
        .route("/api/shield/alerts", get(shield_list_alerts))
        .route("/api/shield/approve/{pid}", post(shield_approve))
        .route("/api/shield/reject/{pid}", post(shield_reject))
        .route("/api/shield/resolve", post(shield_resolve))
        // Audit (user-facing query only, stats/export in admin)
        .route("/api/audit", get(audit_query))
        // Config (view only, reload in admin)
        .route("/api/config", get(config_get))
        .route("/api/config/push/{agent_id}", post(config_push_agent))
        // Billing (except webhook and public plans)
        .route("/api/billing", axum::routing::get(crate::billing_api::get_billing_info))
        .route("/api/billing/usage", axum::routing::get(crate::billing_api::get_usage))
        .route("/api/billing/plans", axum::routing::get(crate::billing_api::list_plans))
        .route("/api/billing/change-plan", axum::routing::post(crate::billing_api::change_plan))
        .route("/api/billing/invoices", axum::routing::get(crate::billing_api::list_invoices))
        .route("/api/billing/invoices/{id}", axum::routing::get(crate::billing_api::get_invoice))
        .route("/api/billing/payments/methods", axum::routing::get(crate::billing_api::list_payment_methods))
        .route("/api/billing/subscribe", axum::routing::post(crate::billing_api::subscribe))
        .route("/api/billing/create-payment", axum::routing::post(crate::billing_api::subscribe))
        .route("/api/billing/subscription", axum::routing::get(crate::billing_api::get_subscription))
        .route("/api/billing/subscription/pause", axum::routing::post(crate::billing_api::pause_subscription))
        .route("/api/billing/subscription/resume", axum::routing::post(crate::billing_api::resume_subscription))
        .route("/api/billing/subscription", axum::routing::delete(crate::billing_api::cancel_tochka_subscription))
        .route("/api/billing/subscription/change-plan", axum::routing::post(crate::billing_api::change_subscription_plan))
        .route("/api/billing/subscriptions", axum::routing::get(crate::billing_api::list_subscriptions))
        .route("/api/billing/subscriptions/{id}/cancel", axum::routing::post(crate::billing_api::cancel_subscription))
        .route("/api/billing/orders", axum::routing::get(crate::billing_api::list_orders).post(crate::billing_api::create_order))
        // Control Plane (agents listing)
        .route("/api/v1/agents", axum::routing::get(crate::control_plane::list_agents))
        .route("/api/v1/agents/{id}", axum::routing::get(crate::control_plane::get_agent))
        .route("/api/v1/agents/{id}", axum::routing::delete(crate::control_plane::deregister_agent))
        // Policy management
        .route("/api/v1/policies", axum::routing::get(crate::policy_db::list_policies).post(crate::policy_db::create_policy))
        .route("/api/v1/policies/{id}", axum::routing::get(crate::policy_db::get_policy).delete(crate::policy_db::delete_policy))
        .route("/api/v1/policies/bind", axum::routing::post(crate::policy_db::bind_policy_to_agent))
        .route("/api/v1/policies/unbind", axum::routing::post(crate::policy_db::unbind_policy_from_agent))
        // Account
        .route("/api/account/info", axum::routing::get(account_info))
        .route("/api/account", axum::routing::delete(crate::account_deletion_api::request_deletion))
        .route("/api/account/cancel-deletion", axum::routing::post(crate::account_deletion_api::cancel_deletion))
        .route("/api/account/hard", axum::routing::delete(crate::account_deletion_api::hard_delete))
        .route("/api/account/settings", axum::routing::get(account_get_settings))
        .route("/api/account/settings", axum::routing::put(account_update_settings))
        .route("/api/account/notifications", axum::routing::get(crate::preferences_api::get_notifications))
        .route("/api/account/notifications/{id}/read", axum::routing::post(crate::preferences_api::mark_notification_read))
        // Notification channel management (user's own channels)
        .route("/api/notifications/channels", axum::routing::get(crate::notifications_api::list_channels))
        .route("/api/notifications/channels", axum::routing::post(crate::notifications_api::bind_channel))
        .route("/api/notifications/channels/{id}", axum::routing::patch(crate::notifications_api::update_channel))
        .route("/api/notifications/channels/{id}", axum::routing::delete(crate::notifications_api::unbind_channel))
        .route("/api/notifications/channels/{id}/verify", axum::routing::post(crate::notifications_api::verify_channel))
        .route("/api/notifications/channels/{id}/primary", axum::routing::post(crate::notifications_api::set_primary))
        .route("/api/notifications/test", axum::routing::post(crate::notifications_api::send_test))
        .route("/api/notifications/link-code", axum::routing::post(crate::notifications_api::generate_link_code))
        .route("/api/notifications/confirm-code", axum::routing::post(crate::notifications_api::confirm_link_code))
        // Auth (me, logout, account)
        .route("/api/auth/me", axum::routing::get(crate::auth_oauth::auth_me))
        .route("/api/auth/logout", axum::routing::post(crate::auth_oauth::logout))
        .route("/api/auth/account", axum::routing::get(crate::auth_oauth::account_info))
        .route("/api/auth/link-email", axum::routing::post(crate::auth_oauth::link_email))
        // 2FA management (protected)
        .route("/api/auth/2fa/setup", axum::routing::post(crate::auth_2fa::setup_2fa))
        .route("/api/auth/2fa/enable", axum::routing::post(crate::auth_2fa::enable_2fa))
        .route("/api/auth/2fa/disable", axum::routing::post(crate::auth_2fa::disable_2fa))
        .route("/api/auth/2fa/status", axum::routing::get(crate::auth_2fa::status_2fa))
        // Email change (protected)
        .route("/api/auth/email/change-start", axum::routing::post(crate::email_auth::change_email_start))
            .route_layer(axum::middleware::from_fn_with_state(state.clone(), crate::auth_rate_middleware::change_email_rate_limit))
        .route("/api/auth/email/change-confirm", axum::routing::post(crate::email_auth::change_email_confirm))
        // Session management
        .route("/api/auth/sessions", axum::routing::get(crate::auth_oauth::list_sessions))
        .route("/api/auth/sessions", axum::routing::delete(crate::auth_oauth::revoke_other_sessions))
        .route("/api/auth/sessions/{id}", axum::routing::delete(crate::auth_oauth::revoke_session))
        // Apply JWT auth + rate limiting to protected routes
        .layer(middleware::from_fn_with_state(std::sync::Arc::new(state.clone()), crate::middleware::jwt_auth))
        .layer(middleware::from_fn_with_state(std::sync::Arc::new(state.clone()), crate::billing_middleware::billing_enforcement_middleware));

    // ── Admin routes (require JWT auth + admin RBAC permission) ──
    let admin_routes = Router::new()
        .route("/api/admin/config", axum::routing::get(config_get))
        .route("/api/admin/config/reload", axum::routing::post(config_reload))
        .route("/api/admin/shield/alerts", axum::routing::get(shield_list_alerts))
        .route("/api/admin/audit/query", axum::routing::get(audit_query))
        .route("/api/admin/audit/stats", axum::routing::get(audit_stats_handler))
        .route("/api/admin/audit/export", axum::routing::get(audit_export))
        .route("/api/admin/clients", axum::routing::get(list_clients))
        .route("/api/admin/shield/stats", axum::routing::get(shield_stats))
        .route("/api/admin/llm/backends", axum::routing::get(llm_backends))
        .route("/api/admin/llm/health", axum::routing::get(llm_health))
        // Account management
        .route("/api/admin/accounts", axum::routing::get(admin_list_accounts))
        .route("/api/admin/accounts/{id}/plan", axum::routing::put(admin_change_plan))
        .route("/api/admin/accounts/{id}/toggle", axum::routing::post(admin_toggle_active))
        .route("/api/admin/dashboard-stats", axum::routing::get(admin_dashboard_stats))
        // Plans CRUD
        .route("/api/admin/plans", axum::routing::get(admin_list_plans).post(admin_create_plan))
        .route("/api/admin/plans/{id}", axum::routing::put(admin_update_plan).delete(admin_delete_plan))
        // Subscriptions & Orders listing
        .route("/api/admin/subscriptions", axum::routing::get(admin_list_subscriptions))
        .route("/api/admin/orders", axum::routing::get(admin_list_orders))
        .layer(middleware::from_fn_with_state(std::sync::Arc::new(state.clone()), crate::middleware::jwt_auth))
        .layer(axum::middleware::from_fn(|req: axum::extract::Request, next: axum::middleware::Next| {
            async move {
                // Dev mode: no claims in extensions means jwt_auth was skipped (no auth_engine)
                // RBAC: extract claims from extensions (set by jwt_auth middleware)
                let claims = req.extensions().get::<crate::auth::Claims>();
                match claims {
                    Some(c) if c.is_admin => next.run(req).await,
                    Some(_) => axum::http::StatusCode::FORBIDDEN.into_response(),
                    None => next.run(req).await, // dev mode — no auth_engine, allow
                }
            }
        }));

    // ── Organizations routes (require JWT auth) ──
    let org_routes = Router::new()
        .route("/api/orgs/onboard", axum::routing::post(crate::orgs_api::onboard))
        .route("/api/orgs", axum::routing::get(crate::orgs_api::list_my_orgs).post(crate::orgs_api::create_org))
        .route("/api/orgs/switch", axum::routing::post(crate::orgs_api::switch_org))
        .route("/api/orgs/{org_id}", axum::routing::get(crate::orgs_api::get_org).put(crate::orgs_api::update_org).delete(crate::orgs_api::delete_org))
        .route("/api/orgs/{org_id}/members", axum::routing::get(crate::orgs_api::list_members))
        .route("/api/orgs/{org_id}/invites", axum::routing::get(crate::orgs_api::list_invites).post(crate::orgs_api::invite_member))
        .route("/api/orgs/invites/accept", axum::routing::post(crate::orgs_api::accept_invite))
        .route("/api/orgs/{org_id}/members/{account_id}", axum::routing::delete(crate::orgs_api::remove_member).patch(crate::orgs_api::change_member_role))
        // Audit log + Webhooks
        .route("/api/orgs/{org_id}/audit", axum::routing::get(crate::webhooks_api::list_org_audit))
        .route("/api/orgs/{org_id}/webhooks", axum::routing::get(crate::webhooks_api::list_webhooks).post(crate::webhooks_api::create_webhook))
        .route("/api/orgs/{org_id}/webhooks/{id}", axum::routing::delete(crate::webhooks_api::delete_webhook))
        .route("/api/orgs/{org_id}/webhooks/{id}/test", axum::routing::post(crate::webhooks_api::test_webhook))
        .layer(middleware::from_fn_with_state(std::sync::Arc::new(state.clone()), crate::middleware::jwt_auth));

    let api_routes = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(admin_routes)
        .merge(org_routes);

    Router::new()
        .merge(api_routes)
        // Dashboard SPA (stateless routes)
        .route("/dashboard", get(crate::dashboard::serve_dashboard_root))
        .route("/dashboard/", get(crate::dashboard::serve_dashboard_root))
        .route("/dashboard/{*path}", get(crate::dashboard::serve_dashboard))
        .with_state(state)
        // API versioning: rewrite /api/v1/* → /api/*
        .layer(axum::middleware::from_fn(
            |req: axum::extract::Request, next: axum::middleware::Next| async move {
                let path = req.uri().path().to_string();
                if let Some(rest) = path.strip_prefix("/api/v1") {
                    let new_path = if rest.is_empty() { "/api".to_string() } else { format!("/api{}", rest) };
                    let new_uri = if let Some(q) = req.uri().query() {
                        format!("{}?{}", new_path, q).parse::<axum::http::Uri>().unwrap()
                    } else {
                        new_path.parse::<axum::http::Uri>().unwrap()
                    };
                    let (mut parts, body) = req.into_parts();
                    parts.uri = new_uri;
                    let new_req: axum::extract::Request = axum::http::Request::from_parts(parts, body);
                    next.run(new_req).await
                } else {
                    next.run(req).await
                }
            }
        ))
        // Middleware layers (innermost first)
        .layer(axum::middleware::from_fn(logging_middleware))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(axum::middleware::from_fn(rate_limit_layer(
            rate_limiter,
            vec!["/healthz".to_string(), "/ws".to_string(), "/api/playground/scan".to_string()],
        )))
        .layer(cors_layer(vec!["*".to_string()]))
        .fallback(handle_fallback)
}

/// 404 fallback handler — returns JSON error for unknown routes
async fn handle_fallback() -> impl IntoResponse {
    crate::middleware::json_error(
        StatusCode::NOT_FOUND,
        "not_found",
        "The requested resource does not exist",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthManager;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            start_time: std::time::Instant::now(),
            pool: Arc::new(AgentPool::new()),
            approvals: Arc::new(ApprovalQueue::new()),
            eventbus: Arc::new(EventBus::new()),
            handler: Arc::new(RelayHandler::new(
                Arc::new(AgentPool::new()),
                Arc::new(AuthManager::new(None)),
                Arc::new(EventBus::new()),
                Arc::new(ApprovalQueue::new()),
            )),
            registry: Arc::new(Registry::new(tempfile::tempdir().unwrap().path()).unwrap()),
            device_manager: Arc::new(DeviceManager::new(crate::devices::PushConfig::default())),
            llm_proxy: None,
            shield_alerts: Arc::new(ShieldAlertManager::new()),
            audit_store: Arc::new(AuditStore::new(&tempfile::tempdir().unwrap().path().join("audit.jsonl"), None)),
            metrics: Arc::new(Metrics::new()),
            billing: None,
            db: None,
            config_reloader: None,
            e2ee: Arc::new(crate::e2ee::E2eeSessionManager::new()),
            usage_tracker: Arc::new(crate::billing_middleware::UsageTracker::new()),
            rate_limiter: Arc::new(RateLimiter::new(100, 10)),
            control_plane: crate::control_plane::ControlPlaneState::new(),
            email_queue: std::sync::OnceLock::new(),
            tg_bot: std::sync::OnceLock::new(),
            notification_router: std::sync::OnceLock::new(),
            auth_engine: None,
            email_service: None,
            auth: Arc::new(AuthManager::new(None)),
            tochka: None,
            notification_store: None,
            rbac: Arc::new(crate::rbac_manager::RbacManager::new()),
            auth_rate_limiter: Arc::new(crate::auth_rate_limiter::AuthRateLimiter::new()),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["db"], "disabled");
    }

    #[tokio::test]
    async fn test_list_agents_empty() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().uri("/api/agents").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_agents_with_registered() {
        let state = test_state();
        state.pool.register(AgentInfo {
            agent_id: "a1".into(), hostname: "h1".into(), os: "linux".into(),
            arch: "x86_64".into(), connected_at: 1000, last_heartbeat: 1000,
            labels: vec![], capabilities: vec![], online: true,
        });
        let app = build_router(state);
        let resp = app.oneshot(HttpRequest::builder().uri("/api/agents").body(Body::empty()).unwrap()).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_build_router_does_not_panic() {
        let _ = build_router(test_state());
    }

    #[tokio::test]
    async fn test_shield_alerts_flow() {
        let state = test_state();
        state.shield_alerts.add(ShieldAlertEntry {
            alert_id: "al-1".into(), pid: 1234, uid: 1000, username: "root".into(),
            command: "rm -rf /".into(), rule_name: "danger".into(), action: "block".into(),
            snapshot: None, timestamp: 1000, agent_id: None, resolved: false, approved: None,
        });
        let (total, pending, resolved) = state.shield_alerts.stats();
        assert_eq!(total, 1);
        assert_eq!(pending, 1);
        assert_eq!(resolved, 0);
        assert!(state.shield_alerts.resolve_by_pid(1234, true));
        assert!(state.shield_alerts.list_active().is_empty());
    }

    #[tokio::test]
    async fn test_llm_not_configured() {
        let app = build_router(test_state());
        let req_body = r#"{"messages":[{"role":"user","content":"hi"}]}"#;
        let resp = app.oneshot(HttpRequest::builder().method("POST").uri("/api/llm")
            .header("content-type", "application/json")
            .body(Body::new(req_body.to_string())).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_mcp_initialize() {
        let app = build_router(test_state());
        let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"});
        let resp = app.oneshot(HttpRequest::builder().method("POST").uri("/mcp")
            .header("content-type", "application/json")
            .body(Body::new(serde_json::to_string(&body).unwrap())).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json["result"]["serverInfo"]["name"], "flowlink-relay");
    }

    #[tokio::test]
    async fn test_device_pair() {
        let app = build_router(test_state());
        let pair_body = serde_json::json!({"user_id": "u1"});
        let resp = app.oneshot(HttpRequest::builder().method("POST").uri("/api/devices/pair")
            .header("content-type", "application/json")
            .body(Body::new(serde_json::to_string(&pair_body).unwrap())).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        let code = json["code"].as_str().unwrap();
        assert_eq!(code.len(), 6);
    }

    #[tokio::test]
    async fn test_sse_no_token_returns_401() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().uri("/api/events").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sse_invalid_token_returns_401() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().uri("/api/events?token=badtoken").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sse_valid_token_returns_stream() {
        let state = test_state();
        // Register a client with a known token
        let client = crate::auth::Client {
            client_id: "test-client".into(),
            api_token: "test-token-123".into(),
            name: "test".into(),
            active: true,
        };
        state.handler.auth.register_client(client);

        let app = build_router(state);
        let resp = app.oneshot(HttpRequest::builder().uri("/api/events?token=test-token-123&channels=heartbeat").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(ct.contains("text/event-stream"), "expected text/event-stream, got {ct}");
    }

    #[tokio::test]
    async fn test_sse_events_received() {
        let state = test_state();
        let client = crate::auth::Client {
            client_id: "sub-client".into(),
            api_token: "sub-token-456".into(),
            name: "sub".into(),
            active: true,
        };
        state.handler.auth.register_client(client);
        let eventbus = state.eventbus.clone();

        let app = build_router(state);

        // Verify endpoint is accessible with valid token
        let resp = app.oneshot(HttpRequest::builder().uri("/api/events?token=sub-token-456&channels=test_channel").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify EventBus works independently
        let mut rx = eventbus.subscribe("test_channel");
        eventbus.publish("test_channel", r#"{\"type\":\"ping\"}"#);
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg, r#"{\"type\":\"ping\"}"#);
    }

    #[tokio::test]
    async fn test_message_routing_via_eventbus() {
        let state = test_state();
        let eventbus = state.eventbus.clone();

        let mut sub = eventbus.subscribe("heartbeat");
        eventbus.publish("heartbeat", r#"{\"agent_id\":\"a1\"}"#);
        let msg = sub.recv().await.unwrap();
        assert_eq!(msg, r#"{\"agent_id\":\"a1\"}"#);
    }

    #[tokio::test]
    async fn test_device_pair_confirm_and_list() {
        let state = test_state();

        // 1. Request pairing
        let pair_body = serde_json::json!({"user_id": "u1"});
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().method("POST").uri("/api/devices/pair")
            .header("content-type", "application/json")
            .body(Body::new(serde_json::to_string(&pair_body).unwrap())).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        let code = json["code"].as_str().unwrap().to_string();
        assert_eq!(code.len(), 6);

        // 2. Confirm pairing with correct code
        let confirm_body = serde_json::json!({"code": code, "name": "iPhone", "device_type": "ios"});
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().method("POST").uri("/api/devices/confirm")
            .header("content-type", "application/json")
            .body(Body::new(serde_json::to_string(&confirm_body).unwrap())).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert!(json["token"].is_string());
        assert!(json["device"]["id"].is_string());
        let device_id = json["device"]["id"].as_str().unwrap().to_string();

        // 3. List devices
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().uri("/api/devices?user_id=u1")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);

        // 4. Remove device
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().method("DELETE")
            .uri(&format!("/api/devices/{device_id}"))
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // 5. Device list should be empty now
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().uri("/api/devices?user_id=u1")
            .body(Body::empty()).unwrap()).await.unwrap();
        let resp_body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_device_confirm_wrong_code() {
        let state = test_state();
        let confirm_body = serde_json::json!({"code": "000000", "name": "X", "device_type": "ios"});
        let resp = build_router(state).oneshot(HttpRequest::builder().method("POST").uri("/api/devices/confirm")
            .header("content-type", "application/json")
            .body(Body::new(serde_json::to_string(&confirm_body).unwrap())).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_device_remove_not_found() {
        let state = test_state();
        let resp = build_router(state).oneshot(HttpRequest::builder().method("DELETE")
            .uri("/api/devices/nonexistent-id")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_404() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().uri("/nonexistent").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_config_reload_not_enabled() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().method("POST").uri("/api/admin/config/reload")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_config_get_not_enabled() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().uri("/api/config")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_config_push_agent_not_enabled() {
        let app = build_router(test_state());
        let resp = app.oneshot(HttpRequest::builder().method("POST").uri("/api/config/push/a1")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_config_reload_with_reloader() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"api_token":"tok","http_addr":"0.0.0.0:9090"}"#).unwrap();

        let config = flowlink_core::config::RelayConfig::load(config_path.to_str().unwrap()).unwrap();
        let shared_config = Arc::new(tokio::sync::RwLock::new(config));
        let handler = Arc::new(RelayHandler::new(
            Arc::new(AgentPool::new()),
            Arc::new(AuthManager::new(None)),
            Arc::new(EventBus::new()),
            Arc::new(ApprovalQueue::new()),
        ));
        let metrics = Arc::new(Metrics::new());
        let reloader = Arc::new(crate::config_reload::ConfigReloader::new(
            config_path, shared_config, handler, metrics,
        ));

        let mut state = test_state();
        state.config_reloader = Some(reloader.clone());

        // GET config
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().uri("/api/config")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["http_addr"], "0.0.0.0:9090");
        assert_eq!(json["reload_count"], 0);

        // POST reload
        let resp = build_router(state.clone()).oneshot(HttpRequest::builder().method("POST").uri("/api/admin/config/reload")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["reload_count"], 1);
    }
}

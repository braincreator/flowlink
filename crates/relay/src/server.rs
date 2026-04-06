use axum::{
    extract::{Path, Query, State, ws::{WebSocket, WebSocketUpgrade, Message as AxumMsg}},
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
use std::{convert::Infallible, pin::Pin};
use futures_util::stream::Stream;
use std::sync::Arc;
// StreamExt comes from futures_util (re-exported via axum)

use crate::approval::{ApprovalDecision, ApprovalQueue};
use crate::devices::DeviceManager;
use crate::eventbus::EventBus;
use crate::handler::RelayHandler;
use crate::pool::{AgentInfo, AgentPool};
use crate::registry::Registry;

// ═══════════════════════════════════════════════
// Shared State
// ═══════════════════════════════════════════════

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<AgentPool>,
    pub approvals: Arc<ApprovalQueue>,
    pub eventbus: Arc<EventBus>,
    pub handler: Arc<RelayHandler>,
    pub registry: Arc<Registry>,
    pub device_manager: Arc<DeviceManager>,
}

// ═══════════════════════════════════════════════
// Response types
// ═══════════════════════════════════════════════

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    agents: usize,
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
    #[serde(default = "default_channels")]
    channels: String,
}

fn default_channels() -> String { "all".into() }

// ═══════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        agents: state.pool.count(),
    })
}

async fn list_agents(State(state): State<AppState>) -> Json<Vec<AgentInfo>> {
    Json(state.pool.list())
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
async fn sse_events(
    State(state): State<AppState>,
    Query(params): Query<SseParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
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
        .map(|event| Ok::<Event, Infallible>(event));

    Sse::new(stream).keep_alive(KeepAlive::default())
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

    info!("WS upgrade for agent {} (client {})", agent_id, client.client_id);

    ws.on_upgrade(move |socket| handle_ws(socket, agent_id, state))
}

async fn handle_ws(socket: WebSocket, agent_id: String, state: AppState) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AxumMsg>(256);

    // Register sender
    state.handler.register_sender(agent_id.clone(), tx.clone());

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
    });

    // Send Connected ack
    let connected = flowlink_core::Message::new(flowlink_core::MessageType::Connected)
        .with_agent_id(&agent_id)
        .with_payload(flowlink_core::ConnectedPayload {
            agent_id: agent_id.clone(),
            relay_id: "relay-0".into(),
            heartbeat_interval_sec: 30,
            server_time: chrono::Utc::now().timestamp(),
            relay_public_key: None,
            relay_key_id: None,
        });
    if let Ok(json) = serde_json::to_string(&connected) {
        let _ = ws_sink.send(AxumMsg::Text(json.into())).await;
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
                    if let Ok(msg) = serde_json::from_str::<flowlink_core::Message>(&text_str) {
                        match msg.msg_type {
                            flowlink_core::MessageType::Heartbeat => {
                                pool.update_heartbeat(&aid);
                                eventbus.publish("heartbeat", &text_str);
                            }
                            flowlink_core::MessageType::ExecDone => {
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
                            }
                            flowlink_core::MessageType::SysInfo => {
                                eventbus.publish("sysinfo", &text_str);
                            }
                            flowlink_core::MessageType::Disconnect => break,
                            other => {
                                log::info!("Agent {aid}: {:?}", other);
                            }
                        }
                    }
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

    tokio::select! {
        _ = read_task => {},
        _ = write_task => {},
    }

    pool.unregister(&aid);
    state.handler.remove_sender(&aid);
    eventbus.publish("agent_disconnect", &serde_json::to_string(&serde_json::json!({"agent_id": aid})).unwrap_or_default());
}

// ═══════════════════════════════════════════════
// Router Builder
// ═══════════════════════════════════════════════

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/agents", get(list_agents))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}/approve", post(approve_approval))
        .route("/api/approvals/{id}/reject", post(reject_approval))
        .route("/api/clients", get(list_clients))
        .route("/api/exec/{agent_id}", post(exec_agent))
        .route("/api/events", get(sse_events))
        .route("/ws", get(ws_upgrade))
        .route("/mcp", axum::routing::post(crate::mcp::handle_mcp))
        .route("/api/devices/pair", axum::routing::post(crate::devices::pair_device))
        .route("/api/devices/confirm", axum::routing::post(crate::devices::confirm_pairing))
        .route("/api/devices", axum::routing::get(crate::devices::list_devices))
        .route("/api/devices/{id}", axum::routing::delete(crate::devices::remove_device))
        .with_state(state)
}

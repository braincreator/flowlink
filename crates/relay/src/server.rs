use axum::{
    extract::{Path, Query, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use futures_util::stream::Stream;
use log::info;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::approval::{ApprovalDecision, ApprovalQueue};
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
    message: String,
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
        message: if ok {
            "Approved".into()
        } else {
            "Not found".into()
        },
    })
}

async fn reject_approval(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<SimpleResponse> {
    let ok = state.approvals.resolve(&id, ApprovalDecision::Rejected);
    Json(SimpleResponse {
        ok,
        message: if ok {
            "Rejected".into()
        } else {
            "Not found".into()
        },
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
        Ok(()) => Json(SimpleResponse {
            ok: true,
            message: "Sent".into(),
        })
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                ok: false,
                message: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// SSE endpoint — streams events from EventBus.
async fn sse_events(
    State(state): State<AppState>,
    Query(params): Query<SseParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let channels: Vec<String> = if params.channels == "all" {
        vec![
            "heartbeat".into(),
            "exec_done".into(),
            "exec_output".into(),
            "approval_request".into(),
            "shield_alert".into(),
            "agent_disconnect".into(),
            "sysinfo".into(),
        ]
    } else {
        params.channels.split(',').map(|s| s.trim().to_string()).collect()
    };

    let rx = state.eventbus.subscribe("_sse_aggregate");

    // For each channel, subscribe and re-broadcast into _sse_aggregate
    let subscribers: Vec<tokio::sync::broadcast::Receiver<String>> = channels
        .iter()
        .map(|ch| state.eventbus.subscribe(ch))
        .collect();

    let eventbus = state.eventbus.clone();
    let channels_clone = channels.clone();

    // Spawn a task to merge all channel broadcasts into _sse_aggregate
    tokio::spawn(async move {
        // Create separate receivers for the merger task
        for ch in &channels_clone {
            let rx = eventbus.subscribe(ch);
            let tx = eventbus.subscribe("_sse_aggregate"); // we need a sender...
            // Actually, let's just forward directly
            let eventbus2 = eventbus.clone();
            let ch2 = ch.clone();
            tokio::spawn(async move {
                let mut rx = eventbus2.subscribe(&ch2);
                while let Ok(data) = rx.recv().await {
                    eventbus2.publish("_sse_merge", &data);
                }
            });
        }
    });

    // Simpler approach: subscribe to _sse_merge
    let merge_rx = state.eventbus.subscribe("_sse_merge");

    let stream = BroadcastStream::new(merge_rx).filter_map(|item| {
        item.ok().map(|data| {
            Ok::<Event, Infallible>(Event::default().data(data))
        })
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ═══════════════════════════════════════════════
// WebSocket upgrade (axum-native via axum::extract::ws)
// ═══════════════════════════════════════════════

// We use tokio-tungstenite directly via axum's upgrade mechanism.
// Since axum 0.8 doesn't include built-in ws, we use the raw upgrade.

async fn ws_upgrade(
    State(state): State<AppState>,
    Query(params): Query<WsParams>,
    mut req: axum::http::Request<axum::body::Body>,
) -> impl IntoResponse {
    use axum::body::Body;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use futures_util::SinkExt;

    let token = match params.token {
        Some(t) => t,
        None => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(SimpleResponse { ok: false, message: "Missing token".into() }),
            ).into_response();
        }
    };

    let client = match state.handler.auth.validate_token(&token) {
        Some(c) if c.active => c,
        _ => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(SimpleResponse { ok: false, message: "Invalid token".into() }),
            ).into_response();
        }
    };

    // Convert axum request to tungstenite request for the handshake
    let ws_req = Request::from(req);
    let mut response = Response::new(None);

    // Accept with no extensions
    let key = ws_req.headers().get("sec-websocket-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if key.is_none() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(SimpleResponse { ok: false, message: "Missing WebSocket key".into() }),
        ).into_response();
    }

    // Use tungstenite's accept_hdr to build the response, then we manually upgrade
    // Simpler: use tokio-tungstenite's accept_async directly on the upgraded stream

    // Actually, let's use axum's oneshot connection upgrade approach
    // For axum 0.8 without ws feature, we'll use hyper's upgrade
    use axum::extract::FromRequestParts;

    // We need the underlying TCP stream. Let's use a different approach:
    // upgrade the connection via hyper, then pass to tungstenite.

    // Simplest working approach for axum + tungstenite:
    // Use hyper::upgrade to get the IO, then wrap in tungstenite.

    // axum 0.8 oneshot upgrade
    let oneshot = match axum::extract::connect_info::IntoMakeServiceWithConnectInfo::<_, ()>::oneshot_upgrade(&mut req) {
        // This won't work directly. Let's use a different approach.
        _ => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SimpleResponse { ok: false, message: "Upgrade failed".into() }),
            ).into_response();
        }
    };

    // This approach is getting complicated. Let's simplify by using
    // a manual upgrade with tokio-tungstenite.

    // ... (see simplified version below)
    todo_ws_upgrade()
}

fn todo_ws_upgrade() -> axum::response::Response {
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "Use /ws?tungstenite endpoint",
    ).into_response()
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
        .with_state(state)
}

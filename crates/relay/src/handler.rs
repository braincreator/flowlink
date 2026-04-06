// HTTP + WebSocket handler
// Port of internal/relay/relay.go HTTP routes

use dashmap::DashMap;
use flowlink_core::Message;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::approval::ApprovalQueue;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;
use crate::pool::AgentPool;

/// A handle to a connected agent's WebSocket sink.
type WsSender = mpsc::Sender<WsMessage>;

pub struct RelayHandler {
    pool: Arc<AgentPool>,
    auth: Arc<AuthManager>,
    eventbus: Arc<EventBus>,
    approvals: Arc<ApprovalQueue>,
    /// Active WS senders keyed by agent_id.
    ws_senders: Arc<DashMap<String, WsSender>>,
}

impl RelayHandler {
    pub fn new(
        pool: Arc<AgentPool>,
        auth: Arc<AuthManager>,
        eventbus: Arc<EventBus>,
        approvals: Arc<ApprovalQueue>,
    ) -> Self {
        Self {
            pool,
            auth,
            eventbus,
            approvals,
            ws_senders: Arc::new(DashMap::new()),
        }
    }

    /// Handle incoming WebSocket connection from agent.
    pub async fn handle_agent_ws(
        &self,
        agent_id: String,
        ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) {
        let (mut ws_sink, mut ws_stream) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<WsMessage>(256);

        self.ws_senders.insert(agent_id.clone(), tx.clone());

        // Send Connected ack
        let connected = Message::new(flowlink_core::MessageType::Connected)
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
            let _ = ws_sink.send(WsMessage::Text(json.into())).await;
        }

        let aid = agent_id.clone();
        let senders = self.ws_senders.clone();
        let pool = self.pool.clone();
        let eventbus = self.eventbus.clone();
        let approvals = self.approvals.clone();

        // Read task
        let read_task = tokio::spawn(async move {
            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        let text_str: String = text.to_string();
                        if let Ok(msg) = serde_json::from_str::<Message>(&text_str) {
                            match msg.msg_type {
                                flowlink_core::MessageType::Heartbeat => {
                                    pool.update_heartbeat(&aid);
                                    // publish to eventbus for SSE clients
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
                                flowlink_core::MessageType::Disconnect => {
                                    break;
                                }
                                other => {
                                    info!("Agent {aid}: {:?}", other);
                                }
                            }
                        }
                    }
                    Ok(WsMessage::Ping(data)) => {
                        // tungstenite auto-responds, but just in case
                        let _ = tx.send(WsMessage::Pong(data)).await;
                    }
                    Ok(WsMessage::Close(_)) => break,
                    Err(e) => {
                        error!("Agent {aid} WS error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
            pool.unregister(&aid);
            senders.remove(&aid);
            eventbus.publish("agent_disconnect", &serde_json::to_string(&serde_json::json!({"agent_id": aid})).unwrap_or_default());
        });

        // Write task — forward queued messages to the WebSocket sink
        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_sink.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let _ = tokio::join!(read_task, write_task);
    }

    /// Send a message to a specific connected agent.
    pub async fn send_to_agent(&self, agent_id: &str, msg: Message) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        let ws_msg = WsMessage::Text(json.into());
        if let Some(sender) = self.ws_senders.get(agent_id) {
            sender.send(ws_msg).await?;
            Ok(())
        } else {
            anyhow::bail!("Agent {agent_id} not connected");
        }
    }
}

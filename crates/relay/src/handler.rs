// HTTP + WebSocket handler
// Port of internal/relay/relay.go HTTP routes

use flowlink_core::Message;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn, error};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::pool::AgentPool;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;

pub struct RelayHandler {
    pool: AgentPool,
    auth: AuthManager,
    eventbus: EventBus,
}

impl RelayHandler {
    pub fn new(pool: AgentPool, auth: AuthManager, eventbus: EventBus) -> Self {
        Self { pool, auth, eventbus }
    }

    /// Handle incoming WebSocket connection from agent.
    pub async fn handle_agent_ws(
        &self,
        agent_id: String,
        ws_stream: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        mut ws_sink: futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            WsMessage,
        >,
    ) {
        let mut stream = ws_stream;

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Err(e) = self.handle_agent_message(&agent_id, &text).await {
                        warn!("Agent {agent_id} message error: {e}");
                    }
                }
                Ok(WsMessage::Ping(data)) => {
                    if ws_sink.send(WsMessage::Pong(data)).await.is_err() {
                        break;
                    }
                }
                Ok(WsMessage::Close(_)) => break,
                Err(e) => {
                    error!("Agent {agent_id} WS error: {e}");
                    break;
                }
                _ => {}
            }
        }

        self.pool.unregister(&agent_id);
    }

    async fn handle_agent_message(&self, agent_id: &str, text: &str) -> anyhow::Result<()> {
        let msg: Message = serde_json::from_str(text)?;
        match msg.msg_type {
            flowlink_core::MessageType::Heartbeat => {
                self.pool.update_heartbeat(agent_id);
            }
            flowlink_core::MessageType::ExecDone => {
                self.eventbus.publish("exec_done", text);
            }
            other => {
                info!("Agent {agent_id}: {:?}", other);
            }
        }
        Ok(())
    }
}

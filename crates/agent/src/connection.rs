// WebSocket connection to relay with auto-reconnect
// Port of internal/agent/connection.go

use flowlink_core::*;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn, error};
use tokio_tungstenite::tungstenite::Message as WsMessage;

pub struct Connection {
    url: String,
    agent_id: String,
    token: String,
}

impl Connection {
    pub fn new(url: String, agent_id: String, token: String) -> Self {
        Self { url, agent_id, token }
    }

    /// Connect, authenticate, run message loop with auto-reconnect + exponential backoff.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut backoff_secs: u64 = 1;
        const MAX_BACKOFF: u64 = 60;

        loop {
            match self.connect_and_loop().await {
                Ok(()) => {
                    info!("Connection closed cleanly, reconnecting...");
                    backoff_secs = 1;
                }
                Err(e) => {
                    error!("Connection error: {e}, reconnecting in {backoff_secs}s...");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF);
        }
    }

    async fn connect_and_loop(&mut self) -> anyhow::Result<()> {
        let ws_url = format!("{}/ws?agent_id={}&token={}", self.url, self.agent_id, self.token);

        let (mut ws_stream, _resp) = tokio_tungstenite::connect_async(&ws_url).await?;
        info!("Connected to relay {}", self.url);

        // Send connect message
        let connect_payload = ConnectPayload {
            agent_id: self.agent_id.clone(),
            token: self.token.clone(),
            hostname: get_hostname(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            client_version: Some(env!("CARGO_PKG_VERSION").into()),
            public_key: None,
            protocol_version: Some(PROTOCOL_VERSION),
        };

        let connect_msg = Message::new(MessageType::Connect)
            .with_agent_id(&self.agent_id)
            .with_payload(connect_payload);

        let json = serde_json::to_string(&connect_msg)?;
        ws_stream.send(WsMessage::Text(json.into())).await?;

        // Message loop
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Err(e) = self.handle_message(&text).await {
                        warn!("Failed to handle message: {e}");
                    }
                }
                Ok(WsMessage::Ping(data)) => {
                    ws_stream.send(WsMessage::Pong(data)).await?;
                }
                Ok(WsMessage::Close(_)) => {
                    info!("Relay closed connection");
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) -> anyhow::Result<()> {
        let msg: Message = serde_json::from_str(text)?;
        info!("Received: {:?}", msg.msg_type);
        Ok(())
    }
}

fn get_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
        .unwrap_or_default()
}

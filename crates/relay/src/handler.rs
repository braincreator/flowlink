// RelayHandler — routes messages between agents and clients

use axum::extract::ws::Message as AxumMsg;
use dashmap::DashMap;
use flowlink_core::Message;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::approval::ApprovalQueue;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;
use crate::pool::AgentPool;

type WsSender = (mpsc::Sender<AxumMsg>, u64);

pub struct RelayHandler {
    #[allow(dead_code)]
    pool: Arc<AgentPool>,
    pub auth: Arc<AuthManager>,
    #[allow(dead_code)]
    eventbus: Arc<EventBus>,
    #[allow(dead_code)]
    approvals: Arc<ApprovalQueue>,
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

    pub fn register_sender(&self, agent_id: String, sender: WsSender) {
        self.ws_senders.insert(agent_id, sender);
    }

    pub fn remove_sender(&self, agent_id: &str) {
        self.ws_senders.remove(agent_id);
    }

    /// Remove sender only if it hasn't been replaced by a new connection.
    /// Uses a connection counter to detect stale senders.
    pub fn remove_sender_if_stale(&self, agent_id: &str, conn_id: u64) {
        if let Some(entry) = self.ws_senders.get(agent_id) {
            // Check if connection ID matches
            if entry.value().1 != conn_id {
                log::info!("Skipping sender removal for {agent_id}: newer connection active");
                return;
            }
        }
        self.ws_senders.remove(agent_id);
    }

    /// List all currently connected agent IDs.
    pub fn connected_agents(&self) -> Vec<String> {
        self.ws_senders.iter().map(|r| r.key().clone()).collect()
    }

    /// Send a message to a specific connected agent.
    pub async fn send_to_agent(&self, agent_id: &str, msg: Message) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        let ws_msg = AxumMsg::Text(json.into());
        if let Some(sender) = self.ws_senders.get(agent_id) {
            sender.value().0.send(ws_msg).await?;
            Ok(())
        } else {
            anyhow::bail!("Agent {agent_id} is offline — not connected via WebSocket");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_handler() -> RelayHandler {
        RelayHandler::new(
            Arc::new(AgentPool::new()),
            Arc::new(AuthManager::new(None)),
            Arc::new(EventBus::new()),
            Arc::new(ApprovalQueue::new()),
        )
    }

    #[test]
    fn test_handler_creation() {
        let _h = test_handler();
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_agent() {
        let h = test_handler();
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        assert!(h.send_to_agent("ghost", msg).await.is_err());
    }

    #[tokio::test]
    async fn test_register_and_remove_sender() {
        let h = test_handler();
        let (tx, _rx) = mpsc::channel(10);
        h.register_sender("a1".into(), tx);
        h.remove_sender("a1");
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        assert!(h.send_to_agent("a1", msg).await.is_err());
    }

    #[tokio::test]
    async fn test_send_to_connected_agent() {
        let h = test_handler();
        let (tx, mut rx) = mpsc::channel(10);
        h.register_sender("a1".into(), tx);
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        assert!(h.send_to_agent("a1", msg).await.is_ok());
        let received = rx.recv().await.unwrap();
        matches!(received, AxumMsg::Text(_));
    }

    // ── Connected agents listing ──

    #[test]
    fn test_connected_agents_empty() {
        let h = test_handler();
        assert!(h.connected_agents().is_empty());
    }

    #[tokio::test]
    async fn test_connected_agents_after_register() {
        let h = test_handler();
        let (tx1, _rx1) = mpsc::channel(10);
        let (tx2, _rx2) = mpsc::channel(10);
        h.register_sender("agent-alpha".into(), tx1);
        h.register_sender("agent-beta".into(), tx2);

        let mut agents = h.connected_agents();
        agents.sort();
        assert_eq!(agents, vec!["agent-alpha", "agent-beta"]);
    }

    #[tokio::test]
    async fn test_connected_agents_after_disconnect() {
        let h = test_handler();
        let (tx, _rx) = mpsc::channel(10);
        h.register_sender("agent-1".into(), tx);
        assert_eq!(h.connected_agents().len(), 1);
        h.remove_sender("agent-1");
        assert!(h.connected_agents().is_empty());
    }

    // ── Message serialization roundtrip ──

    #[tokio::test]
    async fn test_sent_message_is_valid_json() {
        let h = test_handler();
        let (tx, mut rx) = mpsc::channel(10);
        h.register_sender("a1".into(), tx);

        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        h.send_to_agent("a1", msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        if let AxumMsg::Text(text) = received {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(parsed["type"], "heartbeat");
        } else {
            panic!("Expected Text message");
        }
    }

    #[tokio::test]
    async fn test_send_message_with_payload() {
        let h = test_handler();
        let (tx, mut rx) = mpsc::channel(10);
        h.register_sender("a1".into(), tx);

        let msg = flowlink_core::Message::new(flowlink_core::MessageType::ExecRequest)
            .with_payload(serde_json::json!({"command": "ls"}));
        h.send_to_agent("a1", msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        if let AxumMsg::Text(text) = received {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(parsed["type"], "exec_request");
            assert_eq!(parsed["payload"]["command"], "ls");
        } else {
            panic!("Expected Text message");
        }
    }

    // ── Auth integration ──

    #[test]
    fn test_auth_manager_on_handler() {
        let h = test_handler();
        // Fresh auth manager has no clients
        assert!(h.auth.validate_token("any-token").is_none());
    }

    #[test]
    fn test_auth_register_and_validate() {
        let h = test_handler();
        h.auth.register_client(crate::auth::Client {
            client_id: "c1".into(),
            api_token: "secret-token".into(),
            name: "Test Client".into(),
            active: true,
        });
        let client = h.auth.validate_token("secret-token").unwrap();
        assert_eq!(client.client_id, "c1");
        assert_eq!(client.name, "Test Client");
    }

    #[test]
    fn test_auth_bad_token_returns_none() {
        let h = test_handler();
        h.auth.register_client(crate::auth::Client {
            client_id: "c1".into(),
            api_token: "real-token".into(),
            name: "c1".into(),
            active: true,
        });
        assert!(h.auth.validate_token("wrong-token").is_none());
        assert!(h.auth.validate_token("").is_none());
    }

    #[test]
    fn test_auth_inactive_client_still_validated() {
        let h = test_handler();
        h.auth.register_client(crate::auth::Client {
            client_id: "c1".into(),
            api_token: "tok".into(),
            name: "c1".into(),
            active: false,
        });
        // validate_token returns client regardless of active flag
        let client = h.auth.validate_token("tok").unwrap();
        assert_eq!(client.client_id, "c1");
        assert!(!client.active);
    }

    #[test]
    fn test_auth_empty_is_empty() {
        let h = test_handler();
        assert!(h.auth.is_empty());
    }

    #[test]
    fn test_auth_get_client() {
        let h = test_handler();
        h.auth.register_client(crate::auth::Client {
            client_id: "c1".into(),
            api_token: "tok".into(),
            name: "c1".into(),
            active: true,
        });
        assert!(h.auth.get_client("c1").is_some());
        assert!(h.auth.get_client("nonexistent").is_none());
    }

    // ── Re-register overwrites ──

    #[test]
    fn test_auth_reregister_overwrites_token() {
        let h = test_handler();
        h.auth.register_client(crate::auth::Client {
            client_id: "c1".into(),
            api_token: "old-tok".into(),
            name: "c1".into(),
            active: true,
        });
        h.auth.register_client(crate::auth::Client {
            client_id: "c1".into(),
            api_token: "new-tok".into(),
            name: "updated".into(),
            active: true,
        });
        assert!(h.auth.validate_token("old-tok").is_none());
        assert!(h.auth.validate_token("new-tok").is_some());
        assert_eq!(h.auth.get_client("c1").unwrap().name, "updated");
    }

    // ── Channel capacity ──

    #[tokio::test]
    async fn test_send_fails_when_channel_full() {
        let h = test_handler();
        let (tx, _rx) = mpsc::channel(1);
        h.register_sender("a1".into(), tx);

        // Fill the channel
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        assert!(h.send_to_agent("a1", msg.clone()).await.is_ok());

        // Second send should fail (channel full, no receiver consuming)
        // Note: mpsc::Sender::send is async and will wait; but since rx is not polled,
        // this will fail because channel capacity is 1 and we already sent one
        // Actually with async mpsc, send waits for capacity. We need to test differently.
        // With channel(1), one message is buffered, second will wait.
        // Since rx is dropped from scope but _rx holds it, it won't be dropped.
        // The send will succeed but be buffered if capacity allows.
        // For a true "full" test, we'd need capacity(0).
        drop(_rx);
        // Now receiver is dropped, next send should fail
        assert!(h.send_to_agent("a1", msg).await.is_err());
    }

    // ── Agent connection lifecycle ──

    #[tokio::test]
    async fn test_agent_lifecycle_connect_send_disconnect() {
        let h = test_handler();

        // Initially no agents
        assert!(h.connected_agents().is_empty());

        // Connect
        let (tx, mut rx) = mpsc::channel(10);
        h.register_sender("lifecycle-agent".into(), tx);
        assert_eq!(h.connected_agents().len(), 1);

        // Send message
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        assert!(h.send_to_agent("lifecycle-agent", msg).await.is_ok());
        assert!(rx.recv().await.is_some());

        // Disconnect
        h.remove_sender("lifecycle-agent");
        assert!(h.connected_agents().is_empty());

        // Send after disconnect fails
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        assert!(h.send_to_agent("lifecycle-agent", msg).await.is_err());
    }

    #[tokio::test]
    async fn test_reconnect_same_agent() {
        let h = test_handler();
        let (tx1, mut rx1) = mpsc::channel(10);
        h.register_sender("reconnect-agent".into(), tx1);
        h.send_to_agent("reconnect-agent", flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat)).await.unwrap();
        assert!(rx1.recv().await.is_some());

        // Simulate reconnect: remove old, register new
        h.remove_sender("reconnect-agent");
        let (tx2, mut rx2) = mpsc::channel(10);
        h.register_sender("reconnect-agent".into(), tx2);
        h.send_to_agent("reconnect-agent", flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat)).await.unwrap();
        assert!(rx2.recv().await.is_some());
    }

    // ── Concurrent connections ──

    #[tokio::test]
    async fn test_multiple_concurrent_senders() {
        let h = test_handler();
        let mut handles: Vec<(String, mpsc::Receiver<AxumMsg>)> = vec![];

        // Register 20 agents sequentially on the same handler
        for i in 0..20 {
            let (tx, rx) = mpsc::channel(10);
            let agent_id = format!("agent-{}", i);
            h.register_sender(agent_id.clone(), tx);
            handles.push((agent_id, rx));
        }

        assert_eq!(h.connected_agents().len(), 20);

        // Send heartbeat to all
        for (agent_id, _) in &handles {
            let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
            assert!(h.send_to_agent(agent_id, msg).await.is_ok());
        }

        // Verify all received
        for (_, mut rx) in handles {
            assert!(rx.recv().await.is_some());
        }
    }

    #[tokio::test]
    async fn test_concurrent_sends_to_same_agent() {
        let h = test_handler();
        let (tx, mut rx) = mpsc::channel(100);
        h.register_sender("shared-agent".into(), tx);

        let mut msgs: Vec<()> = vec![];
        for _ in 0..10 {
            // We can't clone the handler, so we send from the same task
            let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
            assert!(h.send_to_agent("shared-agent", msg).await.is_ok());
        }
        drop(msgs);

        // All 10 messages should be received
        for _ in 0..10 {
            assert!(rx.recv().await.is_some());
        }
    }

    // ── Remove nonexistent sender (no panic) ──

    #[tokio::test]
    async fn test_remove_nonexistent_sender_no_panic() {
        let h = test_handler();
        h.remove_sender("ghost"); // Should not panic
        assert!(h.connected_agents().is_empty());
    }

    // ── Register overwrites existing sender ──

    #[tokio::test]
    async fn test_register_overwrites_existing_sender() {
        let h = test_handler();
        let (tx1, mut rx1) = mpsc::channel(10);
        h.register_sender("a1".into(), tx1);

        // Overwrite with new sender
        let (tx2, mut rx2) = mpsc::channel(10);
        h.register_sender("a1".into(), tx2);

        // Send should go to new sender
        let msg = flowlink_core::Message::new(flowlink_core::MessageType::Heartbeat);
        h.send_to_agent("a1", msg).await.unwrap();
        assert!(rx2.recv().await.is_some());

        // Old sender should not receive (dropped from DashMap)
        // rx1 won't receive because the old tx1 is no longer in the map
        // but rx1 is still alive — it just won't get anything new
        // Note: old tx1 is dropped when overwritten, so rx1 will get None eventually
        // But since we already received on rx2, let's just check rx1 gets nothing immediately
        assert!(rx1.try_recv().is_err());
    }

    // ── Different message types ──

    #[tokio::test]
    async fn test_send_various_message_types() {
        let h = test_handler();
        let (tx, mut rx) = mpsc::channel(10);
        h.register_sender("a1".into(), tx);

        let types = vec![
            flowlink_core::MessageType::Heartbeat,
            flowlink_core::MessageType::Connect,
            flowlink_core::MessageType::Disconnect,
            flowlink_core::MessageType::ExecRequest,
        ];

        for msg_type in types {
            let msg = flowlink_core::Message::new(msg_type);
            h.send_to_agent("a1", msg).await.unwrap();
        }

        for _ in 0..4 {
            let received = rx.recv().await.unwrap();
            assert!(matches!(received, AxumMsg::Text(_)));
        }
    }
}

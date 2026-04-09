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

type WsSender = mpsc::Sender<AxumMsg>;

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

    /// Send a message to a specific connected agent.
    pub async fn send_to_agent(&self, agent_id: &str, msg: Message) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        let ws_msg = AxumMsg::Text(json.into());
        if let Some(sender) = self.ws_senders.get(agent_id) {
            sender.send(ws_msg).await?;
            Ok(())
        } else {
            anyhow::bail!("Agent {agent_id} not connected");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_handler() -> RelayHandler {
        RelayHandler::new(
            Arc::new(AgentPool::new()),
            Arc::new(AuthManager::new()),
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
}

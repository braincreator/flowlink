// RelayHandler — routes messages between agents and clients

use axum::extract::ws::Message as AxumMsg;
use dashmap::DashMap;
use flowlink_core::Message;
use log::warn;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::approval::ApprovalQueue;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;
use crate::pool::AgentPool;

type WsSender = mpsc::Sender<AxumMsg>;

pub struct RelayHandler {
    pool: Arc<AgentPool>,
    pub auth: Arc<AuthManager>,
    eventbus: Arc<EventBus>,
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

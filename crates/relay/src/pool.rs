// Agent Pool — tracks connected agents (concurrent hashmap)
// Port of internal/relay/relay.go AgentPool

use dashmap::DashMap;
use flowlink_core::MessageType;
use log::{info, warn};
use serde_json;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub agent_id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub connected_at: i64,
    pub last_heartbeat: i64,
    pub labels: Vec<String>,
    pub capabilities: Vec<String>,
}

pub struct AgentPool {
    agents: Arc<DashMap<String, AgentInfo>>,
}

impl AgentPool {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
        }
    }

    pub fn register(&self, info: AgentInfo) {
        info!("Agent registered: {}", info.agent_id);
        self.agents.insert(info.agent_id.clone(), info);
    }

    pub fn unregister(&self, agent_id: &str) {
        if self.agents.remove(agent_id).is_some() {
            info!("Agent disconnected: {agent_id}");
        }
    }

    pub fn get(&self, agent_id: &str) -> Option<AgentInfo> {
        self.agents.get(agent_id).map(|r| r.value().clone())
    }

    pub fn update_heartbeat(&self, agent_id: &str) {
        if let Some(mut agent) = self.agents.get_mut(agent_id) {
            agent.last_heartbeat = chrono::Utc::now().timestamp();
        }
    }

    pub fn list(&self) -> Vec<AgentInfo> {
        self.agents.iter().map(|r| r.value().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.agents.len()
    }
}

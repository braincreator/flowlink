// Agent Pool — tracks connected agents (concurrent hashmap)
// Port of internal/relay/relay.go AgentPool

use dashmap::DashMap;
use log::info;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub connected_at: i64,
    pub last_heartbeat: i64,
    pub labels: Vec<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub online: bool,
}

impl AgentInfo {
    pub fn id(&self) -> &str {
        &self.agent_id
    }
    
    pub fn name(&self) -> &str {
        &self.hostname
    }
    
    pub fn is_connected(&self) -> bool {
        self.online
    }
    
    pub fn last_activity(&self) -> Option<i64> {
        Some(self.last_heartbeat)
    }
}

pub struct AgentPool {
    agents: Arc<DashMap<String, AgentInfo>>,
}

impl Default for AgentPool {
    fn default() -> Self {
        Self::new()
    }
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

    /// Mark agent as offline (WS disconnected) but keep in pool.
    /// The agent record persists — only the live session is gone.
    pub fn set_offline(&self, agent_id: &str) {
        if let Some(mut agent) = self.agents.get_mut(agent_id) {
            agent.online = false;
            info!("Agent went offline: {agent_id}");
        }
    }

    /// Permanently remove agent from pool (explicit deregister).
    pub fn deregister(&self, agent_id: &str) {
        if self.agents.remove(agent_id).is_some() {
            info!("Agent deregistered: {agent_id}");
        }
    }

    pub fn unregister(&self, agent_id: &str) {
        self.set_offline(agent_id);
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

#[cfg(test)]
mod tests {
    use super::*;

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
            online: true,
        }
    }

    #[test]
    fn test_register_and_get() {
        let pool = AgentPool::new();
        pool.register(test_agent("agent-1"));
        let a = pool.get("agent-1").unwrap();
        assert_eq!(a.hostname, "host-agent-1");
    }

    #[test]
    fn test_unregister_sets_offline() {
        let pool = AgentPool::new();
        pool.register(test_agent("a1"));
        assert!(pool.get("a1").unwrap().online);
        pool.unregister("a1");
        let agent = pool.get("a1").unwrap();
        assert!(!agent.online);
    }

    #[test]
    fn test_deregister_removes() {
        let pool = AgentPool::new();
        pool.register(test_agent("a1"));
        pool.deregister("a1");
        assert!(pool.get("a1").is_none());
    }

    #[test]
    fn test_list_agents() {
        let pool = AgentPool::new();
        pool.register(test_agent("a1"));
        pool.register(test_agent("a2"));
        assert_eq!(pool.list().len(), 2);
    }

    #[test]
    fn test_count() {
        let pool = AgentPool::new();
        assert_eq!(pool.count(), 0);
        pool.register(test_agent("a1"));
        assert_eq!(pool.count(), 1);
    }

    #[test]
    fn test_update_heartbeat() {
        let pool = AgentPool::new();
        pool.register(test_agent("a1"));
        pool.update_heartbeat("a1");
        let a = pool.get("a1").unwrap();
        assert!(a.last_heartbeat >= 1000);
    }

    #[test]
    fn test_double_register_overwrite() {
        let pool = AgentPool::new();
        pool.register(AgentInfo { agent_id: "a1".into(), hostname: "old".into(), ..test_agent("a1") });
        pool.register(AgentInfo { agent_id: "a1".into(), hostname: "new".into(), ..test_agent("a1") });
        assert_eq!(pool.get("a1").unwrap().hostname, "new");
        assert_eq!(pool.count(), 1);
    }

    #[test]
    fn test_unregister_nonexistent() {
        let pool = AgentPool::new();
        pool.unregister("ghost"); // should not panic
        assert_eq!(pool.count(), 0);
    }

    #[test]
    fn test_update_heartbeat_nonexistent() {
        let pool = AgentPool::new();
        pool.update_heartbeat("ghost"); // should not panic
    }

    #[test]
    fn test_get_nonexistent() {
        let pool = AgentPool::new();
        assert!(pool.get("nope").is_none());
    }

    #[test]
    fn test_concurrent_register_deregister() {
        use std::sync::Arc;
        use std::thread;
        let pool = Arc::new(AgentPool::new());
        let handles: Vec<_> = (0..20)
            .map(|i| {
                let p = pool.clone();
                thread::spawn(move || {
                    let id = format!("agent-{i}");
                    p.register(test_agent(&id));
                    assert!(p.get(&id).is_some());
                    p.deregister(&id);
                    assert!(p.get(&id).is_none());
                })
            })
            .collect();
        for h in handles { h.join().unwrap(); }
        assert_eq!(pool.count(), 0);
    }

    #[test]
    fn test_default() {
        let pool = AgentPool::default();
        assert_eq!(pool.count(), 0);
    }
}

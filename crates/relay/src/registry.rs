// Registry — persistent client and agent storage (JSON files)
// Port of internal/relay/registry.go

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ═══════════════════════════════════════════════
// Data types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredClient {
    pub id: String,
    pub name: String,
    pub api_token: String,
    #[serde(default)]
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub active: bool,
    #[serde(default = "default_max_agents")]
    pub max_agents: u32,
    #[serde(default)]
    pub exec_count: i64,
    #[serde(default)]
    pub last_activity: Option<DateTime<Utc>>,
}

fn default_max_agents() -> u32 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredAgent {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub token: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_seen: Option<DateTime<Utc>>,
    pub active: bool,
}

// ═══════════════════════════════════════════════
// Registry
// ═══════════════════════════════════════════════

pub struct Registry {
    data_dir: PathBuf,
    clients: Arc<DashMap<String, RegisteredClient>>,
    agents: Arc<DashMap<String, RegisteredAgent>>,
}

impl Registry {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;

        let reg = Self {
            data_dir,
            clients: Arc::new(DashMap::new()),
            agents: Arc::new(DashMap::new()),
        };

        reg.load_clients()?;
        reg.load_agents()?;
        Ok(reg)
    }

    // ── Clients ──

    pub fn register_client(&self, name: String, email: String) -> Result<RegisteredClient> {
        let client = RegisteredClient {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            api_token: generate_token(),
            email,
            created_at: Utc::now(),
            active: true,
            max_agents: default_max_agents(),
            exec_count: 0,
            last_activity: None,
        };
        self.clients.insert(client.id.clone(), client.clone());
        self.save_clients()?;
        Ok(client)
    }

    pub fn get_client(&self, id: &str) -> Option<RegisteredClient> {
        self.clients.get(id).map(|r| r.value().clone())
    }

    pub fn get_client_by_token(&self, token: &str) -> Option<RegisteredClient> {
        self.clients.iter()
            .find(|r| r.value().api_token == token)
            .map(|r| r.value().clone())
    }

    pub fn list_clients(&self) -> Vec<RegisteredClient> {
        self.clients.iter().map(|r| r.value().clone()).collect()
    }

    pub fn deactivate_client(&self, id: &str) -> bool {
        if let Some(mut c) = self.clients.get_mut(id) {
            c.active = false;
            let _ = self.save_clients();
            true
        } else {
            false
        }
    }

    // ── Agents ──

    pub fn register_agent(&self, client_id: &str, name: String, token: String) -> Result<RegisteredAgent> {
        // Check client exists and has room
        let client = self.get_client(client_id)
            .ok_or_else(|| anyhow::anyhow!("Client not found: {client_id}"))?;
        if !client.active {
            anyhow::bail!("Client deactivated: {client_id}");
        }
        let agent_count = self.agents.iter()
            .filter(|r| r.value().client_id == client_id && r.value().active)
            .count() as u32;
        if agent_count >= client.max_agents {
            anyhow::bail!("Agent limit reached ({}/{})", agent_count, client.max_agents);
        }

        let agent = RegisteredAgent {
            id: uuid::Uuid::new_v4().to_string(),
            client_id: client_id.to_string(),
            name,
            token,
            hostname: None,
            os: None,
            arch: None,
            labels: vec![],
            created_at: Utc::now(),
            last_seen: None,
            active: true,
        };
        self.agents.insert(agent.id.clone(), agent.clone());
        self.save_agents()?;
        Ok(agent)
    }

    pub fn get_agent(&self, id: &str) -> Option<RegisteredAgent> {
        self.agents.get(id).map(|r| r.value().clone())
    }

    pub fn get_agent_by_token(&self, token: &str) -> Option<RegisteredAgent> {
        self.agents.iter()
            .find(|r| r.value().token == token)
            .map(|r| r.value().clone())
    }

    pub fn list_agents_for_client(&self, client_id: &str) -> Vec<RegisteredAgent> {
        self.agents.iter()
            .filter(|r| r.value().client_id == client_id)
            .map(|r| r.value().clone())
            .collect()
    }

    pub fn update_agent_heartbeat(&self, id: &str) {
        if let Some(mut a) = self.agents.get_mut(id) {
            a.last_seen = Some(Utc::now());
        }
    }

    // ── Persistence ──

    fn load_clients(&self) -> Result<()> {
        let path = self.data_dir.join("clients.json");
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            let clients: Vec<RegisteredClient> = serde_json::from_str(&data)?;
            for c in clients {
                self.clients.insert(c.id.clone(), c);
            }
        }
        Ok(())
    }

    fn save_clients(&self) -> Result<()> {
        let clients: Vec<_> = self.clients.iter().map(|r| r.value().clone()).collect();
        let json = serde_json::to_string_pretty(&clients)?;
        std::fs::write(self.data_dir.join("clients.json"), json)?;
        Ok(())
    }

    fn load_agents(&self) -> Result<()> {
        let path = self.data_dir.join("agents.json");
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            let agents: Vec<RegisteredAgent> = serde_json::from_str(&data)?;
            for a in agents {
                self.agents.insert(a.id.clone(), a);
            }
        }
        Ok(())
    }

    fn save_agents(&self) -> Result<()> {
        let agents: Vec<_> = self.agents.iter().map(|r| r.value().clone()).collect();
        let json = serde_json::to_string_pretty(&agents)?;
        std::fs::write(self.data_dir.join("agents.json"), json)?;
        Ok(())
    }
}

fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

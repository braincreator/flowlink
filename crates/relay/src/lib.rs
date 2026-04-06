// FlowLink Relay — WebSocket hub for agents, HTTP API for clients
// Port of internal/relay/relay.go

pub mod pool;
pub mod auth;
pub mod handler;
pub mod eventbus;
pub mod approval;

use flowlink_core::config::RelayConfig;
use log::info;

pub struct Relay {
    config: RelayConfig,
}

impl Relay {
    pub fn new(config: RelayConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let pool = pool::AgentPool::new();
        let auth = auth::AuthManager::new();
        let eventbus = eventbus::EventBus::new();
        let approval = approval::ApprovalQueue::new();

        // TODO: start HTTP server + WS upgrade handler
        info!("Relay starting on {}", self.config.http_addr);
        Ok(())
    }
}

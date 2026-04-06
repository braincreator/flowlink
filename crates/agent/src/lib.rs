// FlowLink Agent — connects to relay, executes commands, manages lifecycle
// Port of internal/agent/*.go

pub mod executor;
pub mod policy;
pub mod connection;
pub mod approval;

use flowlink_core::config::AgentConfig;

pub struct Agent {
    config: AgentConfig,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut conn = connection::Connection::new(
            self.config.relay_url.clone(),
            self.config.agent_id.clone(),
            self.config.token.clone(),
        );
        conn.run().await
    }
}

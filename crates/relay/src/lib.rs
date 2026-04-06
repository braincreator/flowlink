pub mod pool;
pub mod auth;
pub mod handler;
pub mod eventbus;
pub mod approval;
pub mod ratelimit;
pub mod audit;

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
        info!("relay listening on {}", self.config.http_addr);
        // TODO: hyper HTTP server + tungstenite WS upgrade
        // TODO: route messages between agents and clients
        // TODO: mount SSE endpoint for event streaming
        tokio::signal::ctrl_c().await?;
        info!("relay shutting down");
        Ok(())
    }
}

pub mod pool;
pub mod auth;
pub mod handler;
pub mod eventbus;
pub mod approval;
pub mod ratelimit;
pub mod audit;
pub mod registry;
pub mod llm;
pub mod server;

use std::sync::Arc;
use log::info;
use flowlink_core::config::RelayConfig;

use crate::approval::ApprovalQueue;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;
use crate::handler::RelayHandler;
use crate::pool::AgentPool;
use crate::registry::Registry;
use crate::server::AppState;

pub struct Relay {
    config: RelayConfig,
}

impl Relay {
    pub fn new(config: RelayConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let pool = Arc::new(AgentPool::new());
        let auth = Arc::new(AuthManager::new());
        let eventbus = Arc::new(EventBus::new());
        let approvals = Arc::new(ApprovalQueue::new());

        let data_dir = shellexpand::tilde(&self.config.registry.data_path).to_string();
        let registry = Arc::new(Registry::new(&data_dir)?);

        let handler = Arc::new(RelayHandler::new(
            pool.clone(), auth.clone(), eventbus.clone(), approvals.clone(),
        ));

        let state = AppState {
            pool, approvals, eventbus, handler, registry,
        };

        let app = server::build_router(state);

        let addr = self.config.http_addr;
        info!("relay listening on {addr}");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        info!("relay shut down");
        Ok(())
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl-c");
    info!("shutdown signal received");
}

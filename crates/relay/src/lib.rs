pub mod pool;
pub mod auth;
pub mod handler;
pub mod eventbus;
pub mod approval;
pub mod ratelimit;
pub mod audit;
pub mod registry;
pub mod llm;
pub mod middleware;
pub mod tls;
pub mod server;
pub mod mcp;
pub mod devices;
pub mod rbac_manager;
pub mod metrics;
pub mod billing_api;

use std::sync::Arc;
use log::info;
use flowlink_core::config::RelayConfig;

use crate::approval::ApprovalQueue;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;
use crate::handler::RelayHandler;
use crate::pool::AgentPool;
use crate::registry::Registry;
use crate::devices::DeviceManager;
use crate::llm::LlmProxy;
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

        let llm_proxy = if self.config.llm.enabled {
            Some(Arc::new(LlmProxy::new(
                self.config.llm.backends.clone(),
                self.config.llm.timeout_sec,
            )))
        } else {
            None
        };

        // Database (optional — Supabase PostgreSQL)
        let db = if let Some(url) = &self.config.database_url {
            match flowlink_db::DbPool::open(url).await {
                Ok(pool) => {
                    pool.run_migrations().await?;
                    log::info!("Database connected (PostgreSQL/Supabase)");
                    Some(Arc::new(pool))
                }
                Err(e) => {
                    log::warn!("Database connection failed: {e}. Running without DB.");
                    None
                }
            }
        } else {
            None
        };

        let state = AppState {
            pool, approvals, eventbus, handler, registry,
            device_manager: Arc::new(DeviceManager::new(devices::PushConfig::default())),
            llm_proxy,
            shield_alerts: Arc::new(server::ShieldAlertManager::new()),
            audit_store: Arc::new(audit::AuditStore::new(
                std::path::Path::new(&shellexpand::tilde("~/.flowlink/audit.jsonl").to_string())
            )),
            metrics: Arc::new(metrics::Metrics::new()),
            billing: if self.config.billing.enabled {
                Some(Arc::new(flowlink_billing::BillingEngine::new(
                    flowlink_billing::payment::PaymentConfig::default(),
                )))
            } else {
                None
            },
            db,
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

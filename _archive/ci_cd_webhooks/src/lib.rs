pub mod models;
pub mod handlers;
pub mod router;
pub mod storage;
pub mod error;

pub use models::*;
pub use handlers::*;
pub use router::*;
pub use storage::*;
pub use error::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct CIConfig {
    pub webhook_port: u16,
    pub github_secret: Option<String>,
    pub gitlab_secret: Option<String>,
    pub github_token: Option<String>,
    pub gitlab_token: Option<String>,
    pub flowlink_endpoint: String,
    pub auto_approve: bool,
}

pub struct CIWebhookReceiver {
    pub config: CIConfig,
    pub handlers: Arc<RwLock<HashMap<String, Arc<dyn CIHandler + Send + Sync>>>>,
    pub storage: Arc<CIStorage>,
}

impl CIWebhookReceiver {
    pub fn new(config: CIConfig) -> Self {
        Self {
            config,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(CIStorage::new()),
        }
    }

    pub async fn start(&self) -> Result<()> {
        log::info!("Starting CI Webhook Receiver on port {}", self.config.webhook_port);
        // TODO: Start HTTP server
        Ok(())
    }

    pub async fn register_handler(&self, handler: CIHandler) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(handler.name().to_string(), Arc::new(handler));
        log::info!("Registered CI handler: {}", handler.name());
        Ok(())
    }

    pub async fn process_webhook(&self, provider: &str, payload: &str) -> Result<CIResponse> {
        let handlers = self.handlers.read().await;
        
        if let Some(handler) = handlers.get(provider) {
            handler.handle(payload).await
        } else {
            Err(anyhow::anyhow!("No handler registered for provider: {}", provider))
        }
    }
}
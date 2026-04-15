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

pub struct CRMConfig {
    pub amocrm_enabled: bool,
    pub bitrix24_enabled: bool,
    pub amocrm_client_id: Option<String>,
    pub amocrm_client_secret: Option<String>,
    pub amocrm_redirect_uri: Option<String>,
    pub bitrix24_client_id: Option<String>,
    pub bitrix24_client_secret: Option<String>,
    pub webhook_port: u16,
}

pub struct CRMIntegration {
    pub config: CRMConfig,
    pub handlers: Arc<RwLock<HashMap<String, Arc<dyn CRMHandler + Send + Sync>>>>,
    pub storage: Arc<CRMStorage>,
}

impl CRMIntegration {
    pub fn new(config: CRMConfig) -> Self {
        Self {
            config,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(CRMStorage::new()),
        }
    }

    pub async fn start(&self) -> Result<()> {
        log::info!("Starting CRM Integration system");
        
        // Register handlers
        self.register_handlers().await?;
        
        // Start webhook server
        self.start_webhook_server().await?;
        
        log::info!("CRM Integration system started successfully");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        log::info!("Stopping CRM Integration system");
        Ok(())
    }

    async fn register_handlers(&self) -> Result<()> {
        let mut handlers = self.handlers.write().await;

        if self.config.amocrm_enabled {
            let amocrm_handler = AmoCRMHandler::new(
                self.config.amocrm_client_id.clone(),
                self.config.amocrm_client_secret.clone(),
            );
            handlers.insert("amocrm".to_string(), Arc::new(amocrm_handler));
            log::info!("Registered AmoCRM handler");
        }

        if self.config.bitrix24_enabled {
            let bitrix_handler = Bitrix24Handler::new(
                self.config.bitrix24_client_id.clone(),
                self.config.bitrix24_client_secret.clone(),
            );
            handlers.insert("bitrix24".to_string(), Arc::new(bitrix_handler));
            log::info!("Registered Bitrix24 handler");
        }

        Ok(())
    }

    async fn start_webhook_server(&self) -> Result<()> {
        log::info!("Starting webhook server on port {}", self.config.webhook_port);
        // TODO: Start HTTP server for CRM webhooks
        Ok(())
    }
}
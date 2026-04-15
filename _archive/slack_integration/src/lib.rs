pub mod bot;
pub mod webhook;
pub mod models;
pub mod handlers;

pub use bot::{SlackBot, SlackBotConfig};
pub use webhook::{SlackWebhook, SlackWebhookConfig};
pub use models::*;
pub use handlers::SlackCommandHandler;

// Main orchestrator that ties everything together
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SlackIntegration {
    pub bot: Arc<SlackBot>,
    pub webhook: Arc<SlackWebhook>,
    pub command_handler: Arc<SlackCommandHandler>,
    pub flowlink_client: Arc<flowlink_relay::FlowLinkClient>,
}

impl SlackIntegration {
    pub async fn new(config: SlackConfig, flowlink_client: Arc<flowlink_relay::FlowLinkClient>) -> Result<Self> {
        let bot = Arc::new(SlackBot::new(config.bot.clone()).await?);
        let webhook = Arc::new(SlackWebhook::new(config.webhook.clone()).await?);
        let command_handler = Arc::new(SlackCommandHandler::new(config.clone(), flowlink_client.clone()));
        
        Ok(Self {
            bot,
            webhook,
            command_handler,
            flowlink_client,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        // Start webhook server
        let webhook_server = self.webhook.start();
        
        // Start bot event processing
        let bot_events = self.bot.start_event_processing();
        
        // Run concurrently
        tokio::select! {
            result = webhook_server => {
                log::error!("Webhook server failed: {:?}", result);
                result
            }
            result = bot_events => {
                log::error!("Bot event processing failed: {:?}", result);
                result
            }
        }
    }
    
    pub async fn stop(&self) -> Result<()> {
        self.webhook.stop().await;
        self.bot.stop().await;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SlackConfig {
    pub bot: SlackBotConfig,
    pub webhook: SlackWebhookConfig,
    pub flowlink_endpoint: String,
    pub app_id: String,
    pub sign_secret: String,
    pub allowed_channels: Vec<String>,
    pub approval_channel: String,
    pub admin_users: Vec<String>,
}
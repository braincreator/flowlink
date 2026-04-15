//! Discord Bot for FlowLink
//! 
//! Full Discord integration with agent management, notifications, and approval workflows

pub mod bot;
pub mod handlers;
pub mod models;
pub mod webhook;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::bot::DiscordBot;
use crate::webhook::DiscordWebhookHandler;

// Discord Bot Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub guild_id: String,
    pub channel_id: String,
    pub webhook_url: Option<String>,
    pub allowed_roles: Vec<String>,
    pub enable_approvals: bool,
    pub enable_notifications: bool,
    pub webhook_secret: Option<String>,
}

// Discord Bot Context
#[derive(Clone)]
pub struct DiscordContext {
    pub bot: Arc<DiscordBot>,
    pub webhook_handler: Arc<DiscordWebhookHandler>,
    pub config: DiscordConfig,
    pub guild_name: String,
    pub channel_name: String,
}

impl DiscordContext {
    pub async fn new(config: DiscordConfig) -> Result<Self> {
        let bot = Arc::new(DiscordBot::new(&config).await?);
        let webhook_handler = Arc::new(DiscordWebhookHandler::new(config.webhook_url.clone())?);
        
        let guild_name = bot.get_guild_name(&config.guild_id).await.unwrap_or_default();
        let channel_name = bot.get_channel_name(&config.channel_id).await.unwrap_or_default();
        
        Ok(Self {
            bot,
            webhook_handler,
            config,
            guild_name,
            channel_name,
        })
    }
}

// Message types for Discord integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscordMessage {
    Text { content: String },
    Embed { title: String, description: String, color: u32 },
    AgentCommand { agent_id: String, command: String, approval_needed: bool },
    Notification { type_: String, message: String, details: Option<String> },
}

impl DiscordMessage {
    pub fn text(content: String) -> Self {
        Self::Text { content }
    }
    
    pub fn embed(title: String, description: String, color: u32) -> Self {
        Self::Embed { title, description, color }
    }
    
    pub fn agent_command(agent_id: String, command: String, approval_needed: bool) -> Self {
        Self::AgentCommand { agent_id, command, approval_needed }
    }
    
    pub fn notification(type_: String, message: String, details: Option<String>) -> Self {
        Self::Notification { type_, message, details }
    }
}

// Error types
#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
    #[error("Bot token is missing")]
    MissingBotToken,
    #[error("Guild ID is missing")]
    MissingGuildId,
    #[error("Channel ID is missing")]
    MissingChannelId,
    #[error("Failed to create webhook: {0}")]
    WebhookCreation(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),
}

// Re-export types for external use
pub use bot::DiscordBot;
pub use handlers::{DiscordCommand, DiscordCommandHandler};
pub use models::{DiscordUser, DiscordChannel, DiscordGuild, DiscordReaction};
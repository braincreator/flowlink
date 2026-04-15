use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{DiscordConfig, DiscordError, DiscordMessage};

// Discord Bot implementation
pub struct DiscordBot {
    config: DiscordConfig,
    http_client: reqwest::Client,
    commands: HashMap<String, DiscordCommand>,
    active_sessions: RwLock<HashMap<String, DiscordSession>>,
}

// Discord session state
#[derive(Debug, Clone)]
pub struct DiscordSession {
    pub user_id: String,
    pub agent_id: String,
    pub command_buffer: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

// Discord API response types
#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordChannel {
    pub id: String,
    pub name: String,
    pub kind: ChannelType,
}

#[derive(Debug, Deserialize)]
pub struct DiscordGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordMessageResponse {
    pub id: String,
    pub channel_id: String,
    pub author: DiscordUser,
    pub content: String,
    pub timestamp: String,
    pub embeds: Vec<DiscordEmbed>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub color: Option<u32>,
    pub fields: Vec<DiscordEmbedField>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Text,
    Voice,
    Private,
}

impl DiscordBot {
    pub async fn new(config: &DiscordConfig) -> Result<Self> {
        if config.bot_token.is_empty() {
            return Err(DiscordError::MissingBotToken.into());
        }
        
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        // Test bot token by making API call
        let bot_info = Self::get_bot_info(&http_client, &config.bot_token).await?;
        log::info!("Discord bot initialized: {}#{}", bot_info.username, bot_info.discriminator);
        
        Ok(Self {
            config: config.clone(),
            http_client,
            commands: Self::register_commands(),
            active_sessions: RwLock::new(HashMap::new()),
        })
    }
    
    async fn get_bot_info(http_client: &reqwest::Client, bot_token: &str) -> Result<DiscordUser> {
        let url = "https://discord.com/api/v9/users/@me";
        let response = http_client
            .get(url)
            .header("Authorization", format!("Bot {}", bot_token))
            .send()
            .await?;
            
        if response.status().is_success() {
            response.json().await.map_err(|e| anyhow!("Failed to parse bot info: {}", e))
        } else {
            Err(anyhow!("Failed to get bot info: {}", response.status()))
        }
    }
    
    fn register_commands() -> HashMap<String, DiscordCommand> {
        let mut commands = HashMap::new();
        
        commands.insert("agents".to_string(), DiscordCommand {
            name: "agents".to_string(),
            description: "List connected agents".to_string(),
            usage: "agents",
            handler: Box::new(|ctx, args| Box::pin(async move {
                Self::handle_list_agents(ctx, args).await
            })),
        });
        
        commands.insert("exec".to_string(), DiscordCommand {
            name: "exec".to_string(),
            description: "Execute command on agent".to_string(),
            usage: "exec <agent_id> <command>",
            handler: Box::new(|ctx, args| Box::pin(async move {
                Self::handle_exec_command(ctx, args).await
            })),
        });
        
        commands.insert("approve".to_string(), DiscordCommand {
            name: "approve".to_string(),
            description: "Approve pending command".to_string(),
            usage: "approve <request_id>",
            handler: Box::new(|ctx, args| Box::pin(async move {
                Self::handle_approve(ctx, args).await
            })),
        });
        
        commands.insert("reject".to_string(), DiscordCommand {
            name: "reject".to_string(),
            description: "Reject pending command".to_string(),
            usage: "reject <request_id>",
            handler: Box::new(|ctx, args| Box::pin(async move {
                Self::handle_reject(ctx, args).await
            })),
        });
        
        commands.insert("status".to_string(), DiscordCommand {
            name: "status".to_string(),
            description: "System status".to_string(),
            usage: "status",
            handler: Box::new(|ctx, args| Box::pin(async move {
                Self::handle_status(ctx, args).await
            })),
        });
        
        commands
    }
    
    // Command handlers
    async fn handle_list_agents(_ctx: Arc<DiscordContext>, args: Vec<String>) -> DiscordMessage {
        // TODO: Integrate with FlowLink agent pool
        DiscordMessage::text("🤖 **Connected Agents:**\n- agent-1 (Linux)\n- agent-2 (Windows)".to_string())
    }
    
    async fn handle_exec_command(ctx: Arc<DiscordContext>, args: Vec<String>) -> DiscordMessage {
        if args.len() < 2 {
            return DiscordMessage::text("❌ Usage: exec <agent_id> <command>".to_string());
        }
        
        let agent_id = &args[0];
        let command = args[1..].join(" ");
        
        // Check if approval is needed
        let approval_needed = ctx.config.enable_approvals && Self::is_dangerous_command(&command);
        
        if approval_needed {
            DiscordMessage::agent_command(agent_id.clone(), command, true)
        } else {
            // TODO: Execute command via FlowLink relay
            DiscordMessage::text(format!("🚀 Executing '{}' on {}", command, agent_id))
        }
    }
    
    async fn handle_approve(ctx: Arc<DiscordContext>, args: Vec<String>) -> DiscordMessage {
        if args.is_empty() {
            return DiscordMessage::text("❌ Usage: approve <request_id>".to_string());
        }
        
        let request_id = &args[0];
        
        // TODO: Send approval to FlowLink approval system
        DiscordMessage::text(format!("✅ Approved request {}", request_id))
    }
    
    async fn handle_reject(ctx: Arc<DiscordContext>, args: Vec<String>) -> DiscordMessage {
        if args.is_empty() {
            return DiscordMessage::text("❌ Usage: reject <request_id>".to_string());
        }
        
        let request_id = &args[0];
        let reason = args.get(1).cloned().unwrap_or_else(|| "No reason provided".to_string());
        
        // TODO: Send rejection to FlowLink approval system
        DiscordMessage::text(format!("❌ Rejected request {}: {}", request_id, reason))
    }
    
    async fn handle_status(_ctx: Arc<DiscordContext>, _args: Vec<String>) -> DiscordMessage {
        DiscordMessage::embed(
            "FlowLink Status".to_string(),
            "🟢 All systems operational".to_string(),
            0x00ff00,
        )
    }
    
    fn is_dangerous_command(command: &str) -> bool {
        let dangerous_patterns = [
            "rm -rf /",
            "sudo rm",
            "mkfs",
            "dd if=",
            ":(){ :|:& };:",
            "chmod 777",
            "chown root",
            "passwd",
        ];
        
        dangerous_patterns.iter().any(|pattern| command.contains(pattern))
    }
    
    // Public API methods
    pub async fn get_guild_name(&self, guild_id: &str) -> Result<String> {
        let url = format!("https://discord.com/api/v9/guilds/{}", guild_id);
        let response = self.http_client
            .get(&url)
            .header("Authorization", format!("Bot {}", self.config.bot_token))
            .send()
            .await?;
            
        if response.status().is_success() {
            let guild: DiscordGuild = response.json().await?;
            Ok(guild.name)
        } else {
            Err(anyhow!("Failed to get guild name: {}", response.status()))
        }
    }
    
    pub async fn get_channel_name(&self, channel_id: &str) -> Result<String> {
        let url = format!("https://discord.com/api/v9/channels/{}", channel_id);
        let response = self.http_client
            .get(&url)
            .header("Authorization", format!("Bot {}", self.config.bot_token))
            .send()
            .await?;
            
        if response.status().is_success() {
            let channel: DiscordChannel = response.json().await?;
            Ok(channel.name)
        } else {
            Err(anyhow!("Failed to get channel name: {}", response.status()))
        }
    }
    
    pub async fn send_message(&self, message: DiscordMessage) -> Result<String> {
        let url = format!("https://discord.com/api/v9/channels/{}/messages", self.config.channel_id);
        
        let payload = match message {
            DiscordMessage::Text { content } => serde_json::json!({
                "content": content
            }),
            DiscordMessage::Embed { title, description, color } => serde_json::json!({
                "embeds": [{
                    "title": title,
                    "description": description,
                    "color": color
                }]
            }),
            DiscordMessage::AgentCommand { agent_id, command, approval_needed } => serde_json::json!({
                "embeds": [{
                    "title": "🔧 Agent Command",
                    "description": format!("**Agent:** {}\n**Command:** {}\n**Approval Needed:** {}", agent_id, command, approval_needed),
                    "color": 0x0088ff
                }]
            }),
            DiscordMessage::Notification { type_, message, details } => serde_json::json!({
                "embeds": [{
                    "title": format!("📢 {}", type_),
                    "description": message,
                    "color": 0xff8800,
                    "fields": details.map(|d| vec![
                        {
                            "name": "Details".to_string(),
                            "value": d,
                            "inline": false
                        }
                    ]).unwrap_or_default()
                }]
            }),
        };
        
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;
            
        if response.status().is_success() {
            let resp: DiscordMessageResponse = response.json().await?;
            Ok(resp.id)
        } else {
            Err(anyhow!("Failed to send message: {}", response.status()))
        }
    }
    
    pub async fn create_webhook(&self) -> Result<String> {
        let url = format!("https://discord.com/api/v9/channels/{}/webhooks", self.config.channel_id);
        let payload = serde_json::json!({
            "name": "FlowLink",
            "avatar": "https://flowlink.app/logo.png"
        });
        
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.config.bot_token))
            .json(&payload)
            .send()
            .await?;
            
        if response.status().is_success() {
            let webhook: DiscordWebhookResponse = response.json().await?;
            Ok(webhook.url)
        } else {
            Err(DiscordError::WebhookCreation(response.status().to_string()).into())
        }
    }
    
    pub async fn list_commands(&self) -> Vec<DiscordCommand> {
        self.commands.values().cloned().collect()
    }
}

// Webhook response type
#[derive(Debug, Deserialize)]
pub struct DiscordWebhookResponse {
    pub id: String,
    pub url: String,
    pub token: String,
}

// Discord command type
#[derive(Debug, Clone)]
pub struct DiscordCommand {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub handler: Box<dyn for<'a> Fn(Arc<DiscordContext>, Vec<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = DiscordMessage> + Send + 'a>> + Send + Sync>,
}
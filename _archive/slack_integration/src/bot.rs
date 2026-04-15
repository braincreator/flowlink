use anyhow::Result;
use slack_morphism::prelude::*;
use slack_morphism::SlackClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use super::*;

pub struct SlackBot {
    pub config: SlackBotConfig,
    pub client: Arc<SlackClient>,
    pub is_running: Arc<RwLock<bool>>,
    pub flowlink_client: Arc<flowlink_relay::FlowLinkClient>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SlackBotConfig {
    pub bot_token: String,
    pub app_token: String,
    pub signing_secret: String,
    pub allowed_channels: Vec<String>,
    pub approval_channel: String,
}

impl SlackBot {
    pub async fn new(config: SlackBotConfig) -> Result<Self> {
        let client = Arc::new(
            SlackClient::new_from_env().unwrap_or_else(|_| {
                log::warn!("Could not create client from env, using defaults");
                SlackClient::new_with_token(config.bot_token.clone()).unwrap()
            })
        );
        
        Ok(Self {
            config,
            client,
            is_running: Arc::new(RwLock::new(false)),
            flowlink_client: Arc::new(flowlink_relay::FlowLinkClient::new()), // TODO: pass from outside
        })
    }
    
    pub async fn start_event_processing(&self) -> Result<()> {
        *self.is_running.write().await = true;
        
        let client = self.client.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let bot_token = config.bot_token.clone();
            let app_token = config.app_token.clone();
            
            log::info!("Starting Slack bot event processing");
            
            loop {
                // TODO: Implement proper event processing
                // For now, just keep the connection alive
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                
                if !*SlackBot::get_is_running(&client).await {
                    break;
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;
        log::info!("Slack bot stopped");
        Ok(())
    }
    
    pub async fn send_message(&self, channel: &str, message: &str) -> Result<SlackMessageSendResponse> {
        let client = self.client.clone();
        
        let api_request = SlackMessageSendRequest::new(channel.to_string(), message.to_string());
        
        client.send_message(api_request).await
    }
    
    pub async fn send_embed(&self, channel: &str, embed: SlackEmbed) -> Result<SlackMessageSendResponse> {
        let client = self.client.clone();
        
        let mut api_request = SlackMessageSendRequest::new(channel.to_string(), "");
        api_request.add_embed(embed);
        
        client.send_message(api_request).await
    }
    
    pub async fn send_approval_request(&self, command_request: ApprovalRequest) -> Result<()> {
        let embed = SlackEmbed::new()
            .set_title("🔒 Command Approval Required")
            .set_color(SlackColor::Danger)
            .add_field("Agent", &command_request.agent_id, true)
            .add_field("Command", &command_request.command, true)
            .add_field("User", &command_request.user_id, true)
            .add_field("Request ID", &command_request.request_id, true)
            .set_footer(SlackFooter::new("FlowLink").with_icon_url("https://flowlink.app/icon.png"));
        
        self.send_message(&self.config.approval_channel, "Approval Required").await?;
        self.send_embed(&self.config.approval_channel, embed).await?;
        
        Ok(())
    }
    
    pub async fn send_notification(&self, notification: SlackNotification) -> Result<()> {
        let embed = SlackEmbed::new()
            .set_title(notification.title)
            .set_color(notification.color)
            .set_description(notification.message)
            .set_footer(SlackFooter::new("FlowLink").with_icon_url("https://flowlink.app/icon.png"));
        
        self.send_message(&notification.channel, &notification.message).await?;
        self.send_embed(&notification.channel, embed).await?;
        
        Ok(())
    }
    
    pub async fn send_command_result(&self, result: CommandResult) -> Result<()> {
        let embed = SlackEmbed::new()
            .set_title(match result.success {
                true => "✅ Command Executed",
                false => "❌ Command Failed",
            })
            .set_color(if result.success { SlackColor::Good } else { SlackColor::Danger })
            .add_field("Agent", &result.agent_id, true)
            .add_field("Command", &result.command, true)
            .add_field("Exit Code", &result.exit_code.to_string(), true)
            .set_description(&result.output)
            .set_footer(SlackFooter::new("FlowLink").with_icon_url("https://flowlink.app/icon.png"));
        
        self.send_message(&result.channel, "Command Result").await?;
        self.send_embed(&result.channel, embed).await?;
        
        Ok(())
    }
    
    pub async fn send_help_message(&self, channel: &str) -> Result<()> {
        let help_text = r#"
🤖 **FlowLink Slack Bot Help**

**Agent Management:**
• `agents` - List connected agents
• `exec <agent_id> <command>` - Execute command on agent
• `status` - System status overview

**Approval System:**
• `approve <request_id>` - Approve pending command
• `reject <request_id> <reason>` - Reject pending command

**Utilities:**
• `stats` - System statistics
• `backup <agent_id>` - Create backup
• `logs <agent_id> <lines>` - View logs
• `config show` - Show configuration
• `help` - This help message

**Examples:**
• `exec server-1 uptime`
• `approve req_123456`
• `reject req_123456 Too risky`

React with ✅ to approve or ❌ to reject approval requests.
"#;
        
        self.send_message(channel, help_text).await?;
        Ok(())
    }
    
    pub async fn is_allowed_channel(&self, channel_id: &str) -> bool {
        self.config.allowed_channels.contains(&channel_id.to_string())
    }
    
    pub async fn is_admin_user(&self, user_id: &str) -> bool {
        self.config.admin_users.contains(&user_id.to_string())
    }
    
    async fn get_is_running(client: &Arc<SlackClient>) -> bool {
        // TODO: Implement proper connection check
        true
    }
}
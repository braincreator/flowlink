use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;

use super::*;

pub struct SlackCommandHandler {
    pub config: SlackConfig,
    pub flowlink_client: Arc<flowlink_relay::FlowLinkClient>,
    pub pending_approvals: Arc<tokio::sync::RwLock<HashMap<String, ApprovalRequest>>>,
}

impl SlackCommandHandler {
    pub fn new(config: SlackConfig, flowlink_client: Arc<flowlink_relay::FlowLinkClient>) -> Self {
        Self {
            config,
            flowlink_client,
            pending_approvals: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn handle_command(
        &self,
        channel: String,
        user: String,
        team: String,
        command: &str,
        args: Vec<String>,
    ) -> Result<()> {
        match command {
            "help" => self.handle_help(channel.clone()).await?,
            "agents" => self.handle_agents(channel.clone()).await?,
            "exec" => self.handle_exec(channel.clone(), user.clone(), team.clone(), args).await?,
            "status" => self.handle_status(channel.clone()).await?,
            "config" => self.handle_config(channel.clone(), args).await?,
            "stats" => self.handle_stats(channel.clone()).await?,
            "backup" => self.handle_backup(channel.clone(), args).await?,
            "logs" => self.handle_logs(channel.clone(), args).await?,
            "approve" => self.handle_approve(channel.clone(), user.clone(), args).await?,
            "reject" => self.handle_reject(channel.clone(), user.clone(), args).await?,
            _ => self.handle_unknown_command(channel.clone(), command).await?,
        }
        
        Ok(())
    }
    
    async fn handle_help(&self, channel: String) -> Result<()> {
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
        
        // TODO: Send via Slack bot
        log::info!("Sending help to channel {}: {}", channel, help_text);
        Ok(())
    }
    
    async fn handle_agents(&self, channel: String) -> Result<()> {
        // TODO: Get agents from FlowLink
        let agents = vec![
            ("server-1", "Linux", "Online", "production"),
            ("server-2", "Windows", "Online", "staging"),
            ("server-3", "Linux", "Offline", "backup"),
        ];
        
        let message = "🤖 **Connected Agents:**\n\n".to_string();
        let mut agent_list = String::new();
        
        for (id, os, status, env) in agents {
            let status_icon = match status {
                "Online" => "🟢",
                "Offline" => "🔴",
                _ => "🟡",
            };
            agent_list.push_str(&format!("{} **{}** - *{}* | Environment: {}\n", status_icon, id, os, env));
        }
        
        let full_message = format!("{}\n{}", message, agent_list);
        
        // TODO: Send via Slack bot
        log::info!("Sending agents list to channel {}: {}", channel, full_message);
        Ok(())
    }
    
    async fn handle_exec(&self, channel: String, user: String, team: String, mut args: Vec<String>) -> Result<()> {
        if args.len() < 2 {
            self.send_error(channel, "Usage: exec <agent_id> <command>").await?;
            return Ok(());
        }
        
        let agent_id = args.remove(0);
        let command = args.join(" ");
        
        // Check if command is dangerous
        if self.is_dangerous_command(&command) {
            // Create approval request
            let request_id = uuid::Uuid::new_v4().to_string();
            let approval_request = ApprovalRequest {
                request_id: request_id.clone(),
                agent_id: agent_id.clone(),
                command: command.clone(),
                user_id: user.clone(),
                team_id: team.clone(),
                timestamp: chrono::Utc::now(),
            };
            
            // Store in pending approvals
            self.pending_approvals.write().await.insert(request_id.clone(), approval_request);
            
            // Send approval request
            self.send_approval_request(channel.clone(), approval_request).await?;
            
            log::info!("Command requires approval: {} on {}", command, agent_id);
        } else {
            // Execute command directly
            self.execute_command(channel.clone(), user.clone(), agent_id, command).await?;
        }
        
        Ok(())
    }
    
    async fn handle_status(&self, channel: String) -> Result<()> {
        // TODO: Get actual status from FlowLink
        let status = serde_json::json!({
            "agents_online": 2,
            "agents_total": 3,
            "pending_approvals": self.pending_approvals.read().await.len(),
            "commands_today": 45,
            "uptime": "99.9%"
        });
        
        let message = format!("📊 **System Status:**\n\n**Agents:** {}/online\n**Approvals:** {} pending\n**Commands Today:** {}\n**Uptime:** {}",
            status["agents_online"], status["pending_approvals"], status["commands_today"], status["uptime"]);
        
        // TODO: Send via Slack bot
        log::info!("Sending status to channel {}: {}", channel, message);
        Ok(())
    }
    
    async fn handle_config(&self, channel: String, args: Vec<String>) -> Result<()> {
        if args.is_empty() || args[0] != "show" {
            self.send_error(channel, "Usage: config show").await?;
            return Ok(());
        }
        
        let config_info = serde_json::json!({
            "guild": self.config.app_id,
            "approval_channel": self.config.bot.approval_channel,
            "approvals_enabled": true,
            "notifications_enabled": true,
            "allowed_channels": self.config.bot.allowed_channels
        });
        
        let message = format!("🔧 **Current Configuration:**\n```json\n{}```", config_info.to_string());
        
        // TODO: Send via Slack bot
        log::info!("Sending config to channel {}: {}", channel, message);
        Ok(())
    }
    
    async fn handle_stats(&self, channel: String) -> Result<()> {
        // TODO: Get actual stats from FlowLink
        let stats = serde_json::json!({
            "total_commands": 1234,
            "successful": 1200,
            "failed": 34,
            "approvals_given": 45,
            "approvals_rejected": 12,
            "uptime_days": 15
        });
        
        let success_rate = (stats["successful"].as_u64().unwrap_or(0) as f64 / stats["total_commands"].as_u64().unwrap_or(1) as f64) * 100.0;
        
        let message = format!("📈 **System Statistics:**\n\n**Total Commands:** {}\n**Success Rate:** {:.1}%\n**Approvals Given:** {}\n**Days Running:** {}",
            stats["total_commands"], success_rate, stats["approvals_given"], stats["uptime_days"]);
        
        // TODO: Send via Slack bot
        log::info!("Sending stats to channel {}: {}", channel, message);
        Ok(())
    }
    
    async fn handle_backup(&self, channel: String, mut args: Vec<String>) -> Result<()> {
        if args.is_empty() {
            self.send_error(channel, "Usage: backup <agent_id>").await?;
            return Ok(());
        }
        
        let agent_id = &args[0];
        
        // TODO: Trigger backup via FlowLink API
        let message = format!("🗃️ Creating backup for {}...", agent_id);
        
        // TODO: Send via Slack bot
        log::info!("Sending backup notification to channel {}: {}", channel, message);
        Ok(())
    }
    
    async fn handle_logs(&self, channel: String, mut args: Vec<String>) -> Result<()> {
        if args.len() < 2 {
            self.send_error(channel, "Usage: logs <agent_id> <lines>").await?;
            return Ok(());
        }
        
        let agent_id = &args[0];
        let lines = args[1].parse().unwrap_or(100);
        
        // TODO: Fetch logs from FlowLink
        let message = format!("📄 Fetching last {} logs from {}...", lines, agent_id);
        
        // TODO: Send via Slack bot
        log::info!("Sending log request to channel {}: {}", channel, message);
        Ok(())
    }
    
    async fn handle_approve(&self, channel: String, user: String, mut args: Vec<String>) -> Result<()> {
        if args.is_empty() {
            self.send_error(channel, "Usage: approve <request_id>").await?;
            return Ok(());
        }
        
        let request_id = args.remove(0);
        let reason = args.join(" ");
        
        self.handle_approval_action(&request_id, true, channel, user).await?;
        Ok(())
    }
    
    async fn handle_reject(&self, channel: String, user: String, mut args: Vec<String>) -> Result<()> {
        if args.is_empty() {
            self.send_error(channel, "Usage: reject <request_id> <reason>").await?;
            return Ok(());
        }
        
        let request_id = args.remove(0);
        let reason = if args.is_empty() {
            "No reason provided".to_string()
        } else {
            args.join(" ")
        };
        
        self.handle_approval_action(&request_id, false, channel, user).await?;
        Ok(())
    }
    
    async fn handle_unknown_command(&self, channel: String, command: &str) -> Result<()> {
        let message = format!("❌ Unknown command: `{}`. Use `help` for available commands.", command);
        
        // TODO: Send via Slack bot
        log::info!("Sending error to channel {}: {}", channel, message);
        Ok(())
    }
    
    // Helper methods
    fn is_dangerous_command(&self, command: &str) -> bool {
        let dangerous = [
            "rm -rf", "sudo rm", "mkfs", "dd if=", ":(){ :|:& };:",
            "chmod 777", "chown root", "passwd", "fdisk", "mkpart"
        ];
        
        dangerous.iter().any(|&cmd| command.contains(cmd))
    }
    
    async fn execute_command(&self, channel: String, user: String, agent_id: String, command: String) -> Result<()> {
        // TODO: Send to FlowLink relay to execute command
        // For now, simulate execution
        let result = format!("Command '{}' executed on {}", command, agent_id);
        
        let command_result = CommandResult {
            agent_id,
            command,
            output: result,
            success: true,
            exit_code: 0,
            channel,
            user,
        };
        
        self.send_command_result(command_result).await
    }
    
    async fn send_approval_request(&self, channel: String, request: ApprovalRequest) -> Result<()> {
        // TODO: Send via Slack bot with interactive elements
        log::info!("Sending approval request for request {}: {}", request.request_id, request.command);
        Ok(())
    }
    
    async fn send_command_result(&self, result: CommandResult) -> Result<()> {
        // TODO: Send via Slack bot
        log::info!("Command result for {}: success={}, exit_code={}", 
            result.agent_id, result.success, result.exit_code);
        Ok(())
    }
    
    async fn send_error(&self, channel: String, error: &str) -> Result<()> {
        let message = format!("❌ {}", error);
        log::info!("Sending error to channel {}: {}", channel, message);
        Ok(())
    }
    
    async fn handle_approval_action(&self, request_id: &str, approve: bool, channel: String, user: String) -> Result<()> {
        let mut pending = self.pending_approvals.write().await;
        
        if let Some(request) = pending.remove(request_id) {
            // TODO: Send approval to FlowLink approval system
            let action = if approve { "approved" } else { "rejected" };
            log::info!("Slack {} approval for request {}: {} by {}", action, request_id, 
                if approve { "" } else { pending.get(request_id).map(|r| r.command.clone()).unwrap_or_default() }, user);
            
            // Remove from pending
            pending.remove(request_id);
        } else {
            log::warn!("Request {} not found in pending approvals", request_id);
        }
        
        Ok(())
    }
}
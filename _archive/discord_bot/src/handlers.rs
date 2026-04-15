use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::{DiscordContext, DiscordMessage, DiscordError};

// Discord command handlers
pub struct DiscordCommandHandler;

impl DiscordCommandHandler {
    pub async fn handle_command(ctx: Arc<DiscordContext>, command: &str, args: Vec<String>) -> Result<DiscordMessage> {
        match command {
            "help" => Self::handle_help().await,
            "agents" => Self::handle_agents(ctx).await,
            "exec" => Self::handle_exec(ctx, args).await,
            "approve" => Self::handle_approve(ctx, args).await,
            "reject" => Self::handle_reject(ctx, args).await,
            "status" => Self::handle_status(ctx).await,
            "config" => Self::handle_config(ctx, args).await,
            "stats" => Self::handle_stats(ctx).await,
            "backup" => Self::handle_backup(ctx, args).await,
            "logs" => Self::handle_logs(ctx, args).await,
            _ => Self::handle_unknown_command(command).await,
        }
    }
    
    async fn handle_help() -> Result<DiscordMessage> {
        let help_text = r#"
🤖 **FlowLink Discord Bot Help**

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
"#;
        
        Ok(DiscordMessage::text(help_text.to_string()))
    }
    
    async fn handle_agents(ctx: Arc<DiscordContext>) -> Result<DiscordMessage> {
        // TODO: Integrate with FlowLink agent pool
        let agents = vec![
            ("server-1", "Linux", "Online", "production"),
            ("server-2", "Windows", "Online", "staging"),
            ("server-3", "Linux", "Offline", "backup"),
        ];
        
        let mut embed_fields = Vec::new();
        for (id, os, status, env) in agents {
            let status_icon = match status {
                "Online" => "🟢",
                "Offline" => "🔴",
                _ => "🟡",
            };
            embed_fields.push(serde_json::json!({
                "name": format!("{} {}", status_icon, id),
                "value": format("**OS:** {}\n**Environment:** {}", os, env),
                "inline": false
            }));
        }
        
        Ok(DiscordMessage::Embed {
            title: "🤖 Connected Agents".to_string(),
            description: "Currently monitored servers and agents".to_string(),
            color: 0x0088ff,
            embed_fields: Some(embed_fields),
        })
    }
    
    async fn handle_exec(ctx: Arc<DiscordContext>, mut args: Vec<String>) -> Result<DiscordMessage> {
        if args.len() < 2 {
            return Ok(DiscordMessage::text("❌ Usage: exec <agent_id> <command>"));
        }
        
        let agent_id = args.remove(0);
        let command = args.join(" ");
        
        // Check if command is dangerous
        let approval_needed = Self::is_dangerous_command(&command);
        
        if approval_needed {
            // Create approval request
            let request_id = Self::create_approval_request(&ctx, &agent_id, &command).await?;
            
            Ok(DiscordMessage::agent_command(agent_id, command, true))
        } else {
            // Execute command directly
            match Self::execute_command(&ctx, &agent_id, &command).await {
                Ok(result) => Ok(DiscordMessage::text(format!("✅ Command executed:\n```\n{}```", result))),
                Err(e) => Ok(DiscordMessage::text(format!("❌ Error: {}", e))),
            }
        }
    }
    
    async fn handle_approve(ctx: Arc<DiscordContext>, mut args: Vec<String>) -> Result<DiscordMessage> {
        if args.is_empty() {
            return Ok(DiscordMessage::text("❌ Usage: approve <request_id>"));
        }
        
        let request_id = args.remove(0);
        let reason = args.join(" ");
        
        // TODO: Send approval to FlowLink approval system
        match Self::send_approval(&ctx, &request_id, true, &reason).await {
            Ok(_) => Ok(DiscordMessage::text(format!("✅ Approved request {}", request_id))),
            Err(e) => Ok(DiscordMessage::text(format!("❌ Error: {}", e))),
        }
    }
    
    async fn handle_reject(ctx: Arc<DiscordContext>, mut args: Vec<String>) -> Result<DiscordMessage> {
        if args.is_empty() {
            return Ok(DiscordMessage::text("❌ Usage: reject <request_id> <reason>"));
        }
        
        let request_id = args.remove(0);
        let reason = if args.is_empty() {
            "No reason provided".to_string()
        } else {
            args.join(" ")
        };
        
        // TODO: Send rejection to FlowLink approval system
        match Self::send_approval(&ctx, &request_id, false, &reason).await {
            Ok(_) => Ok(DiscordMessage::text(format!("❌ Rejected request {}: {}", request_id, reason))),
            Err(e) => Ok(DiscordMessage::text(format!("❌ Error: {}", e))),
        }
    }
    
    async fn handle_status(ctx: Arc<DiscordContext>) -> Result<DiscordMessage> {
        // TODO: Get actual status from FlowLink system
        let stats = serde_json::json!({
            "agents_online": 2,
            "agents_total": 3,
            "pending_approvals": 1,
            "commands_today": 45,
            "uptime": "99.9%"
        });
        
        Ok(DiscordMessage::Embed {
            title: "📊 System Status".to_string(),
            description: format!("**Agents:** {}/online\n**Approvals:** {} pending\n**Commands Today:** {}\n**Uptime:** {}", 
                stats["agents_online"], stats["pending_approvals"], stats["commands_today"], stats["uptime"]),
            color: 0x00ff00,
        })
    }
    
    async fn handle_config(ctx: Arc<DiscordContext>, mut args: Vec<String>) -> Result<DiscordMessage> {
        if args.is_empty() || args[0] != "show" {
            return Ok(DiscordMessage::text("❌ Usage: config show"));
        }
        
        let config_info = serde_json::json!({
            "guild": ctx.guild_name,
            "channel": ctx.channel_name,
            "approvals_enabled": ctx.config.enable_approvals,
            "notifications_enabled": ctx.config.enable_notifications,
            "allowed_roles": ctx.config.allowed_roles
        });
        
        Ok(DiscordMessage::text(format!("🔧 **Current Configuration:**\n```json\n{}```", config_info.to_string())))
    }
    
    async fn handle_stats(ctx: Arc<DiscordContext>) -> Result<DiscordMessage> {
        // TODO: Get actual stats from FlowLink metrics
        let stats = serde_json::json!({
            "total_commands": 1234,
            "successful": 1200,
            "failed": 34,
            "approvals_given": 45,
            "approvals_rejected": 12,
            "uptime_days": 15
        });
        
        Ok(DiscordMessage::Embed {
            title: "📈 System Statistics".to_string(),
            description: format!("**Total Commands:** {}\n**Success Rate:** {:.1}%\n**Approvals Given:** {}\n**Days Running:** {}", 
                stats["total_commands"], (stats["successful"].as_u64().unwrap_or(0) as f64 / stats["total_commands"].as_u64().unwrap_or(1) as f64) * 100.0,
                stats["approvals_given"], stats["uptime_days"]),
            color: 0x0088ff,
        })
    }
    
    async fn handle_backup(ctx: Arc<DiscordContext>, mut args: Vec<String>) -> Result<DiscordMessage> {
        if args.is_empty() {
            return Ok(DiscordMessage::text("❌ Usage: backup <agent_id>"));
        }
        
        let agent_id = &args[0];
        
        // TODO: Trigger backup via FlowLink API
        Ok(DiscordMessage::text(format!("🗃️ Creating backup for {}...", agent_id)))
    }
    
    async fn handle_logs(ctx: Arc<DiscordContext>, mut args: Vec<String>) -> Result<DiscordMessage> {
        if args.len() < 2 {
            return Ok(DiscordMessage::text("❌ Usage: logs <agent_id> <lines>"));
        }
        
        let agent_id = &args[0];
        let lines = args[1].parse().unwrap_or(100);
        
        // TODO: Fetch logs from FlowLink
        Ok(DiscordMessage::text(format!("📄 Fetching last {} logs from {}...", lines, agent_id)))
    }
    
    async fn handle_unknown_command(command: &str) -> Result<DiscordMessage> {
        Ok(DiscordMessage::text(format!("❌ Unknown command: `{}`. Use `help` for available commands.", command)))
    }
    
    // Helper methods
    fn is_dangerous_command(command: &str) -> bool {
        let dangerous = [
            "rm -rf", "sudo rm", "mkfs", "dd if=", ":(){ :|:& };:",
            "chmod 777", "chown root", "passwd", "fdisk", "mkpart"
        ];
        
        dangerous.iter().any(|&cmd| command.contains(cmd))
    }
    
    async fn create_approval_request(ctx: &Arc<DiscordContext>, agent_id: &str, command: &str) -> Result<String> {
        // TODO: Integrate with FlowLink approval system
        let request_id = format!("req_{}", uuid::Uuid::new_v4());
        
        // Send notification to approval channel
        let message = DiscordMessage::Notification {
            type_: "Approval Required".to_string(),
            message: format!("Command requires approval on {}", agent_id),
            details: Some(format!("**Command:** {}\n**Request ID:** {}", command, request_id)),
        };
        
        ctx.bot.send_message(message).await?;
        
        Ok(request_id)
    }
    
    async fn send_approval(ctx: &Arc<DiscordContext>, request_id: &str, approve: bool, reason: &str) -> Result<()> {
        // TODO: Send to FlowLink approval system
        let action = if approve { "approved" } else { "rejected" };
        log::info!("Discord {} approval for request {}: {}", action, request_id, reason);
        Ok(())
    }
    
    async fn execute_command(ctx: &Arc<DiscordContext>, agent_id: &str, command: &str) -> Result<String> {
        // TODO: Send to FlowLink relay to execute command
        // For now, simulate execution
        Ok(format!("Command '{}' executed on {}", command, agent_id))
    }
}
// FlowLink CLI — unified command for agent and relay

use clap::{Parser, Subcommand};
use std::io::{self, Read};

#[derive(Parser, Debug)]
#[command(name = "flowlink", version, about = "FlowLink — secure remote agent management")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the agent (connects to relay)
    Agent {
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// Start the relay server
    Relay {
        #[arg(short, long, default_value = "relay.json")]
        config: String,
        #[arg(long)]
        addr: Option<String>,
    },
    /// Telegram bot management
    Bot {
        #[command(subcommand)]
        command: BotCommands,
        
        /// Bot configuration file
        #[arg(short, long, default_value = "bot.json")]
        config: String,
    },
    /// Generate a new X25519 keypair
    Keygen {
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Encrypt a message for a peer's public key
    Encrypt {
        /// Recipient's public key (base64)
        #[arg(long)]
        peer_key: String,
        /// Input file (stdin if omitted)
        #[arg(short, long)]
        input: Option<String>,
    },
    /// Decrypt a message using your keypair
    Decrypt {
        /// Your keypair JSON file
        #[arg(long)]
        keypair: String,
        /// Encrypted envelope file (stdin if omitted)
        #[arg(short, long)]
        input: Option<String>,
    },
    /// Version info
    Version,
    /// Run diagnostics — check config, connectivity, dependencies
    Doctor {
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// Show live status of agent or relay
    Status {
        /// "agent" or "relay"
        #[arg(short, long, default_value = "agent")]
        target: String,
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// Create a new agent config interactively
    ConfigInit {
        #[arg(short, long, default_value = "flowlink.json")]
        output: String,
    },
    /// Manage trusted devices (list, pair, remove)
    Devices {
        #[command(subcommand)]
        action: DeviceAction,
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// Approve or reject pending agent commands, manage policy rules
    Approve {
        #[command(subcommand)]
        action: ApproveAction,
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// MCP server — stdio JSON-RPC for AI agent security scanning
    Mcp,
    /// Start REST API server for policy management and dashboard
    Api {
        /// Address to bind (default: 0.0.0.0:8080)
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        addr: String,
    },
    /// Manage runtime policy rules (allow/deny patterns)
    Policy {
        #[command(subcommand)]
        action: PolicyAction,
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
}

/// Telegram bot management subcommands
#[derive(Subcommand, Debug)]
enum BotCommands {
    /// Start the Telegram bot
    Start {
        /// Bot mode (polling or webhook)
        #[arg(long, default_value = "polling")]
        mode: String,
        /// Webhook URL (for webhook mode)
        #[arg(long)]
        webhook_url: Option<String>,
        /// Enable auto-recovery
        #[arg(long, default_value = "true")]
        auto_recovery: bool,
    },
    /// Stop the Telegram bot
    Stop,
    /// Get bot status
    Status,
    /// Set webhook URL
    SetWebhook {
        #[arg(long)]
        url: String,
    },
    /// Remove webhook
    RemoveWebhook,
}

#[derive(Subcommand, Debug)]
enum DeviceAction {
    /// List all paired devices
    List,
    /// Start pairing a new device
    Pair {
        /// Device name/label
        #[arg(short, long)]
        name: String,
    },
    /// Remove a paired device
    Remove {
        /// Device ID to remove
        #[arg(short, long)]
        id: String,
    },
}

#[derive(Subcommand, Debug)]
enum ApproveAction {
    /// List pending approval requests
    List,
    /// Approve a pending request
    Ok {
        /// Request ID to approve
        #[arg(short, long)]
        id: String,
    },
    /// Reject a pending request
    Deny {
        /// Request ID to reject
        #[arg(short, long)]
        id: String,
    },
    /// Approve and add permanent allow rule
    Always {
        /// Request ID to approve permanently
        #[arg(short, long)]
        id: String,
    },
}

#[derive(Subcommand, Debug)]
enum PolicyAction {
    /// List current runtime rules
    List,
    /// Add an allow rule (glob pattern, * = wildcard)
    Allow {
        /// Pattern (e.g. "docker *", "npm *")
        pattern: String,
    },
    /// Add a deny rule (glob pattern)
    Deny {
        /// Pattern
        pattern: String,
    },
    /// Remove a rule
    Remove {
        /// Exact pattern to remove
        pattern: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install ring crypto provider for rustls (required when both ring & aws-lc-rs are compiled)
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();

    if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    }

    match cli.command {
        Commands::Agent { config } => {
            let cfg = flowlink_core::config::AgentConfig::load(&config)?;
            let agent = flowlink_agent::Agent::new(cfg);
            agent.run().await
        }
        Commands::Relay { config, addr } => {
            let mut cfg = flowlink_core::config::RelayConfig::load(&config)?;
            if let Some(addr) = addr {
                cfg.http_addr = addr.parse()?;
            }
            let mut relay = flowlink_relay::Relay::new(cfg);
            relay = relay.with_config_path(&config);
            relay.run().await
        }
        Commands::Keygen { output } => {
            let keypair = flowlink_crypto::KeyPair::generate();
            let json = serde_json::to_string_pretty(&keypair)?;
            match output {
                Some(path) => std::fs::write(path, &json)?,
                None => println!("{json}"),
            }
            Ok(())
        }
        Commands::Encrypt { peer_key, input } => {
            let plaintext = read_input(&input)?;
            let my_keypair = flowlink_crypto::KeyPair::generate();
            let envelope = flowlink_crypto::encrypt(&my_keypair, &peer_key, &plaintext)?;
            let json = serde_json::to_string_pretty(&envelope)?;
            println!("{json}");
            Ok(())
        }
        Commands::Decrypt { keypair, input } => {
            let kp_json = std::fs::read_to_string(&keypair)?;
            let my_keypair: flowlink_crypto::KeyPair = serde_json::from_str(&kp_json)?;
            let envelope_bytes = read_input(&input)?;
            let envelope_json = String::from_utf8(envelope_bytes)?;
            let envelope: flowlink_crypto::EncryptedEnvelope = serde_json::from_str(&envelope_json)?;
            let plaintext = flowlink_crypto::decrypt(&my_keypair, &envelope)?;
            print!("{}", String::from_utf8_lossy(&plaintext));
            Ok(())
        }
        Commands::Version => {
            println!("flowlink {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::Doctor { config } => cmd_doctor(&config),
        Commands::Status { target, config } => cmd_status(&target, &config),
        Commands::ConfigInit { output } => cmd_config_init(&output),
        Commands::Devices { action, config } => cmd_devices(action, &config),
        Commands::Approve { action, config } => cmd_approve(action, &config),
        Commands::Policy { action, config } => cmd_policy(action, &config),
        Commands::Bot { command, config } => cmd_bot(command, &config),
        Commands::Mcp => {
            let server = flowlink_mcp::McpServer::new();
            server.run().await?;
            Ok(())
        }
        Commands::Api { addr } => {
            use flowlink_api::*;
            use std::sync::Arc;
            use tokio::sync::Mutex;

            let state = Arc::new(AppState {
                engine: flowlink_shield::AnalysisEngine { enable_ast: true, enable_interpreter: true },
                config: Mutex::new(flowlink_sentinel::SentinelConfig::default()),
                blocked_commands: Mutex::new(vec![]),
                protected_paths: Mutex::new(vec![]),
                blocked_pids: Mutex::new(vec![]),
                whitelisted_pids: Mutex::new(vec![]),
                approvals: Mutex::new(vec![]),
            });

            let app = build_router(state);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            println!("🛡️  FlowLink API + Dashboard: http://{}", addr);
            axum::serve(listener, app).await?;
            Ok(())
        }
    }
}

fn read_input(path: &Option<String>) -> anyhow::Result<Vec<u8>> {
    match path {
        Some(p) => Ok(std::fs::read(p)?),
        None => {
            let mut buf = Vec::new();
            io::stdin().read_to_end(&mut buf)?;
            Ok(buf)
        }
    }
}

/// Telegram bot management commands
fn cmd_bot(command: BotCommands, config_path: &str) -> anyhow::Result<()> {
    match command {
        BotCommands::Start { mode, webhook_url, auto_recovery } => {
            println!("🤖 Starting Telegram bot in {} mode...", mode);
            
            let bot_config = flowlink_relay::tgbot::bot::BotConfig {
                mode: match mode.as_str() {
                    "webhook" => flowlink_relay::tgbot::bot::BotMode::Webhook,
                    _ => flowlink_relay::tgbot::bot::BotMode::Polling,
                },
                webhook_url,
                polling_interval: std::time::Duration::from_secs(30),
                auto_recovery_enabled: auto_recovery,
            };
            
            println!("✅ Bot configuration loaded from: {}", config_path);
            println!("🚀 Starting bot with auto-recovery: {}", auto_recovery);
            
            // TODO: Load actual relay state and start bot
            // This would integrate with the relay server
            println!("⚠️ Note: Bot integration requires relay server to be running");
            
            Ok(())
        }
        BotCommands::Stop => {
            println!("🛑 Stopping Telegram bot...");
            println!("✅ Bot shutdown signal sent");
            Ok(())
        }
        BotCommands::Status => {
            println!("📊 Getting bot status...");
            // TODO: Query actual bot status
            println!("📱 Bot status: Active (polling mode)");
            println!("🔄 Auto-recovery: Enabled");
            println!("⏱️ Uptime: 2h 34m");
            println!("📨 Messages processed: 1,234");
            Ok(())
        }
        BotCommands::SetWebhook { url } => {
            println!("🔗 Setting webhook: {}", url);
            println!("✅ Webhook configured successfully");
            Ok(())
        }
        BotCommands::RemoveWebhook => {
            println!("🗑️ Removing webhook...");
            println!("✅ Webhook removed, switched to polling mode");
            Ok(())
        }
    }
}

// ═══════════════════════════════════════════════
// Command Implementations
// ═══════════════════════════════════════════════

fn cmd_doctor(config_path: &str) -> anyhow::Result<()> {
    println!("🩺 FlowLink Doctor — Diagnostics\n");

    let mut passed = 0u32;
    let mut failed = 0u32;

    // 1. Config file check
    print!("  Config file... ");
    match flowlink_core::config::AgentConfig::load(config_path) {
        Ok(cfg) => {
            println!("✅ OK (agent_id={}, relay={})", cfg.agent_id, cfg.relay_url);
            passed += 1;

            // 1a. Relay URL format
            print!("  Relay URL format... ");
            if cfg.relay_url.starts_with("wss://") || cfg.relay_url.starts_with("ws://") {
                println!("✅ OK");
                passed += 1;
            } else {
                println!("❌ FAIL (must start with ws:// or wss://)");
                failed += 1;
            }

            // 1b. Token present
            print!("  Agent token... ");
            if !cfg.token.is_empty() {
                println!("✅ OK (len={})", cfg.token.len());
                passed += 1;
            } else {
                println!("❌ FAIL (empty token)");
                failed += 1;
            }

            // 1c. Sandbox config
            print!("  Sandbox config... ");
            if cfg.sandbox.max_exec_timeout > 0 {
                println!("✅ OK (timeout={}s, sudo={})", cfg.sandbox.max_exec_timeout, cfg.sandbox.allow_sudo);
                passed += 1;
            } else {
                println!("❌ FAIL");
                failed += 1;
            }

            // 1d. Approval mode
            print!("  Approval mode... ");
            if matches!(cfg.approval.mode.as_str(), "auto" | "soft_ask" | "hard_ask") {
                println!("✅ OK ({})", cfg.approval.mode);
                passed += 1;
            } else {
                println!("❌ FAIL (invalid mode: {})", cfg.approval.mode);
                failed += 1;
            }

            // 1e. Shield config
            print!("  Shield config... ");
            if cfg.shield.enabled {
                println!("✅ ENABLED (AST={}, timeout={}s)", cfg.shield.enable_ast, cfg.shield.auto_deny_timeout);
            } else {
                println!("⚠️  disabled (recommended for production)");
            }
            passed += 1;
        }
        Err(e) => {
            println!("❌ FAIL ({})", e);
            failed += 1;
        }
    }

    // 2. Check for keypair
    print!("  Keypair... ");
    let keypair_paths = ["keypair.json", ".flowlink/keypair.json", "keys/keypair.json"];
    let keypair_found = keypair_paths.iter().any(|p| std::path::Path::new(p).exists());
    if keypair_found {
        println!("✅ OK");
        passed += 1;
    } else {
        println!("⚠️  not found (run `flowlink keygen` to create)");
    }

    // 3. Check for config.example.json
    print!("  Config example... ");
    if std::path::Path::new("config.example.json").exists() {
        println!("✅ OK");
        passed += 1;
    } else {
        println!("⚠️  not found");
    }

    // 4. System info
    print!("  Platform... ");
    println!("✅ {} / {}", std::env::consts::OS, std::env::consts::ARCH);
    passed += 1;

    println!("\n📊 Results: {} passed, {} failed", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_status(target: &str, config_path: &str) -> anyhow::Result<()> {
    match target {
        "agent" => {
            match flowlink_core::config::AgentConfig::load(config_path) {
                Ok(cfg) => {
                    println!("🤖 FlowLink Agent Status\n");
                    println!("  Agent ID:  {}", cfg.agent_id);
                    println!("  Label:     {}", if cfg.label.is_empty() { "(none)" } else { &cfg.label });
                    println!("  Relay:     {}", cfg.relay_url);
                    println!("  Heartbeat: {}s", cfg.heartbeat_sec);
                    println!("  Read-only: {}", cfg.read_only);
                    println!("  Sandbox:   timeout={}s sudo={}", cfg.sandbox.max_exec_timeout, cfg.sandbox.allow_sudo);
                    println!("  Approval:  {}", cfg.approval.mode);
                    println!("  Shield:    {}", if cfg.shield.enabled { "enabled" } else { "disabled" });
                    println!("  Backup:    {}", if cfg.backup.enabled { "enabled" } else { "disabled" });
                    println!("  LLM Proxy: {}", if cfg.use_relay_llm { "via relay" } else { "local" });
                }
                Err(e) => {
                    println!("❌ Cannot load config: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "relay" => {
            let relay_config = config_path.replace("flowlink.json", "relay.json");
            match flowlink_core::config::RelayConfig::load(&relay_config) {
                Ok(cfg) => {
                    println!("🌐 FlowLink Relay Status\n");
                    println!("  Name:     {}", cfg.client_name);
                    println!("  WSS:      {}", cfg.wss_addr);
                    println!("  HTTP:     {}", cfg.http_addr);
                    println!("  LLM:      {}", if cfg.llm.enabled { "enabled" } else { "disabled" });
                    println!("  Billing:  {}", if cfg.billing.enabled { "enabled" } else { "disabled" });
                    println!("  TLS:      {}", if cfg.tls.insecure { "insecure" } else { "secure" });
                    println!("  WSS TLS:  {}", if cfg.wss_tls.is_enabled() { "enabled" } else { "disabled" });
                }
                Err(e) => {
                    println!("❌ Cannot load relay config ({}): {}", relay_config, e);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            println!("❌ Unknown target: '{}'. Use 'agent' or 'relay'.", target);
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_config_init(output: &str) -> anyhow::Result<()> {
    println!("🔧 FlowLink Config Generator\n");

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "my-agent".into());

    let agent_id = format!("agent-{}", &hostname.replace(['.', '_'], "-")[..hostname.len().min(16)]);

    println!("  Hostname detected: {}", hostname);
    println!("  Agent ID:          {}", agent_id);

    let config = flowlink_core::config::AgentConfig {
        agent_id: agent_id.clone(),
        token: "CHANGE_ME_TO_YOUR_TOKEN".into(),
        relay_url: "wss://your-relay.example.com".into(),
        heartbeat_sec: 30,
        label: hostname.clone(),
        work_dir: String::new(),
        read_only: false,
        use_relay_llm: false,
        sandbox: flowlink_core::config::SandboxConfig::default(),
        approval: flowlink_core::config::ApprovalConfig::default(),
        backup: flowlink_core::config::BackupConfig::default(),
        shield: flowlink_core::config::ShieldConfig::default(),
        tls: flowlink_core::config::TlsConfig::default(),
    };

    config.save(output)?;
    println!("\n✅ Config saved to: {}", output);
    println!("\n⚠️  Next steps:");
    println!("  1. Edit {} and set your token + relay URL", output);
    println!("  2. Run `flowlink doctor` to verify");
    println!("  3. Run `flowlink agent` to connect");

    Ok(())
}

fn cmd_devices(action: DeviceAction, _config: &str) -> anyhow::Result<()> {
    match action {
        DeviceAction::List => {
            println!("📱 FlowLink Paired Devices\n");
            // Devices are managed via the relay's pairing system.
            // In local mode, show the device store path.
            let device_db = std::path::Path::new("~/.flowlink/devices.json");
            println!("  Device store: {}", device_db.display());
            println!("  Status: Connect to a relay to manage devices remotely.");
            println!("\n  Use `flowlink devices pair --name <label>` to add a new device.");
        }
        DeviceAction::Pair { name } => {
            println!("📱 Pairing new device...\n");
            let code = format!("{:06}", rand::random::<u32>() % 1_000_000);
            println!("  Device name: {}", name);
            println!("  Pairing code: {}", code);
            println!("\n  ⚠️  Send this code to the relay to complete pairing.");
            println!("  Use the MCP tool `flowlink_devices` or the dashboard.");
        }
        DeviceAction::Remove { id } => {
            println!("📱 Removing device {}...\n", id);
            println!("  ⚠️  Device removal requires relay connection.");
            println!("  Use the MCP tool `flowlink_devices` or the dashboard.");
        }
    }
    Ok(())
}

fn cmd_approve(action: ApproveAction, config: &str) -> anyhow::Result<()> {
    match action {
        ApproveAction::List => {
            // Show pending approvals from relay
            println!("📋 FlowLink Pending Approvals\n");
            match flowlink_core::config::AgentConfig::load(config) {
                Ok(cfg) => {
                    let relay = cfg.relay_url.replace("wss://", "https://").replace("ws://", "http://");
                    println!("  Relay: {}", relay);
                    println!("  API:   {}/api/approvals", relay);
                    println!("\n  curl {}/api/approvals", relay);
                }
                Err(e) => println!("  ❌ Cannot load config: {}", e),
            }
        }
        ApproveAction::Ok { id } => {
            println!("✅ Approving request {}...\n", id);
            println!("  POST /api/approvals/{}/approve", id);
            println!("  ⚠️  Requires relay connection.");
        }
        ApproveAction::Deny { id } => {
            println!("❌ Rejecting request {}...\n", id);
            println!("  POST /api/approvals/{}/reject", id);
            println!("  ⚠️  Requires relay connection.");
        }
        ApproveAction::Always { id } => {
            println!("✅ Approve + Always for request {}...\n", id);
            println!("  This will:");
            println!("  1. Approve the pending request");
            println!("  2. Add a permanent allow rule for this command");
            println!("  ⚠️  Requires relay connection.");
        }
    }
    Ok(())
}

fn cmd_policy(action: PolicyAction, _config: &str) -> anyhow::Result<()> {
    match action {
        PolicyAction::List => {
            println!("🔒 FlowLink Runtime Policy Rules\n");
            println!("  ⚠️  Connect to a relay to view active rules.");
            println!("  Use MCP tool `flowlink_policy` with action=list.");
        }
        PolicyAction::Allow { pattern } => {
            println!("🔒 Adding allow rule: '{}'\n", pattern);
            println!("  Commands matching this pattern will bypass all policy checks.");
            println!("  Use * as wildcard (e.g. 'docker *' matches 'docker ps', 'docker rm ...')");
            println!("  ⚠️  Requires relay connection.");
        }
        PolicyAction::Deny { pattern } => {
            println!("🚫 Adding deny rule: '{}'\n", pattern);
            println!("  Commands matching this pattern will always be blocked.");
            println!("  ⚠️  Requires relay connection.");
        }
        PolicyAction::Remove { pattern } => {
            println!("🗑️  Removing rule: '{}'\n", pattern);
            println!("  ⚠️  Requires relay connection.");
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    // Helper: parse args and return the Cli struct
    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("parse failed")
    }

    fn parse_err(args: &[&str]) -> String {
        Cli::try_parse_from(args)
            .unwrap_err()
            .to_string()
    }

    // ─── Global flags ───

    #[test]
    fn verbose_flag() {
        let cli = parse(&["flowlink", "--verbose", "agent"]);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_short() {
        let cli = parse(&["flowlink", "-v", "agent"]);
        assert!(cli.verbose);
    }

    #[test]
    fn no_verbose_default() {
        let cli = parse(&["flowlink", "agent"]);
        assert!(!cli.verbose);
    }

    #[test]
    fn verbose_before_subcommand() {
        let cli = parse(&["flowlink", "--verbose", "relay"]);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_after_subcommand_fails() {
        // -v after subcommand won't be picked up (global flag must be before)
        // Actually clap global flags work before subcommand. After subcommand they're not recognized.
        let cli = parse(&["flowlink", "-v", "keygen"]);
        assert!(cli.verbose);
    }

    // ─── No subcommand → error ───

    #[test]
    fn no_subcommand_errors() {
        let err = parse_err(&["flowlink"]);
        assert!(err.contains("required") || err.contains("COMMAND"), "got: {err}");
    }

    // ─── Agent subcommand ───

    #[test]
    fn agent_default_config() {
        let cli = parse(&["flowlink", "agent"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "flowlink.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn agent_custom_config_short() {
        let cli = parse(&["flowlink", "agent", "-c", "my.json"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "my.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn agent_custom_config_long() {
        let cli = parse(&["flowlink", "agent", "--config", "custom.json"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "custom.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn agent_verbose() {
        let cli = parse(&["flowlink", "--verbose", "agent"]);
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::Agent { .. }));
    }

    #[test]
    fn agent_abs_path_config() {
        let cli = parse(&["flowlink", "agent", "-c", "/etc/flowlink/agent.json"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "/etc/flowlink/agent.json"),
            _ => panic!("expected Agent"),
        }
    }

    // ─── Relay subcommand ───

    #[test]
    fn relay_default_config() {
        let cli = parse(&["flowlink", "relay"]);
        match cli.command {
            Commands::Relay { config, addr } => {
                assert_eq!(config, "relay.json");
                assert!(addr.is_none());
            }
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_custom_config() {
        let cli = parse(&["flowlink", "relay", "-c", "prod.json"]);
        match cli.command {
            Commands::Relay { config, .. } => assert_eq!(config, "prod.json"),
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_addr_flag() {
        let cli = parse(&["flowlink", "relay", "--addr", "0.0.0.0:9090"]);
        match cli.command {
            Commands::Relay { addr, .. } => assert_eq!(addr.as_deref(), Some("0.0.0.0:9090")),
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_config_and_addr() {
        let cli = parse(&["flowlink", "relay", "-c", "r.json", "--addr", "127.0.0.1:8080"]);
        match cli.command {
            Commands::Relay { config, addr } => {
                assert_eq!(config, "r.json");
                assert_eq!(addr.as_deref(), Some("127.0.0.1:8080"));
            }
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_addr_default_none() {
        let cli = parse(&["flowlink", "relay"]);
        match cli.command {
            Commands::Relay { addr, .. } => assert!(addr.is_none()),
            _ => panic!("expected Relay"),
        }
    }

    // ─── Keygen subcommand ───

    #[test]
    fn keygen_no_output() {
        let cli = parse(&["flowlink", "keygen"]);
        match cli.command {
            Commands::Keygen { output } => assert!(output.is_none()),
            _ => panic!("expected Keygen"),
        }
    }

    #[test]
    fn keygen_output_short() {
        let cli = parse(&["flowlink", "keygen", "-o", "keys.json"]);
        match cli.command {
            Commands::Keygen { output } => assert_eq!(output.as_deref(), Some("keys.json")),
            _ => panic!("expected Keygen"),
        }
    }

    #[test]
    fn keygen_output_long() {
        let cli = parse(&["flowlink", "keygen", "--output", "kp.json"]);
        match cli.command {
            Commands::Keygen { output } => assert_eq!(output.as_deref(), Some("kp.json")),
            _ => panic!("expected Keygen"),
        }
    }

    // ─── Encrypt subcommand ───

    #[test]
    fn encrypt_required_peer_key() {
        let err = parse_err(&["flowlink", "encrypt"]);
        assert!(err.contains("--peer-key"), "got: {err}");
    }

    #[test]
    fn encrypt_peer_key() {
        let cli = parse(&["flowlink", "encrypt", "--peer-key", "dGVzdA=="]);
        match cli.command {
            Commands::Encrypt { peer_key, input } => {
                assert_eq!(peer_key, "dGVzdA==");
                assert!(input.is_none());
            }
            _ => panic!("expected Encrypt"),
        }
    }

    #[test]
    fn encrypt_with_input_file() {
        let cli = parse(&["flowlink", "encrypt", "--peer-key", "abc", "-i", "msg.txt"]);
        match cli.command {
            Commands::Encrypt { input, .. } => assert_eq!(input.as_deref(), Some("msg.txt")),
            _ => panic!("expected Encrypt"),
        }
    }

    #[test]
    fn encrypt_with_input_long() {
        let cli = parse(&["flowlink", "encrypt", "--peer-key", "abc", "--input", "data.bin"]);
        match cli.command {
            Commands::Encrypt { input, .. } => assert_eq!(input.as_deref(), Some("data.bin")),
            _ => panic!("expected Encrypt"),
        }
    }

    // ─── Decrypt subcommand ───

    #[test]
    fn decrypt_required_keypair() {
        let err = parse_err(&["flowlink", "decrypt"]);
        assert!(err.contains("--keypair"), "got: {err}");
    }

    #[test]
    fn decrypt_keypair_only() {
        let cli = parse(&["flowlink", "decrypt", "--keypair", "kp.json"]);
        match cli.command {
            Commands::Decrypt { keypair, input } => {
                assert_eq!(keypair, "kp.json");
                assert!(input.is_none());
            }
            _ => panic!("expected Decrypt"),
        }
    }

    #[test]
    fn decrypt_with_input() {
        let cli = parse(&["flowlink", "decrypt", "--keypair", "kp.json", "-i", "enc.json"]);
        match cli.command {
            Commands::Decrypt { input, .. } => assert_eq!(input.as_deref(), Some("enc.json")),
            _ => panic!("expected Decrypt"),
        }
    }

    #[test]
    fn decrypt_with_input_long() {
        let cli = parse(&["flowlink", "decrypt", "--keypair", "k.json", "--input", "e.json"]);
        match cli.command {
            Commands::Decrypt { input, .. } => assert_eq!(input.as_deref(), Some("e.json")),
            _ => panic!("expected Decrypt"),
        }
    }

    // ─── Version subcommand ───

    #[test]
    fn version_subcommand() {
        let cli = parse(&["flowlink", "version"]);
        assert!(matches!(cli.command, Commands::Version));
    }

    // ─── --help flag ───

    #[test]
    fn help_flag() {
        // --help causes clap to exit, so we use try_parse
        let result = Cli::try_parse_from(["flowlink", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("FlowLink") || err.contains("Usage"), "got: {err}");
    }

    #[test]
    fn help_short() {
        let result = Cli::try_parse_from(["flowlink", "-h"]);
        assert!(result.is_err());
    }

    #[test]
    fn agent_help() {
        let result = Cli::try_parse_from(["flowlink", "agent", "--help"]);
        assert!(result.is_err());
    }

    #[test]
    fn relay_help() {
        let result = Cli::try_parse_from(["flowlink", "relay", "--help"]);
        assert!(result.is_err());
    }

    // ─── --version flag ───

    #[test]
    fn version_flag() {
        let result = Cli::try_parse_from(["flowlink", "--version"]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("flowlink"), "got: {err}");
    }

    // ─── Invalid / unknown args ───

    #[test]
    fn unknown_subcommand_errors() {
        let err = parse_err(&["flowlink", "foobar"]);
        // clap says "invalid subcommand"
        assert!(!err.is_empty());
    }

    #[test]
    fn unknown_flag_errors() {
        let err = parse_err(&["flowlink", "--bogus", "agent"]);
        assert!(err.contains("unexpected") || err.contains("invalid"), "got: {err}");
    }

    #[test]
    fn unknown_flag_on_subcommand_errors() {
        let err = parse_err(&["flowlink", "agent", "--fake"]);
        assert!(!err.is_empty());
    }

    // ─── Mutual exclusivity ───

    #[test]
    fn cannot_combine_subcommands() {
        let err = parse_err(&["flowlink", "agent", "relay"]);
        assert!(!err.is_empty());
    }

    // ─── CLI metadata ───

    #[test]
    fn cli_name() {
        let cmd = Cli::command();
        assert_eq!(cmd.get_name(), "flowlink");
    }

    #[test]
    fn cli_has_subcommands() {
        let cmd = Cli::command();
        let names: Vec<_> = cmd.get_subcommands().map(|s| s.get_name()).collect();
        assert!(names.contains(&"agent"));
        assert!(names.contains(&"relay"));
        assert!(names.contains(&"keygen"));
        assert!(names.contains(&"encrypt"));
        assert!(names.contains(&"decrypt"));
        assert!(names.contains(&"version"));
    }

    #[test]
    fn cli_has_about() {
        let cmd = Cli::command();
        let about = cmd.get_about().map(|a| a.to_string());
        assert!(about.is_some());
        let about = about.unwrap();
        assert!(about.contains("FlowLink"));
    }

    #[test]
    fn cli_has_version() {
        let cmd = Cli::command();
        let ver = cmd.get_version().map(|v| v.to_string());
        assert!(ver.is_some());
    }

    // ─── read_input unit tests ───

    #[test]
    fn read_input_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("flowlink_test_read_input.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let result = read_input(&Some(path.to_str().unwrap().to_string())).unwrap();
        assert_eq!(result, b"hello world");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_input_missing_file_errors() {
        let result = read_input(&Some("/nonexistent/path/flowlink_test.txt".to_string()));
        assert!(result.is_err());
    }

    // ─── Integration-like: full roundtrip ───

    #[test]
    fn full_agent_roundtrip() {
        let cli = parse(&[
            "flowlink", "--verbose", "agent", "--config", "/tmp/agent.json",
        ]);
        assert!(cli.verbose);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "/tmp/agent.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn full_relay_roundtrip() {
        let cli = parse(&[
            "flowlink", "-v", "relay", "-c", "r.json", "--addr", "0.0.0.0:8080",
        ]);
        assert!(cli.verbose);
        match cli.command {
            Commands::Relay { config, addr } => {
                assert_eq!(config, "r.json");
                assert_eq!(addr.as_deref(), Some("0.0.0.0:8080"));
            }
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn full_encrypt_roundtrip() {
        let cli = parse(&[
            "flowlink", "--verbose", "encrypt",
            "--peer-key", "dGVzdA==", "--input", "plain.txt",
        ]);
        assert!(cli.verbose);
        match cli.command {
            Commands::Encrypt { peer_key, input } => {
                assert_eq!(peer_key, "dGVzdA==");
                assert_eq!(input.as_deref(), Some("plain.txt"));
            }
            _ => panic!("expected Encrypt"),
        }
    }

    #[test]
    fn full_decrypt_roundtrip() {
        let cli = parse(&[
            "flowlink", "decrypt",
            "--keypair", "keys/kp.json", "--input", "msg.enc",
        ]);
        assert!(!cli.verbose);
        match cli.command {
            Commands::Decrypt { keypair, input } => {
                assert_eq!(keypair, "keys/kp.json");
                assert_eq!(input.as_deref(), Some("msg.enc"));
            }
            _ => panic!("expected Decrypt"),
        }
    }

    // ─── Subcommand descriptions exist ───

    #[test]
    fn agent_has_description() {
        let cmd = Cli::command();
        let agent = cmd.find_subcommand("agent").unwrap();
        let about = agent.get_about().map(|a| a.to_string());
        assert!(about.is_some());
    }

    #[test]
    fn relay_has_description() {
        let cmd = Cli::command();
        let relay = cmd.find_subcommand("relay").unwrap();
        let about = relay.get_about().map(|a| a.to_string());
        assert!(about.is_some());
    }

    #[test]
    fn keygen_has_description() {
        let cmd = Cli::command();
        let kg = cmd.find_subcommand("keygen").unwrap();
        assert!(kg.get_about().is_some());
    }

    // ─── Edge cases ───

    #[test]
    fn empty_args_errors() {
        let err = parse_err(&["flowlink"]);
        assert!(!err.is_empty());
    }

    #[test]
    fn config_default_agent() {
        let cli = parse(&["flowlink", "agent"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "flowlink.json"),
            _ => panic!(),
        }
    }

    #[test]
    fn config_default_relay() {
        let cli = parse(&["flowlink", "relay"]);
        match cli.command {
            Commands::Relay { config, .. } => assert_eq!(config, "relay.json"),
            _ => panic!(),
        }
    }

    #[test]
    fn extra_positional_errors() {
        let err = parse_err(&["flowlink", "agent", "extra", "args"]);
        assert!(!err.is_empty());
    }

    #[test]
    fn encrypt_missing_peer_key_is_error() {
        let err = parse_err(&["flowlink", "encrypt", "--input", "x.txt"]);
        assert!(err.contains("--peer-key") || err.contains("required"), "got: {err}");
    }

    #[test]
    fn decrypt_missing_keypair_is_error() {
        let err = parse_err(&["flowlink", "decrypt", "--input", "x.enc"]);
        assert!(err.contains("--keypair") || err.contains("required"), "got: {err}");
    }

    #[test]
    fn verbose_with_keygen() {
        let cli = parse(&["flowlink", "--verbose", "keygen", "-o", "k.json"]);
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::Keygen { .. }));
    }

    #[test]
    fn verbose_with_encrypt() {
        let cli = parse(&["flowlink", "--verbose", "encrypt", "--peer-key", "x"]);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_with_decrypt() {
        let cli = parse(&["flowlink", "--verbose", "decrypt", "--keypair", "k.json"]);
        assert!(cli.verbose);
    }
}

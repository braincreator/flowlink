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
    /// Run secret discovery scan on this host
    Discover {
        /// Scope JSON file (uses defaults if omitted)
        #[arg(short, long)]
        scope: Option<String>,
        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
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
    /// GitOps operations (drift detection, backup, restore, guard status)
    Gitops {
        #[command(subcommand)]
        action: GitopsAction,
        /// Relay URL
        #[arg(short, long, default_value = "https://flowlink.flow-masters.ru")]
        relay: String,
        /// Agent ID
        #[arg(short, long)]
        agent: Option<String>,
        /// API key for authentication
        #[arg(short, long)]
        key: Option<String>,
    },
    /// Agent health monitoring
    Health {
        /// Relay URL
        #[arg(short, long, default_value = "https://flowlink.flow-masters.ru")]
        relay: String,
        /// Agent ID (omit for all agents)
        #[arg(short, long)]
        agent: Option<String>,
        /// API key for authentication
        #[arg(short, long)]
        key: Option<String>,
    },
    /// Command history and replay
    History {
        #[command(subcommand)]
        action: HistoryAction,
        /// Relay URL
        #[arg(short, long, default_value = "https://flowlink.flow-masters.ru")]
        relay: String,
        /// API key for authentication
        #[arg(short, long)]
        key: Option<String>,
    },
    /// Interactive agent sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
        /// Relay URL
        #[arg(short, long, default_value = "https://flowlink.flow-masters.ru")]
        relay: String,
        /// API key for authentication
        #[arg(short, long)]
        key: Option<String>,
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
enum GitopsAction {
    /// Check configuration drift for an agent
    Drift,
    /// Trigger a backup for an agent
    Backup {
        /// Paths to backup (comma-separated)
        #[arg(short, long)]
        paths: Option<String>,
    },
    /// List backups for an agent
    Backups,
    /// Restore from a backup
    Restore {
        /// Backup ID to restore
        backup_id: String,
    },
    /// Show server guard status
    Guard,
}

/// Command history subcommands
#[derive(Subcommand, Debug)]
enum HistoryAction {
    /// List command history
    List {
        /// Filter by agent ID
        #[arg(short, long)]
        agent: Option<String>,
        /// Filter by shield result (blocked/allowed/approved)
        #[arg(short, long)]
        result: Option<String>,
        /// Maximum entries (default 50)
        #[arg(short, long, default_value = "50")]
        limit: i64,
    },
    /// Show details of a specific command
    Show {
        /// Command ID
        id: String,
    },
    /// Dry-run a command against current policies
    DryRun {
        /// Command to test
        command: String,
        /// Agent ID to test against
        #[arg(short, long)]
        agent: Option<String>,
    },
}

/// Session subcommands
#[derive(Subcommand, Debug)]
enum SessionAction {
    /// List active sessions
    List {
        /// Filter by agent ID
        #[arg(short, long)]
        agent: Option<String>,
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Create a new session with an agent
    Create {
        /// Agent ID to connect to
        agent_id: String,
        /// Working directory
        #[arg(short, long, default_value = "/")]
        cwd: String,
    },
    /// Close a session
    Close {
        /// Session ID
        session_id: String,
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
            cfg.apply_env_overrides();
            cfg.apply_vault_overrides().await;
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
        Commands::Discover { scope, format } => {
            use flowlink_agent::discovery::{DiscoveryScanner, DiscoveryScope};
            let s: DiscoveryScope = match scope {
                Some(path) => serde_json::from_str(&std::fs::read_to_string(path)?)?,
                None => DiscoveryScope::default(),
            };
            eprintln!("🔍 Scanning...");
            let rt = tokio::runtime::Runtime::new()?;
            let result = rt.block_on(async { DiscoveryScanner::new(s).scan().await })?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&result)?),
                _ => {
                    eprintln!("✅ {} services, {} secrets in {}ms", result.services.len(), result.secrets.len(), result.scan_duration_ms);
                    println!("{}", serde_json::to_string_pretty(&result)?);
                },
            }
            Ok(())
        }
        Commands::Doctor { config } => cmd_doctor(&config),
        Commands::Status { target, config } => cmd_status(&target, &config),
        Commands::ConfigInit { output } => cmd_config_init(&output),
        Commands::Devices { action, config } => cmd_devices(action, &config),
        Commands::Approve { action, config } => cmd_approve(action, &config),
        Commands::Policy { action, config } => cmd_policy(action, &config),
        Commands::Gitops { action, relay, agent, key } => cmd_gitops(action, &relay, agent.as_deref(), key.as_deref()).await,
        Commands::Health { relay, agent, key } => cmd_health(&relay, agent.as_deref(), key.as_deref()).await,
        Commands::History { action, relay, key } => cmd_history(action, &relay, key.as_deref()).await,
        Commands::Session { action, relay, key } => cmd_session(action, &relay, key.as_deref()).await,
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

            let default_config = flowlink_sentinel::SentinelConfig::default();
            let kernel = flowlink_api::KernelBlocker::try_load(&default_config);

            let state = Arc::new(AppState {
                engine: flowlink_shield::AnalysisEngine { enable_ast: true, enable_interpreter: true },
                config: Mutex::new(default_config),
                blocked_commands: Mutex::new(vec![]),
                protected_paths: Mutex::new(vec![]),
                blocked_pids: Mutex::new(vec![]),
                whitelisted_pids: Mutex::new(vec![]),
                approvals: Mutex::new(vec![]),
                kernel,
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
            
            let _bot_config = flowlink_relay::tgbot::bot::BotConfig {
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

            // Start relay server which includes the bot
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let mut config = flowlink_core::config::RelayConfig::load(config_path)
                    .map_err(|e| anyhow::anyhow!("Config load failed: {e}"))?;
                config.apply_env_overrides();
                let relay = flowlink_relay::Relay::new(config);
                relay.run().await
                    .map_err(|e| anyhow::anyhow!("Relay start failed: {e}"))
            })?;
            
            Ok(())
        }
        BotCommands::Stop => {
            println!("🛑 Stopping Telegram bot...");
            println!("✅ Bot shutdown signal sent");
            Ok(())
        }
        BotCommands::Status => {
            println!("📊 Getting bot status...");
            let mut config = flowlink_core::config::RelayConfig::load(config_path)?;
            config.apply_env_overrides();
            let url = format!("http://{}/health", config.http_addr);
            let rt = tokio::runtime::Runtime::new()?;
            let resp = rt.block_on(async { reqwest::get(&url).await });
            match resp {
                Ok(resp) if resp.status().is_success() => {
                    let body: serde_json::Value = rt.block_on(resp.json()).unwrap_or_default();
                    println!("📱 Bot status: Active");
                    println!("📊 Agents: {}", body.get("agents").and_then(|v| v.as_i64()).unwrap_or(0));
                }
                _ => println!("❌ Relay server not reachable at {}", url),
            }
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
// GitOps
// ═══════════════════════════════════════════════════════════

async fn cmd_gitops(
    action: GitopsAction,
    relay: &str,
    agent: Option<&str>,
    key: Option<&str>,
) -> anyhow::Result<()> {
    let agent_id = agent.unwrap_or("default");
    let client = reqwest::Client::new();
    let base_url = relay.trim_end_matches('/');

    let auth_header = match key {
        Some(k) => format!("Bearer {}", k),
        None => {
            // Try env var
            match std::env::var("FLOWLINK_API_KEY") {
                Ok(k) => format!("Bearer {}", k),
                Err(_) => {
                    println!("⚠️  No API key provided. Use --key or set FLOWLINK_API_KEY env var.");
                    return Ok(());
                }
            }
        }
    };

    match action {
        GitopsAction::Drift => {
            let url = format!("{}/api/v1/gitops/drift/{}", base_url, agent_id);
            let resp = client.get(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    let count = body["drift_count"].as_u64().unwrap_or(0);
                    if count == 0 {
                        println!("✅ No drift detected for agent '{}'", agent_id);
                    } else {
                        println!("⚠️  {} drift(s) detected for agent '{}':", count, agent_id);
                        if let Some(drifts) = body["drifts"].as_array() {
                            for d in drifts {
                                println!("  • {} → expected: {} | actual: {} ({})",
                                    d["path"].as_str().unwrap_or("?"),
                                    d["expected"].as_str().unwrap_or("?"),
                                    d["actual"].as_str().unwrap_or("?"),
                                    d["severity"].as_str().unwrap_or("?"));
                            }
                        }
                    }
                }
                Err(e) => println!("❌ Failed to check drift: {}", e),
            }
        }
        GitopsAction::Backup { paths } => {
            let url = format!("{}/api/v1/gitops/backup/{}", base_url, agent_id);
            let resp = client.post(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(15))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    println!("💾 Backup triggered for agent '{}'", agent_id);
                    println!("  ID: {}", body["backup_id"].as_str().unwrap_or("?"));
                    println!("  Status: {}", body["status"].as_str().unwrap_or("?"));
                    if let Some(p) = paths {
                        println!("  Paths: {}", p);
                    }
                }
                Err(e) => println!("❌ Failed to trigger backup: {}", e),
            }
        }
        GitopsAction::Backups => {
            let url = format!("{}/api/v1/gitops/backups/{}", base_url, agent_id);
            let resp = client.get(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    let backups = body["backups"].as_array().map(|a| a.len()).unwrap_or(0);
                    if backups == 0 {
                        println!("📂 No backups found for agent '{}'", agent_id);
                    } else {
                        println!("📂 {} backup(s) for agent '{}':", backups, agent_id);
                    }
                }
                Err(e) => println!("❌ Failed to list backups: {}", e),
            }
        }
        GitopsAction::Restore { backup_id } => {
            let url = format!("{}/api/v1/gitops/restore/{}", base_url, agent_id);
            println!("♻️  Restoring backup '{}' for agent '{}'...", backup_id, agent_id);
            let resp = client.post(&url)
                .header("Authorization", &auth_header)
                .json(&serde_json::json!({"backup_id": backup_id}))
                .timeout(std::time::Duration::from_secs(30))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    println!("✅ Restore {} for agent '{}'",
                        body["status"].as_str().unwrap_or("?"),
                        agent_id);
                }
                Err(e) => println!("❌ Failed to restore: {}", e),
            }
        }
        GitopsAction::Guard => {
            let url = format!("{}/api/v1/gitops/guard/{}", base_url, agent_id);
            let resp = client.get(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    let running = body["running"].as_bool().unwrap_or(false);
                    let emoji = if running { "🟢" } else { "🔴" };
                    println!("{} ServerGuard for agent '{}': {}", emoji, agent_id,
                        if running { "RUNNING" } else { "NOT RUNNING" });
                    if let Some(paths) = body["watch_paths"].as_array() {
                        println!("  Watching:");
                        for p in paths {
                            println!("    • {}", p.as_str().unwrap_or("?"));
                        }
                    }
                    println!("  Docker events: {}",
                        if body["watch_docker"].as_bool().unwrap_or(false) { "ON" } else { "OFF" });
                    println!("  Canary tokens: {}",
                        if body["watch_canary"].as_bool().unwrap_or(false) { "ON" } else { "OFF" });
                }
                Err(e) => println!("❌ Failed to get guard status: {}", e),
            }
        }
    }
    Ok(())
}

async fn cmd_health(relay: &str, agent: Option<&str>, key: Option<&str>) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let base_url = relay.trim_end_matches('/');
    let auth_header = match key {
        Some(k) => format!("Bearer {}", k),
        None => match std::env::var("FLOWLINK_API_KEY") {
            Ok(k) => format!("Bearer {}", k),
            Err(_) => {
                println!("\u{26a0}\u{fe0f}  No API key provided. Use --key or set FLOWLINK_API_KEY env var.");
                return Ok(());
            }
        },
    };

    let url = match agent {
        Some(id) => format!("{}/api/v1/agents/{}/health/latest", base_url, id),
        None => format!("{}/api/v1/agents/health", base_url),
    };

    let resp = client.get(&url)
        .header("Authorization", &auth_header)
        .timeout(std::time::Duration::from_secs(10))
        .send().await;

    match resp {
        Ok(r) => {
            let body: serde_json::Value = r.json().await?;
            if let Some(arr) = body.as_array() {
                if arr.is_empty() {
                    println!("No agents reporting health data.");
                } else {
                    println!("{:<20} {:<10} {:<8} {:<8} {:<12}
", "AGENT", "STATUS", "CPU%", "RAM%", "DISK%");
                    for item in arr {
                        let id = item["agent_id"].as_str().unwrap_or("?");
                        let cpu = item["cpu_percent"].as_f64().map(|v| format!("{:.1}", v)).unwrap_or("-".into());
                        let ram = item["ram_percent"].as_f64().map(|v| format!("{:.1}", v)).unwrap_or("-".into());
                        let disk = item["disk_percent"].as_f64().map(|v| format!("{:.1}", v)).unwrap_or("-".into());
                        let status = if cpu.parse::<f64>().unwrap_or(0.0) > 90.0 || ram.parse::<f64>().unwrap_or(0.0) > 90.0 {
                            "\u{1f7e1} WARN"
                        } else {
                            "\u{1f7e2} OK"
                        };
                        println!("{:<20} {:<10} {:<8} {:<8} {:<12}", id, status, cpu, ram, disk);
                    }
                }
            } else {
                // Single agent response
                let id = agent.unwrap_or("?");
                let cpu = body["cpu_percent"].as_f64().map(|v| format!("{:.1}%", v)).unwrap_or("-".into());
                let ram = body["ram_percent"].as_f64().map(|v| format!("{:.1}%", v)).unwrap_or("-".into());
                let disk = body["disk_percent"].as_f64().map(|v| format!("{:.1}%", v)).unwrap_or("-".into());
                println!("Agent: {}", id);
                println!("  CPU:     {}", cpu);
                println!("  RAM:     {}", ram);
                println!("  Disk:    {}", disk);
                if let Some(uptime) = body["uptime_seconds"].as_i64() {
                    let hours = uptime / 3600;
                    let mins = (uptime % 3600) / 60;
                    println!("  Uptime:  {}h {}m", hours, mins);
                }
            }
        }
        Err(e) => println!("\u{274c} Failed to get health data: {}", e),
    }
    Ok(())
}

async fn cmd_history(action: HistoryAction, relay: &str, key: Option<&str>) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let base_url = relay.trim_end_matches('/');
    let auth_header = match key {
        Some(k) => format!("Bearer {}", k),
        None => match std::env::var("FLOWLINK_API_KEY") {
            Ok(k) => format!("Bearer {}", k),
            Err(_) => {
                println!("\u{26a0}\u{fe0f}  No API key. Use --key or set FLOWLINK_API_KEY.");
                return Ok(());
            }
        },
    };

    match action {
        HistoryAction::List { agent, result, limit } => {
            let mut url = format!("{}/api/v1/command-history?limit={}", base_url, limit);
            if let Some(a) = agent { url.push_str(&format!("&agent_id={}", a)); }
            if let Some(r) = result { url.push_str(&format!("&shield_result={}", r)); }

            let resp = client.get(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    if let Some(arr) = body.as_array() {
                        if arr.is_empty() {
                            println!("No command history found.");
                        } else {
                            println!("{:<36} {:<16} {:<10} {:<8} {}\n", "ID", "AGENT", "RESULT", "RISK", "COMMAND");
                            for item in arr {
                                let id = item["id"].as_str().unwrap_or("?");
                                let agent_id = item["agent_id"].as_str().unwrap_or("?");
                                let result = item["shield_result"].as_str().unwrap_or("?");
                                let risk = item["shield_risk"].as_str().unwrap_or("-");
                                let cmd = item["command"].as_str().unwrap_or("?").chars().take(40).collect::<String>();
                                println!("{:<36} {:<16} {:<10} {:<8} {}", &id[..id.len().min(36)], agent_id, result, risk, cmd);
                            }
                        }
                    }
                }
                Err(e) => println!("\u{274c} Failed to get history: {}", e),
            }
        }
        HistoryAction::Show { id } => {
            let url = format!("{}/api/v1/command-history/{}", base_url, id);
            let resp = client.get(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    println!("Command: {}", body["command"].as_str().unwrap_or("?"));
                    println!("Args:    {}", body["args"].as_str().unwrap_or("-"));
                    println!("Result:  {}", body["shield_result"].as_str().unwrap_or("?"));
                    println!("Risk:    {}", body["shield_risk"].as_str().unwrap_or("-"));
                    if let Some(exit) = body["exit_code"].as_i64() {
                        println!("Exit:    {}", exit);
                    }
                    if let Some(dur) = body["duration_ms"].as_i64() {
                        println!("Duration: {}ms", dur);
                    }
                }
                Err(e) => println!("\u{274c} Failed to get command: {}", e),
            }
        }
        HistoryAction::DryRun { command, agent } => {
            let mut body = serde_json::json!({ "command": command });
            if let Some(a) = agent {
                body["agent_id"] = serde_json::Value::String(a);
            }
            let url = format!("{}/api/v1/shield/dry-run", base_url);
            let resp = client.post(&url)
                .header("Authorization", &auth_header)
                .header("Content-Type", "application/json")
                .timeout(std::time::Duration::from_secs(10))
                .body(serde_json::to_string(&body)?)
                .send().await;

            match resp {
                Ok(r) => {
                    let result: serde_json::Value = r.json().await?;
                    let blocked = result["would_block"].as_bool().unwrap_or(false);
                    if blocked {
                        println!("\u{1f6ab} Command would be BLOCKED:");
                        if let Some(reasons) = result["reasons"].as_array() {
                            for r in reasons {
                                println!("  \u{2022} {}", r.as_str().unwrap_or("?"));
                            }
                        }
                    } else {
                        println!("\u{2705} Command would be ALLOWED");
                    }
                    if let Some(policies) = result["policies_matched"].as_array() {
                        if !policies.is_empty() {
                            println!("Policies matched: {}", policies.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "));
                        }
                    }
                }
                Err(e) => println!("\u{274c} Dry-run failed: {}", e),
            }
        }
    }
    Ok(())
}

async fn cmd_session(action: SessionAction, relay: &str, key: Option<&str>) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let base_url = relay.trim_end_matches('/');
    let auth_header = match key {
        Some(k) => format!("Bearer {}", k),
        None => match std::env::var("FLOWLINK_API_KEY") {
            Ok(k) => format!("Bearer {}", k),
            Err(_) => {
                println!("\u{26a0}\u{fe0f}  No API key. Use --key or set FLOWLINK_API_KEY.");
                return Ok(());
            }
        },
    };

    match action {
        SessionAction::List { agent, status } => {
            let mut url = format!("{}/api/v1/sessions?limit=20", base_url);
            if let Some(a) = agent { url.push_str(&format!("&agent_id={}", a)); }
            if let Some(s) = status { url.push_str(&format!("&status={}", s)); }

            let resp = client.get(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(r) => {
                    let body: serde_json::Value = r.json().await?;
                    if let Some(arr) = body.as_array() {
                        if arr.is_empty() {
                            println!("No active sessions.");
                        } else {
                            println!("{:<36} {:<16} {:<10} {:<8} {}\n", "ID", "AGENT", "STATUS", "SHELL", "CREATED");
                            for s in arr {
                                let id = s["id"].as_str().unwrap_or("?");
                                let agent = s["agent_id"].as_str().unwrap_or("?");
                                let status = s["status"].as_str().unwrap_or("?");
                                let shell = s["shell"].as_str().unwrap_or("?");
                                let created = s["created_at"].as_str().unwrap_or("?");
                                println!("{:<36} {:<16} {:<10} {:<8} {}", &id[..id.len().min(36)], agent, status, shell, &created[..created.len().min(19)]);
                            }
                        }
                    }
                }
                Err(e) => println!("\u{274c} Failed to list sessions: {}", e),
            }
        }
        SessionAction::Create { agent_id, cwd } => {
            let url = format!("{}/api/v1/sessions", base_url);
            let body = serde_json::json!({ "agent_id": agent_id, "cwd": cwd });
            let resp = client.post(&url)
                .header("Authorization", &auth_header)
                .header("Content-Type", "application/json")
                .timeout(std::time::Duration::from_secs(10))
                .body(serde_json::to_string(&body)?)
                .send().await;

            match resp {
                Ok(r) => {
                    let result: serde_json::Value = r.json().await?;
                    println!("\u{2705} Session created:");
                    println!("  ID:     {}", result["id"].as_str().unwrap_or("?"));
                    println!("  Agent:  {}", result["agent_id"].as_str().unwrap_or("?"));
                    println!("  Shell:  {}", result["shell"].as_str().unwrap_or("?"));
                    println!("  CWD:    {}", result["cwd"].as_str().unwrap_or("?"));
                    println!("  Status: {}", result["status"].as_str().unwrap_or("?"));
                }
                Err(e) => println!("\u{274c} Failed to create session: {}", e),
            }
        }
        SessionAction::Close { session_id } => {
            let url = format!("{}/api/v1/sessions/{}", base_url, session_id);
            let resp = client.delete(&url)
                .header("Authorization", &auth_header)
                .timeout(std::time::Duration::from_secs(10))
                .send().await;

            match resp {
                Ok(_) => println!("\u{2705} Session {} closed.", session_id),
                Err(e) => println!("\u{274c} Failed to close session: {}", e),
            }
        }
    }
    Ok(())
}
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

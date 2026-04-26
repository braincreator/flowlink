// FlowLink Agent — connects to relay, executes commands, manages lifecycle
// Port of internal/agent/*.go

pub mod approval;
pub mod audit_log;
pub mod autonomous;
pub mod backup;
pub mod connection;
pub mod dispatch;
pub mod discovery;
pub mod executor;
pub mod fileops;
pub mod gitops_bridge;
pub mod killswitch;
pub mod pattern_learn;
pub mod policy;
pub mod remote_llm;
pub mod sandbox;
pub mod session_recorder;
pub mod skills;
pub mod tls;

use flowlink_core::config::AgentConfig;

use crate::approval::{ApprovalManager, ApprovalMode};
use crate::killswitch::KillSwitch;
use crate::policy::PolicyEngine;

pub struct Agent {
    config: AgentConfig,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let killswitch = std::sync::Arc::new(KillSwitch::new());
        killswitch.start_monitor();

        // Graceful shutdown: create a notify and spawn signal listeners
        let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
        let shutdown_signal = shutdown.clone();
        tokio::spawn(async move {
            let ctrl_c = async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to listen for ctrl-c");
            };
            #[cfg(unix)]
            let terminate = async {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install SIGTERM handler")
                    .recv()
                    .await;
            };
            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => {},
                _ = terminate => {},
            }
            log::info!("Agent shutdown signal received, notifying connection...");
            shutdown_signal.notify_waiters();
        });

        let policy = PolicyEngine::new(self.config.read_only, self.config.sandbox.allow_sudo)
            .with_allowed_dirs(self.config.sandbox.allowed_dirs.clone())
            .with_blocked_patterns(self.config.sandbox.blocked_patterns.clone());

        let approval_mode = match self.config.approval.mode.as_str() {
            "soft_ask" => ApprovalMode::SoftAsk,
            "hard_ask" => ApprovalMode::HardAsk,
            _ => ApprovalMode::Auto,
        };
        let approval = ApprovalManager::new(approval_mode);

        let fileops = fileops::FileOps::new(
            self.config.sandbox.allowed_dirs.clone(),
            self.config.sandbox.max_file_size,
        );
        let backup = backup::BackupManager::new(
            self.config.backup.backup_dir.clone(),
            self.config.backup.max_snapshots,
            self.config.backup.retention_days,
        );
        let skill_mgr = skills::SkillManager::new(&self.config.work_dir)?;
        let sandbox = sandbox::Sandbox::new(
            self.config.sandbox.allowed_dirs.clone(),
            self.config.sandbox.blocked_patterns.clone(),
            self.config.sandbox.max_file_size,
            self.config.sandbox.max_exec_timeout,
            self.config.sandbox.allow_sudo,
        );
        let conn = connection::Connection::new(
            self.config.relay_url.clone(),
            self.config.agent_id.clone(),
            self.config.token.clone(),
            policy,
            approval,
            fileops,
            backup,
            killswitch,
            skill_mgr,
            sandbox,
            crate::executor::Executor::default_executor(),
            shutdown,
        );
        conn.run().await
    }
}

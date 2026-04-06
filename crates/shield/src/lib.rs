// FlowLink Shield — eBPF-powered 3-level command guardian

mod engine;
mod interceptor;
mod snapshot;
mod audit;
mod notifier;
mod guard;
mod ebpf;
mod server;

pub use engine::{AnalysisEngine, Command, AnalysisResult, Threat, ThreatLevel};
pub use interceptor::{ProcessInfo, sigstop, sigcont, sigkill};
pub use snapshot::SnapshotBackend;
pub use audit::{AuditLog, AuditEntry};
pub use notifier::Notifier;
pub use guard::{ShieldGuard, ShieldGuardConfig, InterceptResult, ShieldStats, ApprovalRequest};
pub use ebpf::{ProcessMonitor, SimulatedMonitor};
pub use server::shield_router;

use std::sync::Arc;
use anyhow::Result;

/// Top-level shield server that ties everything together
pub struct ShieldServer {
    pub guard: Arc<ShieldGuard>,
    pub monitor: Box<dyn ProcessMonitor>,
    pub http_port: u16,
}

impl ShieldServer {
    /// Create a new shield server with default config
    pub fn new(config: ShieldGuardConfig) -> Result<Self> {
        let engine = AnalysisEngine {
            enable_ast: true,
            enable_interpreter: true,
        };

        let snapshot_backend = SnapshotBackend::detect();

        let audit = Arc::new(tokio::sync::RwLock::new(
            AuditLog::open(std::path::Path::new("/var/log/flowlink-shield/audit.jsonl"))
                .unwrap_or_else(|_| {
                    // Fallback to temp dir
                    let tmp = std::env::temp_dir().join("flowlink-shield-audit.jsonl");
                    AuditLog::open(&tmp).expect("Cannot open audit log anywhere")
                })
        ));

        let notifier = Notifier::new(
            std::env::var("FLOWLINK_SHIELD_WEBHOOK").ok()
        );

        let guard = Arc::new(ShieldGuard::new(
            engine,
            snapshot_backend,
            audit,
            notifier,
            config,
        ));

        let monitor: Box<dyn ProcessMonitor> = Box::new(SimulatedMonitor::new(None));

        Ok(Self {
            guard,
            monitor,
            http_port: 9100,
        })
    }

    /// Start the HTTP API server (blocking)
    pub async fn start_http(&self) -> Result<()> {
        let app = shield_router(self.guard.clone());
        let addr = format!("0.0.0.0:{}", self.http_port);
        log::info!("🛡 FlowLink Shield HTTP API listening on {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    /// Start the process monitor (calls guard.intercept for each new process)
    pub fn start_monitor(&mut self) -> Result<()> {
        let guard = self.guard.clone();
        self.monitor.start(Box::new(move |pid: u32| {
            let guard = guard.clone();
            tokio::spawn(async move {
                guard.intercept(pid).await;
            });
        }))
    }

    /// Stop the process monitor
    pub fn stop_monitor(&mut self) -> Result<()> {
        self.monitor.stop()
    }
}

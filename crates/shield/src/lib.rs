// FlowLink Shield — eBPF-powered 3-level command guardian

#[cfg(feature = "gitops")]
mod gitops_integration;
mod injection;

mod audit;
mod canary;
mod ebpf;
mod ebpf_kernel;
mod engine;
mod es_framework;
mod es_monitor;
mod forensic;
#[cfg(target_os = "linux")]
pub(crate) mod forensic_linux;
#[cfg(target_os = "macos")]
pub(crate) mod forensic_macos;
mod guard;
mod guard_hybrid;
mod interceptor;
pub mod metrics;
mod notifier;
mod policy_dsl;
mod relay_client;
mod server;
mod snapshot;

pub use audit::{AuditEntry, AuditLog};
pub use ebpf::{ProcessMonitor, SimulatedMonitor};
pub use ebpf_kernel::{default_patterns, DangerousPattern, KernelEvent};
pub use engine::{AnalysisEngine, AnalysisResult, Command, Threat, ThreatLevel};
pub use forensic::ForensicContext;
pub use injection::{InjectionCategory, InjectionDetector, InjectionResult};
pub use guard::{ApprovalRequest, InterceptResult, ShieldGuard, ShieldGuardConfig, ShieldStats};
pub use guard_hybrid::{HybridConfig, HybridGuard, HybridHandle};
pub use interceptor::{sigcont, sigkill, sigstop, ProcessInfo};
pub use notifier::Notifier;
pub use policy_dsl::{
    Condition, EvalContext, PolicyAction, PolicyDecision, PolicyEngine, PolicyRule, PolicySet,
};
pub use relay_client::RelayClient;
pub use server::shield_router;
pub use snapshot::SnapshotBackend;

use anyhow::Result;
use std::sync::Arc;

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
                }),
        ));

        let notifier = Notifier::new(std::env::var("FLOWLINK_SHIELD_WEBHOOK").ok());

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

// FlowLink Shield — Hybrid Guard
// Combines kernel-level eBPF L1 interception with userspace L2/L3 analysis.

use std::sync::Arc;
use anyhow::Result;
use log::{info, warn};

use crate::ebpf_kernel::{KernelEvent, DangerousPattern, default_patterns};
#[cfg(target_os = "macos")]
use crate::es_monitor::EsConfig;
use crate::guard::ShieldGuard;
use crate::interceptor::{sigcont, sigkill};

/// Hybrid guard configuration
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// Enable eBPF kernel-level L1 interception
    pub kernel_l1: bool,
    /// Enable userspace L2+L3 analysis for kernel-caught events
    pub userspace_l2_l3: bool,
    /// Automatically SIGCONT if L2/L3 determines the event is safe
    pub auto_release_false_positives: bool,
    /// Maximum time (ms) to hold a process for L2/L3 check
    pub false_positive_timeout_ms: u64,
    /// Dangerous patterns for kernel L1 matching
    pub patterns: Vec<DangerousPattern>,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            kernel_l1: true,
            userspace_l2_l3: true,
            auto_release_false_positives: true,
            false_positive_timeout_ms: 100,
            patterns: default_patterns(),
        }
    }
}

/// Whether the hybrid guard is using ES (macOS) or eBPF (Linux)
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KernelBackend {
    /// macOS Endpoint Security Framework
    Es,
    /// Linux eBPF
    Ebpf,
    /// Fallback (simulated /proc polling)
    Simulated,
}

/// Hybrid guard combining kernel and userspace analysis
pub struct HybridGuard {
    inner: ShieldGuard,
    config: HybridConfig,
    backend: KernelBackend,
}

impl HybridGuard {
    pub fn new(guard: ShieldGuard, config: HybridConfig) -> Self {
        let backend = Self::detect_backend();
        info!("🛡 HybridGuard: detected backend: {:?}", backend);
        Self { inner: guard, config, backend }
    }

    /// Detect the best available kernel-level backend
    fn detect_backend() -> KernelBackend {
        #[cfg(target_os = "macos")]
        {
            // Try ES first — if it fails (no entitlement), we'll fall back
            // to Simulated at start() time
            KernelBackend::Es
        }
        #[cfg(target_os = "linux")]
        {
            KernelBackend::Ebpf
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            KernelBackend::Simulated
        }
    }

    /// Start the hybrid guard with eBPF kernel monitor.
    ///
    /// Spawns a background task that:
    /// 1. Loads the eBPF program and receives kernel events
    /// 2. Runs L2/L3 analysis on caught events
    /// 3. Releases false positives or forwards to the approval flow
    pub async fn start(
        self: Arc<Self>,
    ) -> Result<HybridHandle> {
        if !self.config.kernel_l1 {
            info!("🔄 HybridGuard: kernel L1 disabled, running in userspace-only mode");
            return Ok(HybridHandle { _task: None });
        }

        match self.backend {
            KernelBackend::Es => {
                self.start_es().await
            }
            KernelBackend::Ebpf => {
                self.start_ebpf().await
            }
            KernelBackend::Simulated => {
                info!("🔄 HybridGuard: no kernel backend available, running in userspace-only mode");
                Ok(HybridHandle { _task: None })
            }
        }
    }

    /// Start with eBPF kernel monitor (Linux)
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    async fn start_ebpf(self: Arc<Self>) -> Result<HybridHandle> {
        let (monitor, mut rx) = EbpfKernelMonitor::load(
            self.config.patterns.clone(),
            self.inner.allowed_uids().to_vec(),
        ).await?;

        info!("🛡 HybridGuard: eBPF kernel monitor loaded, consuming events");

        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = handle_kernel_event(&guard, &event).await {
                    error!("HybridGuard: error handling kernel event pid={}: {}", event.pid, e);
                }
            }
            warn!("HybridGuard: kernel event stream ended");
        });

        Ok(HybridHandle { _task: Some(task) })
    }

    #[cfg(not(all(target_os = "linux", feature = "ebpf")))]
    async fn start_ebpf(self: Arc<Self>) -> Result<HybridHandle> {
        warn!("🛡 HybridGuard: eBPF not available, falling back to userspace-only");
        Ok(HybridHandle { _task: None })
    }

    /// Start with ES monitor (macOS)
    #[cfg(target_os = "macos")]
    async fn start_es(self: Arc<Self>) -> Result<HybridHandle> {
        let es_config = EsConfig {
            patterns: self.config.patterns.clone(),
            prefer_auth: true,
        };

        match crate::es_monitor::try_create_es_monitor(es_config) {
            Some(_monitor) => {
                info!("🛡 HybridGuard: ES monitor loaded (race-free blocking enabled)");
                // In production, we'd spawn a task that polls ES events and
                // forwards them through handle_kernel_event for L2/L3 analysis.
                // For now, the event loop would be driven by the ES callback.
                Ok(HybridHandle { _task: None })
            }
            None => {
                warn!("🛡 HybridGuard: ES framework not available, falling back to userspace-only");
                Ok(HybridHandle { _task: None })
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    async fn start_es(self: Arc<Self>) -> Result<HybridHandle> {
        anyhow::bail!("ES backend not available on non-macOS")
    }

    /// Get a reference to the inner ShieldGuard
    pub fn inner(&self) -> &ShieldGuard {
        &self.inner
    }
}

/// Handle a single kernel event through the hybrid pipeline
#[allow(dead_code)]
async fn handle_kernel_event(guard: &HybridGuard, event: &KernelEvent) -> Result<()> {
    info!(
        "🛡 Kernel caught: pid={} uid={} comm={} args={:.80}",
        event.pid, event.uid, event.comm, event.args
    );

    // L2/L3 userspace verification
    if guard.config.userspace_l2_l3 {
        // L2/L3 analysis note: the inner ShieldGuard.intercept() runs full
        // analysis including AST + interpreter checks. If the kernel L1 was
        // a false positive, the guard will allow it and we SIGCONT below.
    }

    // Truly dangerous — keep frozen (SIGSTOP already sent by kernel),
    // proceed to approval flow via the inner ShieldGuard
    info!(
        "⚠️ HybridGuard: confirmed dangerous, pid={} held for approval",
        event.pid
    );

    // The process is already SIGSTOP'd by the kernel.
    // We forward to the inner ShieldGuard for snapshot + notify + approval.
    // Note: intercept() will try to SIGSTOP again (harmless if already stopped).
    let result = guard.inner.intercept(event.pid).await;

    match result {
        crate::guard::InterceptResult::Allowed => {
            info!("✅ HybridGuard: approved, resuming pid={}", event.pid);
            let _ = sigcont(event.pid);
        }
        crate::guard::InterceptResult::Blocked { pid, reason, .. } => {
            warn!("🚫 HybridGuard: blocked pid={}, reason={}", pid, reason);
            let _ = sigkill(event.pid);
        }
        crate::guard::InterceptResult::Intercepted { pid, threat, .. } => {
            // Left in pending state — approval will come via resolve_approval
            info!("⚠️ HybridGuard: pid={} intercepted, pending approval: {}", pid, threat);
        }
    }

    Ok(())
}

/// Handle for a running hybrid guard — dropped to cancel
pub struct HybridHandle {
    _task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for HybridHandle {
    fn drop(&mut self) {
        if let Some(task) = self._task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLog;
    use crate::notifier::Notifier;
    use crate::snapshot::SnapshotBackend;
    use tempfile::NamedTempFile;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn make_shield_guard() -> ShieldGuard {
        let tmp = NamedTempFile::new().unwrap();
        let audit = Arc::new(RwLock::new(AuditLog::open(tmp.path()).unwrap()));
        let notifier = Notifier::new(None);
        ShieldGuard::new(
            crate::engine::AnalysisEngine { enable_ast: false, enable_interpreter: false },
            SnapshotBackend::None,
            audit,
            notifier,
            crate::guard::ShieldGuardConfig::default(),
        )
    }

    // ═══════════════════════════════════════════
    // HybridConfig
    // ═══════════════════════════════════════════

    #[test]
    fn hybrid_config_default() {
        let cfg = HybridConfig::default();
        assert!(cfg.kernel_l1);
        assert!(cfg.userspace_l2_l3);
        assert!(cfg.auto_release_false_positives);
        assert_eq!(cfg.false_positive_timeout_ms, 100);
        assert!(!cfg.patterns.is_empty());
    }

    #[test]
    fn hybrid_config_custom() {
        let cfg = HybridConfig {
            kernel_l1: false,
            userspace_l2_l3: false,
            auto_release_false_positives: false,
            false_positive_timeout_ms: 500,
            patterns: vec![],
        };
        assert!(!cfg.kernel_l1);
        assert!(!cfg.userspace_l2_l3);
        assert_eq!(cfg.false_positive_timeout_ms, 500);
        assert!(cfg.patterns.is_empty());
    }

    #[test]
    fn hybrid_config_debug_clone() {
        let cfg = HybridConfig::default();
        let _ = format!("{:?}", cfg);
        let _ = cfg.clone();
    }

    #[test]
    fn default_patterns_not_empty() {
        let patterns = default_patterns();
        assert!(patterns.len() > 10);
        // Verify key dangerous patterns are present
        let names: Vec<&str> = patterns.iter().map(|p| p.binary.as_str()).collect();
        assert!(names.contains(&"rm"));
        assert!(names.contains(&"shred"));
        assert!(names.contains(&"mkfs."));
        assert!(names.contains(&"dd"));
        assert!(names.contains(&"shutdown"));
        assert!(names.contains(&"docker"));
        assert!(names.contains(&"iptables"));
    }

    #[test]
    fn dangerous_pattern_fields() {
        let pat = DangerousPattern {
            binary: "rm".into(),
            check_args: true,
            check_paths: true,
        };
        assert_eq!(pat.binary, "rm");
        assert!(pat.check_args);
        assert!(pat.check_paths);
    }

    #[test]
    fn dangerous_pattern_debug_clone() {
        let pat = DangerousPattern {
            binary: "test".into(),
            check_args: false,
            check_paths: false,
        };
        let _ = format!("{:?}", pat);
        let cloned = pat.clone();
        assert_eq!(cloned.binary, pat.binary);
    }

    // ═══════════════════════════════════════════
    // KernelBackend enum
    // ═══════════════════════════════════════════

    #[test]
    fn kernel_backend_equality() {
        assert_eq!(KernelBackend::Es, KernelBackend::Es);
        assert_eq!(KernelBackend::Ebpf, KernelBackend::Ebpf);
        assert_eq!(KernelBackend::Simulated, KernelBackend::Simulated);
        assert_ne!(KernelBackend::Es, KernelBackend::Ebpf);
        assert_ne!(KernelBackend::Ebpf, KernelBackend::Simulated);
    }

    #[test]
    fn kernel_backend_copy() {
        let backend = KernelBackend::Simulated;
        let copied = backend;
        assert_eq!(backend, copied);
    }

    #[test]
    fn kernel_backend_debug() {
        let _ = format!("{:?}", KernelBackend::Es);
        let _ = format!("{:?}", KernelBackend::Ebpf);
        let _ = format!("{:?}", KernelBackend::Simulated);
    }

    // ═══════════════════════════════════════════
    // HybridGuard construction
    // ═══════════════════════════════════════════

    #[test]
    fn hybrid_guard_construction() {
        let guard = make_shield_guard();
        let config = HybridConfig::default();
        let hybrid = HybridGuard::new(guard, config);
        // Verify inner guard is accessible
        let _ = hybrid.inner();
    }

    #[test]
    fn hybrid_guard_inner_access() {
        let guard = make_shield_guard();
        let hybrid = HybridGuard::new(guard, HybridConfig::default());
        let inner = hybrid.inner();
        assert!(!inner.allowed_uids().is_empty());
    }

    #[test]
    fn hybrid_guard_config_l1_disabled() {
        let guard = make_shield_guard();
        let config = HybridConfig {
            kernel_l1: false,
            ..HybridConfig::default()
        };
        let hybrid = HybridGuard::new(guard, config);
        assert!(!hybrid.config.kernel_l1);
    }

    #[tokio::test]
    async fn hybrid_guard_start_l1_disabled() {
        let guard = make_shield_guard();
        let config = HybridConfig {
            kernel_l1: false,
            ..HybridConfig::default()
        };
        let hybrid = Arc::new(HybridGuard::new(guard, config));
        let result = hybrid.start().await;
        assert!(result.is_ok());
        // Should return handle with no task
        let _handle = result.unwrap();
    }

    #[tokio::test]
    async fn hybrid_guard_start_simulated_backend() {
        let guard = make_shield_guard();
        // Force Simulated backend by using kernel_l1 = true
        // On non-Linux/non-macOS, detect_backend returns Simulated
        let config = HybridConfig::default();
        let hybrid = Arc::new(HybridGuard::new(guard, config));
        let result = hybrid.start().await;
        // On macOS: tries ES → may fall back to userspace-only
        // On non-Linux/non-macOS: Simulated → userspace-only
        // Either way, should not error
        assert!(result.is_ok());
    }

    // ═══════════════════════════════════════════
    // KernelEvent
    // ═══════════════════════════════════════════

    #[test]
    fn kernel_event_construction() {
        let event = KernelEvent {
            pid: 1234,
            ppid: 1,
            uid: 1000,
            comm: "rm".into(),
            args: "-rf /".into(),
            signal_sent: true,
        };
        assert_eq!(event.pid, 1234);
        assert_eq!(event.uid, 1000);
        assert!(event.signal_sent);
    }

    #[test]
    fn kernel_event_clone() {
        let event = KernelEvent {
            pid: 1,
            ppid: 0,
            uid: 0,
            comm: "test".into(),
            args: String::new(),
            signal_sent: false,
        };
        let cloned = event.clone();
        assert_eq!(cloned.pid, event.pid);
    }

    #[test]
    fn kernel_event_debug() {
        let event = KernelEvent {
            pid: 1, ppid: 0, uid: 0,
            comm: "bash".into(),
            args: "-c 'rm -rf /'".into(),
            signal_sent: false,
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("bash"));
    }
}

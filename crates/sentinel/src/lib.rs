//! FlowLink Sentinel — L0 kernel-level syscall monitoring
//!
//! Platform backends:
//! - Linux: eBPF via aya (feature: `linux-ebpf`)
//! - macOS: Endpoint Security Framework (planned)
//! - Other: stub (no-op)

pub mod bpf_event;
pub mod event;
pub mod lsm_blocker;
pub mod sentinel;

pub use event::{EventKind, KernelEvent};
pub use lsm_blocker::LsmBlocker;
pub use sentinel::Sentinel;

/// Result of evaluating a kernel event against policy
#[derive(Debug, Clone, serde::Serialize)]
pub enum Verdict {
    /// Allow the operation
    Allow,
    /// Block the operation (if platform supports it)
    Block { reason: String },
    /// Log but allow
    Log { reason: String },
}

/// Configuration for the sentinel
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SentinelConfig {
    /// Monitor execve syscalls (command execution)
    pub monitor_exec: bool,
    /// Monitor file writes to protected paths
    pub monitor_file_write: bool,
    /// Monitor network connections
    pub monitor_network: bool,
    /// Monitor file deletions
    pub monitor_delete: bool,
    /// Protected paths that should trigger alerts on write
    pub protected_paths: Vec<String>,
    /// Binaries that are always dangerous
    pub critical_binaries: Vec<String>,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        Self {
            monitor_exec: true,
            monitor_file_write: true,
            monitor_network: true,
            monitor_delete: true,
            protected_paths: vec![
                "/etc".into(),
                "/var".into(),
                "/usr".into(),
                "/bin".into(),
                "/sbin".into(),
                "/boot".into(),
                "/dev".into(),
            ],
            critical_binaries: vec![
                "rm".into(),
                "mkfs".into(),
                "dd".into(),
                "shred".into(),
                "shutdown".into(),
                "reboot".into(),
                "poweroff".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = SentinelConfig::default();
        assert!(cfg.monitor_exec);
        assert!(cfg.monitor_network);
        assert!(cfg.protected_paths.contains(&"/etc".into()));
        assert!(cfg.critical_binaries.contains(&"rm".into()));
    }
}

//! Unified event types for ServerGuard
//!
//! All event sources (eBPF, FileWatcher, DockerWatcher, Canary, StateCollector)
//! produce GuardEvent variants through a single enum.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// GuardEvent — unified event from any source
// ---------------------------------------------------------------------------

/// Where the event came from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    /// eBPF / ES kernel-level process interception
    Kernel,
    /// File system watcher (inotify/FSEvents)
    FileSystem,
    /// Docker daemon events
    Docker,
    /// Canary honeypot token triggered
    Canary,
    /// Periodic state collector drift
    StateCollector,
}

/// Severity classification for events
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Informational — log and continue
    Info,
    /// Low risk — auto-fix if rule exists
    Low,
    /// Medium risk — auto-fix + notify
    Medium,
    /// High risk — freeze + notify + wait for approval
    High,
    /// Critical — freeze + kill + emergency notify
    Critical,
}

/// What action to take for an event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionTier {
    /// Log only, no action
    Silent,
    /// Try auto-fix, continue if successful
    AutoFix,
    /// Freeze (killswitch) + notify + wait for human
    Escalate,
}

/// Unified event from any monitoring source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardEvent {
    /// Unique event ID
    pub id: String,
    /// Event source
    pub source: EventSource,
    /// Severity (set by classifier, not source)
    pub severity: Severity,
    /// Action tier (set by classifier)
    pub action: ActionTier,
    /// Timestamp (UTC nanos)
    pub timestamp_nanos: u64,
    /// Source-specific data
    pub detail: EventDetail,
}

// ---------------------------------------------------------------------------
// EventDetail — per-source payload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventDetail {
    /// Kernel caught a process (eBPF/ES)
    ProcessCaught {
        pid: u32,
        uid: u32,
        comm: String,
        args: String,
        /// Process is already SIGSTOP'd by kernel
        already_frozen: bool,
    },

    /// File system change detected
    FileChange {
        path: PathBuf,
        kind: String, // "create", "modify", "remove"
        /// Current file hash (None if deleted)
        current_hash: Option<String>,
        /// Baseline hash (None if untracked)
        baseline_hash: Option<String>,
    },

    /// Docker event
    DockerEvent {
        action: String,
        container_id: Option<String>,
        container_name: Option<String>,
        image: Option<String>,
    },

    /// Canary token triggered
    CanaryTriggered {
        token_path: String,
        accessor: String,
        accessor_uid: u32,
        access_type: String,
        risk: String,
    },

    /// State drift detected by collector
    StateDrift {
        component: String, // "packages", "services", "docker", "files"
        description: String,
        /// Key-value diff summary
        diff: HashMap<String, String>,
    },
}

impl GuardEvent {
    /// Create a new guard event with auto-generated ID and timestamp
    pub fn new(source: EventSource, detail: EventDetail) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            severity: Severity::Info, // will be set by classifier
            action: ActionTier::Silent,
            timestamp_nanos: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64,
            detail,
        }
    }

    /// Convenience: create a process-caught event from kernel
    pub fn process_caught(pid: u32, uid: u32, comm: String, args: String, already_frozen: bool) -> Self {
        Self::new(
            EventSource::Kernel,
            EventDetail::ProcessCaught {
                pid, uid, comm, args, already_frozen,
            },
        )
    }

    /// Convenience: create a file-change event
    pub fn file_change(path: PathBuf, kind: String, current_hash: Option<String>, baseline_hash: Option<String>) -> Self {
        Self::new(
            EventSource::FileSystem,
            EventDetail::FileChange { path, kind, current_hash, baseline_hash },
        )
    }

    /// Convenience: create a docker event
    pub fn docker_event(action: String, container_id: Option<String>, container_name: Option<String>, image: Option<String>) -> Self {
        Self::new(
            EventSource::Docker,
            EventDetail::DockerEvent { action, container_id, container_name, image },
        )
    }

    /// Convenience: create a canary trigger event
    pub fn canary_triggered(token_path: String, accessor: String, accessor_uid: u32, access_type: String, risk: String) -> Self {
        Self::new(
            EventSource::Canary,
            EventDetail::CanaryTriggered { token_path, accessor, accessor_uid, access_type, risk },
        )
    }

    /// Convenience: create a state drift event
    pub fn state_drift(component: String, description: String, diff: HashMap<String, String>) -> Self {
        Self::new(
            EventSource::StateCollector,
            EventDetail::StateDrift { component, description, diff },
        )
    }

    /// Get the affected path(s) for debouncing
    pub fn debounce_key(&self) -> String {
        match &self.detail {
            EventDetail::ProcessCaught { pid, .. } => format!("pid:{}", pid),
            EventDetail::FileChange { path, .. } => format!("file:{}", path.display()),
            EventDetail::DockerEvent { container_id, .. } => {
                format!("docker:{}", container_id.as_deref().unwrap_or("unknown"))
            }
            EventDetail::CanaryTriggered { token_path, .. } => format!("canary:{}", token_path),
            EventDetail::StateDrift { component, .. } => format!("state:{}", component),
        }
    }

    /// Get a short human-readable description
    pub fn summary(&self) -> String {
        match &self.detail {
            EventDetail::ProcessCaught { pid, comm, args, .. } => {
                format!("ProcessCaught pid={} {} {}", pid, comm, args)
            }
            EventDetail::FileChange { path, kind, .. } => {
                format!("FileChange {} {}", kind, path.display())
            }
            EventDetail::DockerEvent { action, container_name, .. } => {
                format!("DockerEvent {} {}", action, container_name.as_deref().unwrap_or("?"))
            }
            EventDetail::CanaryTriggered { token_path, accessor, .. } => {
                format!("CanaryTriggered {} by {}", token_path, accessor)
            }
            EventDetail::StateDrift { component, description, .. } => {
                format!("StateDrift[{}] {}", component, description)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Alert — sent to relay (fire-and-forget)
// ---------------------------------------------------------------------------

/// Alert sent to relay for human notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardAlert {
    /// Alert ID (same as event ID)
    pub id: String,
    /// Severity
    pub severity: Severity,
    /// Action taken
    pub action_taken: String,
    /// Event summary
    pub summary: String,
    /// Additional context (JSON-serializable)
    pub context: HashMap<String, serde_json::Value>,
    /// Timestamp ISO
    pub timestamp_iso: String,
}

impl GuardAlert {
    pub fn from_event(event: &GuardEvent, action_taken: &str) -> Self {
        let timestamp_iso = chrono::DateTime::from_timestamp(
            (event.timestamp_nanos / 1_000_000_000) as i64,
            (event.timestamp_nanos % 1_000_000_000) as u32,
        )
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        Self {
            id: event.id.clone(),
            severity: event.severity,
            action_taken: action_taken.to_string(),
            summary: event.summary(),
            context: HashMap::new(),
            timestamp_iso,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_event_process_caught() {
        let event = GuardEvent::process_caught(1234, 0, "rm".into(), "-rf /".into(), true);
        assert_eq!(event.source, EventSource::Kernel);
        assert_eq!(event.debounce_key(), "pid:1234");
        assert!(event.summary().contains("ProcessCaught"));
        assert!(event.summary().contains("1234"));
    }

    #[test]
    fn test_guard_event_file_change() {
        let event = GuardEvent::file_change(
            PathBuf::from("/etc/nginx/nginx.conf"),
            "modify".into(),
            Some("abc123".into()),
            Some("def456".into()),
        );
        assert_eq!(event.source, EventSource::FileSystem);
        assert_eq!(event.debounce_key(), "file:/etc/nginx/nginx.conf");
        assert!(event.summary().contains("modify"));
    }

    #[test]
    fn test_guard_event_docker() {
        let event = GuardEvent::docker_event(
            "start".into(),
            Some("abc123".into()),
            Some("mycontainer".into()),
            Some("nginx:latest".into()),
        );
        assert_eq!(event.source, EventSource::Docker);
        assert_eq!(event.debounce_key(), "docker:abc123");
    }

    #[test]
    fn test_guard_event_canary() {
        let event = GuardEvent::canary_triggered(
            "/etc/shadow.bak".into(), "hacker".into(), 1001, "read".into(), "high".into(),
        );
        assert_eq!(event.source, EventSource::Canary);
        assert!(event.summary().contains("hacker"));
    }

    #[test]
    fn test_guard_event_state_drift() {
        let mut diff = HashMap::new();
        diff.insert("nginx".into(), "stopped".into());
        let event = GuardEvent::state_drift("services".into(), "nginx stopped".into(), diff);
        assert_eq!(event.source, EventSource::StateCollector);
        assert_eq!(event.debounce_key(), "state:services");
    }

    #[test]
    fn test_guard_alert_from_event() {
        let event = GuardEvent::process_caught(42, 0, "cat".into(), "/etc/passwd".into(), false);
        let alert = GuardAlert::from_event(&event, "intercepted");
        assert_eq!(alert.id, event.id);
        assert_eq!(alert.action_taken, "intercepted");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }
}

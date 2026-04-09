//! GuardMode — lightweight local killswitch for ServerGuard
//!
//! Three modes:
//! - Running: normal operation, all events processed
//! - Paused: block all new user-triggered actions, auto-fix continues
//! - Emergency: block everything, including auto-fix
//!
//! Unlike agent::killswitch, this is independent and has no dependency
//! on the agent crate. It's a simple atomic state machine.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// Mode values: 0=Running, 1=Paused, 2=Emergency
const MODE_RUNNING: u8 = 0;
const MODE_PAUSED: u8 = 1;
const MODE_EMERGENCY: u8 = 2;

/// Guard operating mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardMode {
    /// Normal operation — process all events
    Running,
    /// Block new actions, continue auto-fix for known safe patterns
    Paused { reason: String },
    /// Block everything — no auto-fix, no processing
    Emergency { reason: String },
}

impl std::fmt::Display for GuardMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardMode::Running => write!(f, "Running"),
            GuardMode::Paused { reason } => write!(f, "Paused({})", reason),
            GuardMode::Emergency { reason } => write!(f, "Emergency({})", reason),
        }
    }
}

/// Lightweight killswitch for ServerGuard
///
/// Uses atomic operations for the fast path (is_paused, is_emergency).
/// The reason strings are stored separately in a Mutex.
pub struct GuardKillswitch {
    /// Atomic mode byte for lock-free checks
    mode: AtomicU8,
    /// Reason strings (only accessed when mode changes)
    state: parking_lot::RwLock<KillswitchState>,
    /// When the current mode was set
    mode_since: parking_lot::RwLock<Instant>,
}

#[derive(Debug, Clone)]
struct KillswitchState {
    pause_reason: String,
    emergency_reason: String,
    /// Auto-resume after this duration (None = no auto-resume)
    auto_resume_after: Option<Duration>,
    /// Whether an auto-resume task is scheduled
    auto_resume_scheduled: bool,
}

impl GuardKillswitch {
    /// Create a new killswitch in Running mode
    pub fn new() -> Self {
        Self {
            mode: AtomicU8::new(MODE_RUNNING),
            state: parking_lot::RwLock::new(KillswitchState {
                pause_reason: String::new(),
                emergency_reason: String::new(),
                auto_resume_after: None,
                auto_resume_scheduled: false,
            }),
            mode_since: parking_lot::RwLock::new(Instant::now()),
        }
    }

    /// Get current mode
    pub fn mode(&self) -> GuardMode {
        match self.mode.load(Ordering::Relaxed) {
            MODE_RUNNING => GuardMode::Running,
            MODE_PAUSED => {
                let state = self.state.read();
                GuardMode::Paused {
                    reason: state.pause_reason.clone(),
                }
            }
            MODE_EMERGENCY => {
                let state = self.state.read();
                GuardMode::Emergency {
                    reason: state.emergency_reason.clone(),
                }
            }
            _ => GuardMode::Running,
        }
    }

    /// Check if paused (fast path, no lock)
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.mode.load(Ordering::Relaxed) != MODE_RUNNING
    }

    /// Check if in emergency mode (fast path, no lock)
    #[inline]
    pub fn is_emergency(&self) -> bool {
        self.mode.load(Ordering::Relaxed) == MODE_EMERGENCY
    }

    /// Pause — block new actions, continue safe auto-fix
    pub fn pause(&self, reason: &str) {
        let prev = self.mode.swap(MODE_PAUSED, Ordering::SeqCst);
        if prev != MODE_PAUSED {
            let mut state = self.state.write();
            state.pause_reason = reason.to_string();
            *self.mode_since.write() = Instant::now();
            warn!("🛡 GuardKillswitch: PAUSED — {}", reason);
        }
    }

    /// Emergency — block everything
    pub fn emergency(&self, reason: &str) {
        self.mode.store(MODE_EMERGENCY, Ordering::SeqCst);
        let mut state = self.state.write();
        state.emergency_reason = reason.to_string();
        state.auto_resume_after = None;
        *self.mode_since.write() = Instant::now();
        warn!("🚨 GuardKillswitch: EMERGENCY — {}", reason);
    }

    /// Resume to normal operation
    pub fn resume(&self) {
        let prev = self.mode.swap(MODE_RUNNING, Ordering::SeqCst);
        if prev != MODE_RUNNING {
            let mut state = self.state.write();
            state.pause_reason.clear();
            state.emergency_reason.clear();
            state.auto_resume_after = None;
            state.auto_resume_scheduled = false;
            *self.mode_since.write() = Instant::now();
            info!("✅ GuardKillswitch: RESUMED");
        }
    }

    /// Pause with auto-resume after `duration`
    pub fn pause_with_timeout(&self, reason: &str, duration: Duration) {
        self.pause(reason);
        let mut state = self.state.write();
        state.auto_resume_after = Some(duration);
        state.auto_resume_scheduled = false;
    }

    /// Check if auto-resume is pending (call periodically from event loop)
    pub fn check_auto_resume(&self) -> bool {
        let state = self.state.read();
        if self.mode.load(Ordering::Relaxed) != MODE_PAUSED {
            return false;
        }
        if let Some(timeout) = state.auto_resume_after {
            let since = *self.mode_since.read();
            if since.elapsed() >= timeout {
                drop(state);
                info!("🛡 GuardKillswitch: auto-resuming after timeout");
                self.resume();
                return true;
            }
        }
        false
    }

    /// How long we've been in the current mode
    pub fn mode_duration(&self) -> Duration {
        self.mode_since.read().elapsed()
    }

    /// Status for reporting
    pub fn status(&self) -> KillswitchStatus {
        KillswitchStatus {
            mode: self.mode(),
            mode_duration_secs: self.mode_duration().as_secs(),
        }
    }
}

impl Default for GuardKillswitch {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable status for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillswitchStatus {
    pub mode: GuardMode,
    pub mode_duration_secs: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_new_is_running() {
        let ks = GuardKillswitch::new();
        assert_eq!(ks.mode(), GuardMode::Running);
        assert!(!ks.is_paused());
        assert!(!ks.is_emergency());
    }

    #[test]
    fn test_pause_resume() {
        let ks = GuardKillswitch::new();
        ks.pause("test reason");
        assert!(ks.is_paused());
        assert!(!ks.is_emergency());
        assert_eq!(ks.mode(), GuardMode::Paused { reason: "test reason".into() });

        ks.resume();
        assert!(!ks.is_paused());
        assert_eq!(ks.mode(), GuardMode::Running);
    }

    #[test]
    fn test_emergency() {
        let ks = GuardKillswitch::new();
        ks.emergency("critical alert");
        assert!(ks.is_paused());
        assert!(ks.is_emergency());
        assert_eq!(ks.mode(), GuardMode::Emergency { reason: "critical alert".into() });

        ks.resume();
        assert!(!ks.is_emergency());
    }

    #[test]
    fn test_pause_overrides_emergency() {
        let ks = GuardKillswitch::new();
        ks.emergency("e1");
        ks.pause("p1");
        assert!(!ks.is_emergency()); // pause overrides emergency
        assert!(ks.is_paused());
    }

    #[test]
    fn test_status() {
        let ks = GuardKillswitch::new();
        let status = ks.status();
        assert_eq!(status.mode, GuardMode::Running);
    }

    #[test]
    fn test_display() {
        let ks = GuardKillswitch::new();
        ks.pause("test");
        assert_eq!(ks.mode().to_string(), "Paused(test)");
    }
}

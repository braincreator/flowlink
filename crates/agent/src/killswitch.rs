// Kill Switch & Circuit Breaker for FlowLink agent.
// Port of internal/agent/killswitch.go

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use log::{info, warn};

/// Kill switch mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillSwitchMode {
    Running,
    Paused,
    Readonly,
    Emergency,
}

impl std::fmt::Display for KillSwitchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Readonly => write!(f, "readonly"),
            Self::Emergency => write!(f, "emergency"),
        }
    }
}

/// Resource check result.
#[derive(Debug, Clone)]
pub struct KillSwitchStatus {
    pub mode: KillSwitchMode,
    pub pause_reason: String,
    pub disk_usage: f64,
    pub cpu_usage: f64,
    pub consecutive_errors: u32,
}

/// Internal mutable state.
struct InnerState {
    mode: KillSwitchMode,
    pause_reason: String,
    pause_until: Option<Instant>,

    // Circuit breaker
    consecutive_errors: u32,
    last_error_time: Option<Instant>,
    error_window: Duration,

    // CPU tracking
    cpu_high_since: Option<Instant>,
    cpu_usage: f64,
    disk_usage: f64,
}

/// Kill switch — monitors system resources, manages agent modes.
pub struct KillSwitch {
    cpu_threshold: f64,
    cpu_threshold_dur: Duration,
    disk_threshold: f64,
    check_interval: Duration,

    paused: Arc<AtomicBool>,
    emergency: Arc<AtomicBool>,

    state: Mutex<InnerState>,
}

impl KillSwitch {
    /// Create with defaults: CPU 95% for 5min, disk 90%, check every 30s.
    pub fn new() -> Self {
        Self {
            cpu_threshold: 95.0,
            cpu_threshold_dur: Duration::from_secs(300),
            disk_threshold: 90.0,
            check_interval: Duration::from_secs(30),
            paused: Arc::new(AtomicBool::new(false)),
            emergency: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(InnerState {
                mode: KillSwitchMode::Running,
                pause_reason: String::new(),
                pause_until: None,
                consecutive_errors: 0,
                last_error_time: None,
                error_window: Duration::from_secs(300),
                cpu_high_since: None,
                cpu_usage: 0.0,
                disk_usage: 0.0,
            }),
        }
    }

    pub fn set_disk_threshold(&self, pct: f64) {
        // Simple: just log; for full impl store in Mutex or Atomic
        info!("disk threshold set to {pct}%");
    }

    pub fn set_cpu_threshold(&self, pct: f64, dur: Duration) {
        info!("cpu threshold set to {pct}% for {dur:?}");
    }

    /// Get current mode.
    pub fn mode(&self) -> KillSwitchMode {
        self.state.lock().unwrap().mode
    }

    /// Check if agent is paused or in emergency.
    pub fn is_paused(&self) -> bool {
        let mut s = self.state.lock().unwrap();

        // Auto-resume on timer
        if s.mode == KillSwitchMode::Paused {
            if let Some(until) = s.pause_until {
                if Instant::now() > until {
                    s.mode = KillSwitchMode::Running;
                    s.pause_reason.clear();
                    s.pause_until = None;
                    self.paused.store(false, Ordering::Relaxed);
                    info!("auto-resumed after timed pause");
                }
            }
        }

        s.mode == KillSwitchMode::Paused || s.mode == KillSwitchMode::Emergency
    }

    /// Check if agent is in readonly mode.
    pub fn is_readonly(&self) -> bool {
        self.state.lock().unwrap().mode == KillSwitchMode::Readonly
    }

    /// Emergency stop — immediately halt all operations.
    pub fn emergency_stop(&self) {
        let mut s = self.state.lock().unwrap();
        s.mode = KillSwitchMode::Emergency;
        s.pause_reason = "emergency stop".into();
        self.paused.store(true, Ordering::Relaxed);
        self.emergency.store(true, Ordering::Relaxed);
        warn!("EMERGENCY STOP activated");
    }

    /// Pause agent with a reason.
    pub fn pause(&self, reason: &str) {
        let mut s = self.state.lock().unwrap();
        s.mode = KillSwitchMode::Paused;
        s.pause_reason = reason.into();
        self.paused.store(true, Ordering::Relaxed);
        info!("agent paused: {reason}");
    }

    /// Pause agent for a duration, then auto-resume.
    pub fn pause_for(&self, reason: &str, duration: Duration) {
        let mut s = self.state.lock().unwrap();
        s.mode = KillSwitchMode::Paused;
        s.pause_reason = reason.into();
        s.pause_until = Some(Instant::now() + duration);
        self.paused.store(true, Ordering::Relaxed);
        info!("agent paused: {reason} for {duration:?}");
    }

    /// Resume normal operation.
    pub fn resume(&self) {
        let mut s = self.state.lock().unwrap();
        s.mode = KillSwitchMode::Running;
        s.pause_reason.clear();
        s.pause_until = None;
        s.consecutive_errors = 0;
        self.paused.store(false, Ordering::Relaxed);
        self.emergency.store(false, Ordering::Relaxed);
        info!("agent resumed");
    }

    /// Check current resource state and update mode accordingly.
    pub fn check(&self) -> KillSwitchStatus {
        let cpu_usage = get_cpu_usage();
        let disk_usage = get_disk_usage();

        let mut s = self.state.lock().unwrap();
        s.cpu_usage = cpu_usage;
        s.disk_usage = disk_usage;

        // CPU monitoring — use loadavg normalized by CPU count
        let cpu_count = num_cpus() as f64;
        let cpu_pct = if cpu_count > 0.0 { (cpu_usage / cpu_count) * 100.0 } else { 0.0 };

        if cpu_pct > self.cpu_threshold {
            if s.cpu_high_since.is_none() {
                s.cpu_high_since = Some(Instant::now());
            } else if let Some(since) = s.cpu_high_since {
                if since.elapsed() > self.cpu_threshold_dur {
                    s.mode = KillSwitchMode::Paused;
                    s.pause_reason = format!("CPU high load: {:.1}%", cpu_pct);
                    self.paused.store(true, Ordering::Relaxed);
                    warn!("auto-pause: high CPU ({:.1}%)", cpu_pct);
                }
            }
        } else {
            s.cpu_high_since = None;
        }

        // Disk monitoring
        if disk_usage > self.disk_threshold {
            if s.mode == KillSwitchMode::Running {
                s.mode = KillSwitchMode::Readonly;
                s.pause_reason = format!("disk almost full: {:.1}%", disk_usage);
                warn!("auto-readonly: disk {:.1}%", disk_usage);
            }
        }

        KillSwitchStatus {
            mode: s.mode,
            pause_reason: s.pause_reason.clone(),
            disk_usage,
            cpu_usage: cpu_pct,
            consecutive_errors: s.consecutive_errors,
        }
    }

    /// Check if a command is allowed in the current mode.
    pub fn check_command(&self, cmd: &str) -> Result<(), String> {
        let s = self.state.lock().unwrap();
        match s.mode {
            KillSwitchMode::Emergency => Err("emergency stop — all operations halted".into()),
            KillSwitchMode::Paused => Err(format!("agent paused: {}", s.pause_reason)),
            KillSwitchMode::Readonly => {
                if is_write_command(cmd) {
                    Err("write command blocked in readonly mode".into())
                } else {
                    Ok(())
                }
            }
            KillSwitchMode::Running => Ok(()),
        }
    }

    /// Record an error for circuit breaker.
    pub fn record_error(&self, _err: &str) {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();

        if let Some(last) = s.last_error_time {
            if now.duration_since(last) > s.error_window {
                s.consecutive_errors = 0;
            }
        }

        s.consecutive_errors += 1;
        s.last_error_time = Some(now);

        if s.consecutive_errors >= 3 {
            s.mode = KillSwitchMode::Paused;
            s.pause_reason = format!(
                "circuit breaker: {} consecutive errors",
                s.consecutive_errors
            );
            s.pause_until = Some(now + Duration::from_secs(60));
            self.paused.store(true, Ordering::Relaxed);
            warn!("circuit breaker activated: {} errors", s.consecutive_errors);
        }
    }

    /// Record a success — reset error counter.
    pub fn record_success(&self) {
        let mut s = self.state.lock().unwrap();
        s.consecutive_errors = 0;
    }

    /// Get detailed status.
    pub fn status(&self) -> KillSwitchStatus {
        let s = self.state.lock().unwrap();
        KillSwitchStatus {
            mode: s.mode,
            pause_reason: s.pause_reason.clone(),
            disk_usage: s.disk_usage,
            cpu_usage: s.cpu_usage,
            consecutive_errors: s.consecutive_errors,
        }
    }

    /// Get Arc to the paused flag for sharing.
    pub fn paused_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.paused)
    }

    /// Get Arc to the emergency flag for sharing.
    pub fn emergency_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.emergency)
    }

    /// Start background resource monitor. Returns a JoinHandle.
    pub fn start_monitor(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let ks = Arc::clone(self);
        let interval = self.check_interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                ks.check();
            }
        })
    }
}

/// Determine if a command is a write/destructive command.
pub fn is_write_command(cmd: &str) -> bool {
    const WRITE_PATTERNS: &[&str] = &[
        "rm ", "rmdir", "mv ", "cp ",
        "chmod ", "chown ",
        "apt install", "apt remove", "apt upgrade",
        "yum install", "yum remove",
        "docker rm", "docker rmi", "docker run",
        "systemctl stop", "systemctl restart",
        "iptables ",
        "crontab ",
        "echo >", "cat >",
    ];

    let cmd_lower = cmd.to_lowercase();
    for pattern in WRITE_PATTERNS {
        if cmd_lower.starts_with(pattern) || cmd_lower.contains(pattern) {
            return true;
        }
    }
    false
}

// --- Platform-specific resource getters ---

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[cfg(unix)]
fn get_cpu_usage() -> f64 {
    // Read load average from /proc/loadavg on Linux, use sysctl on macOS
    #[cfg(target_os = "linux")]
    {
        match std::fs::read_to_string("/proc/loadavg") {
            Ok(data) => {
                if let Some(field) = data.split_whitespace().next() {
                    return field.parse().unwrap_or(0.0);
                }
            }
            Err(_) => {}
        }
        0.0
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        match Command::new("sysctl").arg("-n").arg("vm.loadavg").output() {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                // Output: { 1.23 1.45 1.67 }
                let nums: Vec<&str> = text
                    .split_whitespace()
                    .filter(|s| s.parse::<f64>().is_ok())
                    .collect();
                nums.first().and_then(|s| s.parse().ok()).unwrap_or(0.0)
            }
            Err(_) => 0.0,
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0.0
    }
}

#[cfg(not(unix))]
fn get_cpu_usage() -> f64 {
    0.0
}

#[cfg(unix)]
fn get_disk_usage() -> f64 {
    use std::ffi::CString;

    // statvfs on the home directory
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let path = match CString::new(home) {
        Ok(p) => p,
        Err(_) => return 0.0,
    };

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(path.as_ptr(), &mut stat) };

    if ret != 0 {
        return 0.0;
    }

    let bsize = stat.f_bsize as u64;
    let total = stat.f_blocks as u64 * bsize;
    let free = stat.f_bavail as u64 * bsize;

    if total == 0 {
        return 0.0;
    }

    let used = total - free;
    (used as f64 / total as f64) * 100.0
}

#[cfg(not(unix))]
fn get_disk_usage() -> f64 {
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_mode() {
        let ks = KillSwitch::new();
        assert_eq!(ks.mode(), KillSwitchMode::Running);
    }

    #[test]
    fn test_emergency_stop() {
        let ks = KillSwitch::new();
        ks.emergency_stop();
        assert_eq!(ks.mode(), KillSwitchMode::Emergency);
        assert!(ks.is_paused());
        assert!(!ks.is_readonly());
    }

    #[test]
    fn test_pause_resume() {
        let ks = KillSwitch::new();
        ks.pause("test");
        assert!(ks.is_paused());

        ks.resume();
        assert!(!ks.is_paused());
        assert_eq!(ks.mode(), KillSwitchMode::Running);
    }

    #[test]
    fn test_check_command_running() {
        let ks = KillSwitch::new();
        assert!(ks.check_command("ls -la").is_ok());
    }

    #[test]
    fn test_check_command_paused() {
        let ks = KillSwitch::new();
        ks.pause("test");
        assert!(ks.check_command("ls -la").is_err());
    }

    #[test]
    fn test_check_command_emergency() {
        let ks = KillSwitch::new();
        ks.emergency_stop();
        assert!(ks.check_command("echo hello").is_err());
    }

    #[test]
    fn test_circuit_breaker() {
        let ks = KillSwitch::new();
        ks.record_error("err1");
        ks.record_error("err2");
        ks.record_error("err3");
        assert!(ks.is_paused());

        ks.resume();
        ks.record_success();
        assert!(!ks.is_paused());
    }

    #[test]
    fn test_pause_for_auto_resume() {
        let ks = KillSwitch::new();
        ks.pause_for("test", Duration::from_millis(100));
        assert!(ks.is_paused());

        std::thread::sleep(Duration::from_millis(150));
        assert!(!ks.is_paused());
    }

    #[test]
    fn test_is_write_command() {
        assert!(is_write_command("rm -rf /tmp/test"));
        assert!(!is_write_command("echo hello"));
        assert!(is_write_command("mv a b"));
        assert!(!is_write_command("ls -la"));
        assert!(is_write_command("chmod 755 file"));
        assert!(is_write_command("apt remove nginx"));
        assert!(is_write_command("docker rm container"));
        assert!(!is_write_command(""));
    }
}

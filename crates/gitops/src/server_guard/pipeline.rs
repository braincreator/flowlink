//! ServerGuard Pipeline — debounce, classify, decide, act
//!
//! Processes GuardEvents through:
//! 1. Debouncer (5s window, per-path)
//! 2. IgnoreFilter (.log, .git, cache, temp files)
//! 3. SelfChangeFilter (changes made by guard itself)
//! 4. Classifier (severity + action tier)
//! 5. Killswitch check
//! 6. Actuator dispatch

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, info, warn};

use super::command_runner::CommandRunner;
use super::event_types::{ActionTier, EventDetail, EventSource, GuardAlert, GuardEvent, Severity};
use super::guard_mode::GuardKillswitch;

// ---------------------------------------------------------------------------
// Pipeline config
// ---------------------------------------------------------------------------

/// Configuration for the ServerGuard pipeline
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PipelineConfig {
    /// Debounce window per event key (seconds)
    #[serde(default = "default_debounce_secs")]
    pub debounce_secs: u64,

    /// Self-change cooldown — ignore events on paths we just modified (seconds)
    #[serde(default = "default_self_change_cooldown")]
    pub self_change_cooldown_secs: u64,

    /// Maximum events per second before throttling
    #[serde(default = "default_max_events_per_sec")]
    pub max_events_per_sec: u32,

    /// Known safe paths that should be Low severity, not High
    #[serde(default)]
    pub safe_paths: Vec<String>,

    /// Known dangerous paths that should escalate to Critical
    #[serde(default = "default_dangerous_paths")]
    pub dangerous_paths: Vec<String>,

    /// Auto-fix rules — maps event pattern to remediation command
    #[serde(default)]
    pub auto_fix_rules: Vec<AutoFixPattern>,
}

fn default_debounce_secs() -> u64 { 5 }
fn default_self_change_cooldown() -> u64 { 10 }
fn default_max_events_per_sec() -> u32 { 100 }
fn default_dangerous_paths() -> Vec<String> {
    vec![
        "/root/.ssh/authorized_keys".into(),
        "/etc/shadow".into(),
        "/etc/passwd".into(),
        "/etc/sudoers".into(),
        "/etc/sudoers.d/".into(),
        "/etc/crontab".into(),
        "/etc/cron.d/".into(),
    ]
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            debounce_secs: default_debounce_secs(),
            self_change_cooldown_secs: default_self_change_cooldown(),
            max_events_per_sec: default_max_events_per_sec(),
            safe_paths: vec![],
            dangerous_paths: default_dangerous_paths(),
            auto_fix_rules: vec![],
        }
    }
}

/// Auto-fix pattern: when an event matches, run this command
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AutoFixPattern {
    /// Event source to match
    pub source: EventSource,
    /// Path prefix to match (for FileSystem events)
    pub path_prefix: Option<String>,
    /// Docker action to match (for Docker events)
    pub docker_action: Option<String>,
    /// Minimum severity to trigger
    pub min_severity: Option<Severity>,
    /// Command to run (binary + args)
    pub command: Vec<String>,
    /// Whether to notify after auto-fix
    #[serde(default = "default_true")]
    pub notify: bool,
}

fn default_true() -> bool { true }

// ---------------------------------------------------------------------------
// Debouncer
// ---------------------------------------------------------------------------

/// Per-key debouncer — holds events for a window before emitting
struct Debouncer {
    /// Pending events keyed by debounce_key()
    pending: HashMap<String, (GuardEvent, tokio::time::Instant)>,
    /// Debounce window
    window: Duration,
}

impl Debouncer {
    fn new(window: Duration) -> Self {
        Self {
            pending: HashMap::new(),
            window,
        }
    }

    /// Add an event. Returns Some(event) if the debounce window has expired
    /// for a previous event with the same key.
    fn push(&mut self, event: GuardEvent) -> Option<GuardEvent> {
        let key = event.debounce_key();
        let now = tokio::time::Instant::now();

        // Zero debounce = pass through immediately
        if self.window.is_zero() {
            return Some(event);
        }

        // Check if any pending events have expired
        let mut expired = Vec::new();
        for (k, (_, instant)) in &self.pending {
            if now.duration_since(*instant) >= self.window {
                expired.push(k.clone());
            }
        }

        let result = if let Some(k) = expired.into_iter().next() {
            self.pending.remove(&k)
        } else {
            None
        };

        // Upsert: keep the latest event for this key
        self.pending.insert(key, (event, now));

        result.map(|(e, _)| e)
    }

    /// Flush all pending events (for shutdown)
    fn flush(&mut self) -> Vec<GuardEvent> {
        self.pending.drain().map(|(_, (e, _))| e).collect()
    }
}

// ---------------------------------------------------------------------------
// SelfChangeFilter
// ---------------------------------------------------------------------------

/// Tracks paths that the guard itself has modified, to avoid infinite loops
struct SelfChangeFilter {
    /// Set of paths we recently modified, with expiry time
    changes: HashMap<String, tokio::time::Instant>,
    /// How long to ignore self-changes
    cooldown: Duration,
}

impl SelfChangeFilter {
    fn new(cooldown: Duration) -> Self {
        Self {
            changes: HashMap::new(),
            cooldown,
        }
    }

    /// Check if a path was recently modified by us
    fn is_self_change(&self, path: &str) -> bool {
        if let Some(instant) = self.changes.get(path) {
            tokio::time::Instant::now().duration_since(*instant) < self.cooldown
        } else {
            false
        }
    }

    /// Register a path as modified by us
    fn mark(&mut self, path: String) {
        self.changes.insert(path, tokio::time::Instant::now());
    }

    /// Clean up expired entries
    fn cleanup(&mut self) {
        let now = tokio::time::Instant::now();
        self.changes.retain(|_, instant| now.duration_since(*instant) < self.cooldown);
    }
}

// ---------------------------------------------------------------------------
// Classifier
// ---------------------------------------------------------------------------

/// Classifies events into severity and action tier
struct Classifier {
    safe_paths: Vec<String>,
    dangerous_paths: Vec<String>,
}

impl Classifier {
    fn new(safe_paths: Vec<String>, dangerous_paths: Vec<String>) -> Self {
        Self { safe_paths, dangerous_paths }
    }

    /// Classify a guard event — sets severity and action tier
    fn classify(&self, event: &mut GuardEvent) {
        // Take detail out temporarily to avoid borrow conflicts
        let detail = std::mem::replace(&mut event.detail, EventDetail::StateDrift {
            component: String::new(),
            description: String::new(),
            diff: std::collections::HashMap::new(),
        });

        match &detail {
            EventDetail::ProcessCaught { pid, uid, comm, args, already_frozen } => {
                self.classify_process(event, *pid, *uid, comm, args, *already_frozen);
            }
            EventDetail::FileChange { path, kind, current_hash, baseline_hash } => {
                self.classify_file(event, path, kind, current_hash.is_some(), baseline_hash.is_some());
            }
            EventDetail::DockerEvent { action, container_name, .. } => {
                self.classify_docker(event, action, container_name.as_deref().unwrap_or("unknown"));
            }
            EventDetail::CanaryTriggered { accessor_uid, risk, .. } => {
                self.classify_canary(event, *accessor_uid, risk);
            }
            EventDetail::StateDrift { component, .. } => {
                self.classify_state_drift(event, component);
            }
        }

        // Put detail back
        event.detail = detail;
    }

    fn classify_process(&self, event: &mut GuardEvent, _pid: u32, uid: u32, comm: &str, _args: &str, already_frozen: bool) {
        // Root running dangerous commands = Critical
        let dangerous_binaries = ["rm", "shred", "mkfs", "dd", "chmod", "chown", "iptables", "useradd", "userdel", "passwd"];

        let is_dangerous = dangerous_binaries.iter().any(|b| *b == comm);
        let is_root = uid == 0;

        if is_dangerous && is_root {
            event.severity = Severity::Critical;
            event.action = ActionTier::Escalate;
            return;
        }

        if is_dangerous {
            event.severity = Severity::High;
            event.action = ActionTier::Escalate;
            return;
        }

        // Non-root, non-dangerous: info
        event.severity = Severity::Low;
        event.action = if already_frozen { ActionTier::AutoFix } else { ActionTier::Silent };
    }

    fn classify_file(&self, event: &mut GuardEvent, path: &PathBuf, kind: &str, exists: bool, has_baseline: bool) {
        let path_str = path.to_string_lossy();

        // Check dangerous paths
        for dangerous in &self.dangerous_paths {
            if path_str.starts_with(dangerous.as_str()) || path_str.as_ref() == dangerous.as_str() {
                event.severity = Severity::Critical;
                event.action = ActionTier::Escalate;
                return;
            }
        }

        // Check safe paths
        for safe in &self.safe_paths {
            if path_str.starts_with(safe) {
                event.severity = Severity::Info;
                event.action = ActionTier::Silent;
                return;
            }
        }

        // No baseline = first time seeing this file
        if !has_baseline {
            event.severity = Severity::Info;
            event.action = ActionTier::Silent;
            return;
        }

        // Known config file modified
        let is_config = path_str.contains("/etc/") || path_str.contains("/nginx/") || path_str.contains("/docker/");
        if is_config && kind == "modify" {
            event.severity = Severity::Medium;
            event.action = ActionTier::AutoFix;
            return;
        }

        // File deleted
        if kind == "remove" && exists == false {
            event.severity = Severity::High;
            event.action = ActionTier::Escalate;
            return;
        }

        // Default
        event.severity = Severity::Low;
        event.action = ActionTier::AutoFix;
    }

    fn classify_docker(&self, event: &mut GuardEvent, action: &str, _container_name: &str) {
        match action {
            "die" | "kill" | "oom" => {
                event.severity = Severity::Medium;
                event.action = ActionTier::AutoFix;
            }
            "start" | "restart" | "create" => {
                // Unknown container starting = suspicious
                event.severity = Severity::Medium;
                event.action = ActionTier::Escalate;
            }
            "exec" | "attach" => {
                event.severity = Severity::High;
                event.action = ActionTier::Escalate;
            }
            _ => {
                event.severity = Severity::Info;
                event.action = ActionTier::Silent;
            }
        }
    }

    fn classify_canary(&self, event: &mut GuardEvent, accessor_uid: u32, risk: &str) {
        if accessor_uid == 0 {
            // Root accessing canary = expected, but log it
            event.severity = Severity::Low;
            event.action = ActionTier::Silent;
        } else {
            // Non-root accessing canary = intruder
            match risk {
                "high" | "critical" => {
                    event.severity = Severity::Critical;
                    event.action = ActionTier::Escalate;
                }
                "medium" => {
                    event.severity = Severity::High;
                    event.action = ActionTier::Escalate;
                }
                _ => {
                    event.severity = Severity::Medium;
                    event.action = ActionTier::Escalate;
                }
            }
        }
    }

    fn classify_state_drift(&self, event: &mut GuardEvent, component: &str) {
        match component {
            "packages" => {
                event.severity = Severity::Medium;
                event.action = ActionTier::AutoFix;
            }
            "services" => {
                event.severity = Severity::Medium;
                event.action = ActionTier::AutoFix;
            }
            "docker" => {
                event.severity = Severity::Low;
                event.action = ActionTier::AutoFix;
            }
            "files" => {
                event.severity = Severity::Medium;
                event.action = ActionTier::AutoFix;
            }
            _ => {
                event.severity = Severity::Info;
                event.action = ActionTier::Silent;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline — main entry point
// ---------------------------------------------------------------------------

/// The ServerGuard event pipeline
pub struct Pipeline {
    config: PipelineConfig,
    debouncer: Debouncer,
    self_change_filter: SelfChangeFilter,
    classifier: Classifier,
    killswitch: Arc<GuardKillswitch>,
    command_runner: Arc<CommandRunner>,
    /// Callback for sending alerts (fire-and-forget)
    alert_callback: Arc<dyn Fn(GuardAlert) + Send + Sync>,
}

impl Pipeline {
    pub fn new(
        config: PipelineConfig,
        killswitch: Arc<GuardKillswitch>,
        command_runner: Arc<CommandRunner>,
        alert_callback: Arc<dyn Fn(GuardAlert) + Send + Sync>,
    ) -> Self {
        let debouncer = Debouncer::new(Duration::from_secs(config.debounce_secs));
        let self_change_filter = SelfChangeFilter::new(
            Duration::from_secs(config.self_change_cooldown_secs),
        );
        let classifier = Classifier::new(
            config.safe_paths.clone(),
            config.dangerous_paths.clone(),
        );

        Self {
            config,
            debouncer,
            self_change_filter,
            classifier,
            killswitch,
            command_runner,
            alert_callback,
        }
    }

    /// Process an event through the full pipeline
    ///
    /// Returns the action taken (for logging/audit)
    pub async fn process(&mut self, event: GuardEvent) -> PipelineOutcome {
        // Step 0: Killswitch check
        if self.killswitch.is_emergency() {
            debug!("Pipeline: emergency mode, dropping event: {}", event.summary());
            return PipelineOutcome::Dropped { reason: "emergency mode".into() };
        }

        // Step 1: Debounce
        let event = match self.debouncer.push(event) {
            Some(emitted) => {
                debug!("Pipeline: debounced event emitted: {}", emitted.summary());
                emitted
            }
            None => {
                debug!("Pipeline: event debounced (waiting for window)");
                return PipelineOutcome::Debounced;
            }
        };

        // Step 2: Self-change filter
        if let EventDetail::FileChange { path, .. } = &event.detail {
            let path_str = path.to_string_lossy().to_string();
            if self.self_change_filter.is_self_change(&path_str) {
                debug!("Pipeline: self-change ignored: {}", path_str);
                return PipelineOutcome::Dropped { reason: "self-change".into() };
            }
        }

        // Step 3: Classify (sets severity + action on the event)
        let mut event = event;
        self.classifier.classify(&mut event);

        debug!(
            "Pipeline: classified {:?} → {:?} / {:?}",
            event.source, event.severity, event.action
        );

        // Step 4: Killswitch check for non-emergency
        if self.killswitch.is_paused() && event.action != ActionTier::Escalate {
            debug!("Pipeline: paused, dropping non-escalate event");
            return PipelineOutcome::Dropped { reason: "paused".into() };
        }

        // Step 5: Act
        let outcome = match event.action {
            ActionTier::Silent => {
                debug!("Pipeline: silent — logging only");
                PipelineOutcome::Logged
            }
            ActionTier::AutoFix => {
                self.handle_auto_fix(event).await
            }
            ActionTier::Escalate => {
                self.handle_escalate(event).await
            }
        };

        // Step 6: Cleanup expired self-change entries
        self.self_change_filter.cleanup();

        // Step 7: Check auto-resume
        self.killswitch.check_auto_resume();

        outcome
    }

    /// Handle auto-fix: try to remediate automatically
    async fn handle_auto_fix(&mut self, event: GuardEvent) -> PipelineOutcome {
        info!("🔧 Pipeline: auto-fix for {}", event.summary());

        // Try matching auto-fix patterns
        for rule in &self.config.auto_fix_rules {
            if self.matches_pattern(&event, rule) {
                let cmd = &rule.command;
                if cmd.is_empty() {
                    continue;
                }
                let binary = &cmd[0];
                let args: Vec<&str> = cmd[1..].iter().map(|s| s.as_str()).collect();

                let result = self.command_runner.run(binary, &args).await;
                if result.success {
                    info!(
                        "🔧 Pipeline: auto-fix SUCCESS for {} — {}",
                        event.summary(),
                        result.stdout.trim()
                    );

                    // Mark path as self-change to avoid re-trigger
                    if let EventDetail::FileChange { path, .. } = &event.detail {
                        self.self_change_filter.mark(path.to_string_lossy().to_string());
                    }

                    if rule.notify {
                        let alert = GuardAlert::from_event(&event, "auto-fixed");
                        (self.alert_callback)(alert);
                    }
                    return PipelineOutcome::AutoFixed {
                        command: cmd.join(" "),
                        success: true,
                    };
                } else {
                    warn!(
                        "🔧 Pipeline: auto-fix FAILED for {} — {}",
                        event.summary(),
                        result.stderr.trim()
                    );
                    // Auto-fix failed — escalate
                    let mut escalate_event = event;
                    escalate_event.action = ActionTier::Escalate;
                    return self.handle_escalate(escalate_event).await;
                }
            }
        }

        // No matching auto-fix rule — just log
        info!("Pipeline: no auto-fix rule for {}, logging", event.summary());
        PipelineOutcome::Logged
    }

    /// Handle escalate: freeze + notify + wait for human
    async fn handle_escalate(&mut self, event: GuardEvent) -> PipelineOutcome {
        warn!("🚨 Pipeline: ESCALATE — {}", event.summary());

        // Freeze dangerous processes
        if let EventDetail::ProcessCaught { pid, already_frozen, .. } = &event.detail {
            if !already_frozen {
                let result = self.command_runner.freeze_process(*pid).await;
                if result.success {
                    info!("🚨 Pipeline: froze process pid={}", pid);
                } else {
                    warn!("🚨 Pipeline: failed to freeze pid={} — {}", pid, result.stderr);
                }
            }
        }

        // Activate killswitch
        match event.severity {
            Severity::Critical => {
                self.killswitch.emergency(&event.summary());
            }
            Severity::High => {
                self.killswitch.pause_with_timeout(&event.summary(), Duration::from_secs(300)); // 5 min auto-resume
            }
            _ => {
                self.killswitch.pause(&event.summary());
            }
        }

        // Send alert
        let alert = GuardAlert::from_event(&event, "escalated");
        (self.alert_callback)(alert);

        PipelineOutcome::Escalated {
            severity: event.severity,
            killswitch_mode: self.killswitch.mode(),
        }
    }

    /// Check if an event matches an auto-fix pattern
    fn matches_pattern(&self, event: &GuardEvent, pattern: &AutoFixPattern) -> bool {
        if event.source != pattern.source {
            return false;
        }

        if let Some(min_sev) = &pattern.min_severity {
            if event.severity < *min_sev {
                return false;
            }
        }

        match &event.detail {
            EventDetail::FileChange { path, .. } => {
                if let Some(prefix) = &pattern.path_prefix {
                    let path_str = path.to_string_lossy();
                    if !path_str.starts_with(prefix) {
                        return false;
                    }
                }
                true
            }
            EventDetail::DockerEvent { action, .. } => {
                if let Some(da) = &pattern.docker_action {
                    action == da
                } else {
                    true
                }
            }
            _ => true,
        }
    }

    /// Flush all pending debounced events (for shutdown)
    pub fn flush(&mut self) -> Vec<GuardEvent> {
        self.debouncer.flush()
    }

    /// Get current killswitch status
    pub fn killswitch_status(&self) -> super::guard_mode::KillswitchStatus {
        self.killswitch.status()
    }
}

// ---------------------------------------------------------------------------
// PipelineOutcome — result of processing an event
// ---------------------------------------------------------------------------

/// What happened to an event
#[derive(Debug, Clone)]
pub enum PipelineOutcome {
    /// Event was debounced (waiting for more events in window)
    Debounced,
    /// Event was dropped (emergency/paused/self-change)
    Dropped { reason: String },
    /// Event was logged only (Silent tier)
    Logged,
    /// Event was auto-fixed
    AutoFixed { command: String, success: bool },
    /// Event was escalated to human
    Escalated { severity: Severity, killswitch_mode: super::guard_mode::GuardMode },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn noop_alert(_: GuardAlert) {}

    fn test_pipeline() -> Pipeline {
        let config = PipelineConfig {
            debounce_secs: 0, // no debounce in tests
            ..PipelineConfig::default()
        };
        let ks = Arc::new(GuardKillswitch::new());
        let cr = Arc::new(CommandRunner::new());
        let cb: Arc<dyn Fn(GuardAlert) + Send + Sync> = Arc::new(noop_alert);
        Pipeline::new(config, ks, cr, cb)
    }

    #[tokio::test]
    async fn test_process_caught_dangerous_root() {
        let mut pipeline = test_pipeline();
        let event = GuardEvent::process_caught(42, 0, "rm".into(), "-rf /".into(), true);

        let outcome = pipeline.process(event).await;
        match outcome {
            PipelineOutcome::Escalated { severity, .. } => {
                assert_eq!(severity, Severity::Critical);
            }
            _ => panic!("Expected Escalated, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_process_caught_safe() {
        let mut pipeline = test_pipeline();
        let event = GuardEvent::process_caught(42, 1000, "ls".into(), "/tmp".into(), false);

        let outcome = pipeline.process(event).await;
        // Should not escalate for non-dangerous commands
        assert!(!matches!(outcome, PipelineOutcome::Escalated { .. }));
    }

    #[tokio::test]
    async fn test_file_change_ssh_keys() {
        let mut pipeline = test_pipeline();
        let event = GuardEvent::file_change(
            PathBuf::from("/root/.ssh/authorized_keys"),
            "modify".into(),
            Some("abc".into()),
            Some("def".into()),
        );

        let outcome = pipeline.process(event).await;
        match outcome {
            PipelineOutcome::Escalated { severity, .. } => {
                assert_eq!(severity, Severity::Critical);
            }
            _ => panic!("Expected Escalated for SSH key change, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_docker_die_auto_fix() {
        let mut pipeline = test_pipeline();
        let event = GuardEvent::docker_event(
            "die".into(),
            Some("abc123".into()),
            Some("nginx".into()),
            Some("nginx:latest".into()),
        );

        let outcome = pipeline.process(event).await;
        // No auto-fix rule configured, so just logged
        assert!(matches!(outcome, PipelineOutcome::Logged | PipelineOutcome::Debounced));
    }

    #[tokio::test]
    async fn test_canary_non_root() {
        let mut pipeline = test_pipeline();
        let event = GuardEvent::canary_triggered(
            "/etc/shadow.bak".into(), "hacker".into(), 1001, "read".into(), "high".into(),
        );

        let outcome = pipeline.process(event).await;
        match outcome {
            PipelineOutcome::Escalated { severity, .. } => {
                assert_eq!(severity, Severity::Critical);
            }
            _ => panic!("Expected Escalated for canary trigger, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_emergency_drops_events() {
        let ks = Arc::new(GuardKillswitch::new());
        ks.emergency("test");
        let config = PipelineConfig::default();
        let cr = Arc::new(CommandRunner::new());
        let cb: Arc<dyn Fn(GuardAlert) + Send + Sync> = Arc::new(noop_alert);
        let mut pipeline = Pipeline::new(config, ks, cr, cb);

        let event = GuardEvent::process_caught(42, 0, "rm".into(), "-rf /".into(), true);
        let outcome = pipeline.process(event).await;
        assert!(matches!(outcome, PipelineOutcome::Dropped { .. }));
    }

    #[tokio::test]
    async fn test_self_change_filter() {
        let ks = Arc::new(GuardKillswitch::new());
        let config = PipelineConfig {
            self_change_cooldown_secs: 10,
            debounce_secs: 0,
            ..PipelineConfig::default()
        };
        let cr = Arc::new(CommandRunner::new());
        let cb: Arc<dyn Fn(GuardAlert) + Send + Sync> = Arc::new(noop_alert);
        let mut pipeline = Pipeline::new(config, ks, cr, cb);

        // Mark a path as self-change
        pipeline.self_change_filter.mark("/etc/nginx/nginx.conf".into());

        let event = GuardEvent::file_change(
            PathBuf::from("/etc/nginx/nginx.conf"),
            "modify".into(),
            Some("abc".into()),
            Some("def".into()),
        );

        let outcome = pipeline.process(event).await;
        assert!(matches!(outcome, PipelineOutcome::Dropped { reason } if reason == "self-change"));
    }

    #[test]
    fn test_debouncer_flush() {
        let mut debouncer = Debouncer::new(Duration::from_secs(5));
        let event = GuardEvent::process_caught(1, 0, "test".into(), "".into(), false);

        // Push and immediately flush
        debouncer.push(event.clone());
        let flushed = debouncer.flush();
        assert_eq!(flushed.len(), 1);
    }
}

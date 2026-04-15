//! ServerGuard — autonomous server protection system
//!
//! Runs locally on the host (not through relay). Combines:
//! - shield: eBPF/ES kernel-level process interception, signals, canary
//! - gitops: FileWatcher, DockerWatcher, StateCollector, auto_fix, backup
//! - local: CommandRunner, GuardKillswitch, Pipeline
//!
//! # Architecture
//!
//! ```text
//! EVENT SOURCES          PIPELINE              ACTUATORS
//! ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐
//! │ HybridGuard  │───→│ Debouncer    │───→│ sigstop/sigkill  │
//! │ (eBPF/ES)    │    │ IgnoreFilter │    │ CommandRunner    │
//! ├──────────────┤    │ Classifier   │    ├──────────────────┤
//! │ FileWatcher  │───→│ Killswitch   │───→│ auto_fix rules   │
//! ├──────────────┤    └──────────────┘    ├──────────────────┤
//! │ DockerWatch  │───→                  │ BackupEngine     │
//! ├──────────────┤                       ├──────────────────┤
//! │ CanaryWatch  │───→                  │ RelayClient      │
//! ├──────────────┤                       │ (alert fire&forget)
//! │ StateCollect │───→                  └──────────────────┘
//! └──────────────┘
//! ```
//!
//! # Lifecycle
//!
//! ```ignore
//! let guard = ServerGuard::new(config)?;
//! guard.start().await?;  // spawns background tasks
//! // ... events are processed automatically ...
//! guard.stop().await?;
//! ```

pub mod command_runner;
pub mod event_types;
pub mod guard_mode;
pub mod pipeline;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use command_runner::CommandRunner;
use event_types::{GuardAlert, GuardEvent};
use guard_mode::GuardKillswitch;
use pipeline::{Pipeline, PipelineConfig, PipelineOutcome};

/// ServerGuard configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerGuardConfig {
    /// Pipeline configuration (debounce, classify, auto-fix)
    #[serde(default)]
    pub pipeline: PipelineConfig,

    /// File system paths to watch
    #[serde(default = "default_watch_paths")]
    pub watch_paths: Vec<String>,

    /// Enable Docker event watching
    #[serde(default = "default_true")]
    pub watch_docker: bool,

    /// Enable canary token monitoring
    #[serde(default = "default_true")]
    pub watch_canary: bool,

    /// Enable periodic state collection
    #[serde(default = "default_true")]
    pub watch_state: bool,

    /// State collection interval (seconds)
    #[serde(default = "default_state_interval")]
    pub state_collect_interval_secs: u64,

    /// Relay URL for sending alerts (HTTP POST)
    pub relay_url: Option<String>,

    /// Agent ID for relay authentication
    pub agent_id: Option<String>,

    /// Agent token for relay authentication
    pub agent_token: Option<String>,
}

fn default_watch_paths() -> Vec<String> {
    vec![
        "/etc/nginx".into(),
        "/etc/docker".into(),
        "/etc/systemd".into(),
        "/etc/ssh".into(),
    ]
}

fn default_true() -> bool {
    true
}
fn default_state_interval() -> u64 {
    300
}

impl Default for ServerGuardConfig {
    fn default() -> Self {
        Self {
            pipeline: PipelineConfig::default(),
            watch_paths: default_watch_paths(),
            watch_docker: true,
            watch_canary: true,
            watch_state: true,
            state_collect_interval_secs: default_state_interval(),
            relay_url: None,
            agent_id: None,
            agent_token: None,
        }
    }
}

/// ServerGuard — the main orchestrator
pub struct ServerGuard {
    config: ServerGuardConfig,
    killswitch: Arc<GuardKillswitch>,
    command_runner: Arc<CommandRunner>,
    /// Channel for receiving events from all sources
    event_tx: mpsc::Sender<GuardEvent>,
    event_rx: Option<mpsc::Receiver<GuardEvent>>,
    /// Background task handles
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl ServerGuard {
    /// Create a new ServerGuard (not yet started)
    pub fn new(config: ServerGuardConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1024);

        Self {
            config,
            killswitch: Arc::new(GuardKillswitch::new()),
            command_runner: Arc::new(CommandRunner::new()),
            event_tx,
            event_rx: Some(event_rx),
            tasks: Vec::new(),
        }
    }

    /// Get a clone of the killswitch for external control
    pub fn killswitch(&self) -> Arc<GuardKillswitch> {
        self.killswitch.clone()
    }

    /// Get a clone of the command runner
    pub fn command_runner(&self) -> Arc<CommandRunner> {
        self.command_runner.clone()
    }

    /// Start all event sources and the processing pipeline
    ///
    /// Spawns background tasks for:
    /// - FileWatcher (inotify/FSEvents)
    /// - DockerEventWatcher (Docker events API)
    /// - Canary monitoring
    /// - State collection (periodic)
    /// - Pipeline event processor
    pub async fn start(&mut self) -> Result<()> {
        info!("🛡 ServerGuard starting...");

        // Take the receiver (can only be consumed once)
        let event_rx = self
            .event_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("ServerGuard already started"))?;

        let event_tx = self.event_tx.clone();
        let config = self.config.clone();
        let killswitch = self.killswitch.clone();
        let command_runner = self.command_runner.clone();

        // Build alert callback
        let relay_url = config.relay_url.clone();
        let agent_id = config.agent_id.clone();
        let agent_token = config.agent_token.clone();
        let alert_cb: Arc<dyn Fn(GuardAlert) + Send + Sync> = Arc::new(move |alert| {
            let relay_url = relay_url.clone();
            let agent_id = agent_id.clone();
            let agent_token = agent_token.clone();
            tokio::spawn(async move {
                if let Some(url) = relay_url {
                    send_alert_to_relay(&url, &agent_id, &agent_token, &alert).await;
                } else {
                    info!(
                        "🛡 ServerGuard alert (no relay configured): {}",
                        alert.summary
                    );
                }
            });
        });

        // Start file watcher
        if !self.config.watch_paths.is_empty() {
            let tx = event_tx.clone();
            let paths = self.config.watch_paths.clone();
            let handle = tokio::spawn(async move {
                start_file_watcher(tx, paths).await;
            });
            self.tasks.push(handle);
        }

        // Start docker watcher
        if self.config.watch_docker {
            let tx = event_tx.clone();
            let handle = tokio::spawn(async move {
                start_docker_watcher(tx).await;
            });
            self.tasks.push(handle);
        }

        // Start state collector
        if self.config.watch_state {
            let tx = event_tx.clone();
            let interval = self.config.state_collect_interval_secs;
            let handle = tokio::spawn(async move {
                start_state_collector(tx, interval).await;
            });
            self.tasks.push(handle);
        }

        // Start pipeline processor (main event loop)
        let mut pipeline = Pipeline::new(config.pipeline, killswitch, command_runner, alert_cb);
        let handle = tokio::spawn(async move {
            run_pipeline(event_rx, &mut pipeline).await;
        });
        self.tasks.push(handle);

        info!(
            "🛡 ServerGuard started with {} background tasks",
            self.tasks.len()
        );
        Ok(())
    }

    /// Stop all background tasks
    pub async fn stop(&mut self) {
        info!("🛡 ServerGuard stopping...");
        for task in self.tasks.drain(..) {
            task.abort();
        }
        info!("🛡 ServerGuard stopped");
    }

    /// Submit an event manually (e.g., from HybridGuard eBPF callback)
    pub async fn submit_event(&self, event: GuardEvent) {
        if self.event_tx.send(event).await.is_err() {
            warn!("ServerGuard: failed to submit event (channel closed)");
        }
    }

    /// Get current status
    pub fn status(&self) -> ServerGuardStatus {
        ServerGuardStatus {
            killswitch: self.killswitch.status(),
            tasks_running: self.tasks.len(),
        }
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerGuardStatus {
    pub killswitch: guard_mode::KillswitchStatus,
    pub tasks_running: usize,
}

// ---------------------------------------------------------------------------
// Event source tasks
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// File hash utility
// ---------------------------------------------------------------------------

fn file_sha256(path: &std::path::Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    Some(flowlink_crypto::sha256_hex(&data))
}

/// File watcher background task
async fn start_file_watcher(tx: mpsc::Sender<GuardEvent>, paths: Vec<String>) {
    use crate::drift::event_driven::FileWatcher;

    // Load baseline hashes for watched paths
    let mut baselines: HashMap<std::path::PathBuf, String> = HashMap::new();
    for p in &paths {
        let path = std::path::PathBuf::from(p);
        if path.is_file() {
            if let Some(hash) = file_sha256(&path) {
                baselines.insert(path.clone(), hash);
            }
        } else if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    if let Some(hash) = file_sha256(&entry.path()) {
                        baselines.insert(entry.path(), hash);
                    }
                }
            }
        }
    }
    info!("🛡 ServerGuard: loaded {} file baselines", baselines.len());

    let mut watcher = match FileWatcher::new(paths) {
        w => w,
    };

    if let Err(e) = watcher.start().await {
        warn!("ServerGuard: file watcher failed to start: {}", e);
        return;
    }

    info!(
        "🛡 ServerGuard: file watcher started on {} paths",
        watcher.watched_paths().len()
    );

    while let Some(event) = watcher.next_event().await {
        let current_hash = file_sha256(&event.path);
        let baseline_hash = baselines.get(&event.path).cloned();

        // Update baseline on create/modify
        if event.kind == "create" || event.kind == "modify" {
            if let Some(ref hash) = current_hash {
                baselines.insert(event.path.clone(), hash.clone());
            }
        }

        let guard_event =
            GuardEvent::file_change(event.path, event.kind, current_hash, baseline_hash);
        if tx.send(guard_event).await.is_err() {
            break;
        }
    }

    info!("🛡 ServerGuard: file watcher ended");
}

/// Docker event watcher background task
async fn start_docker_watcher(tx: mpsc::Sender<GuardEvent>) {
    use crate::drift::event_driven::DockerEventWatcher;

    let mut watcher = DockerEventWatcher::new();

    if let Err(e) = watcher.start().await {
        warn!("ServerGuard: docker watcher failed to start: {}", e);
        // Docker may not be available — this is not fatal
        return;
    }

    info!("🛡 ServerGuard: docker event watcher started");

    while let Some(event) = watcher.next_event().await {
        let guard_event = GuardEvent::docker_event(
            event.action,
            event.container_id,
            event.container_name,
            event.image,
        );
        if tx.send(guard_event).await.is_err() {
            break;
        }
    }

    info!("🛡 ServerGuard: docker event watcher ended");
}

/// State collector background task (periodic)
async fn start_state_collector(tx: mpsc::Sender<GuardEvent>, interval_secs: u64) {
    use tokio::time;

    let mut interval = time::interval(time::Duration::from_secs(interval_secs));

    info!(
        "🛡 ServerGuard: state collector started (interval: {}s)",
        interval_secs
    );

    loop {
        interval.tick().await;

        // Check services
        let result = tokio::process::Command::new("systemctl")
            .args([
                "list-units",
                "--type=service",
                "--state=failed",
                "--no-legend",
                "--no-pager",
            ])
            .output()
            .await;

        if let Ok(output) = result {
            if output.status.success() {
                let failed = String::from_utf8_lossy(&output.stdout);
                let failed_services: Vec<&str> =
                    failed.lines().filter(|l| !l.trim().is_empty()).collect();

                if !failed_services.is_empty() {
                    let mut diff = std::collections::HashMap::new();
                    for svc in &failed_services {
                        // Parse "nginx.service loaded failed failed" → take first field
                        let name = svc.split_whitespace().next().unwrap_or(svc);
                        diff.insert(name.to_string(), "failed".to_string());
                    }

                    let event = GuardEvent::state_drift(
                        "services".into(),
                        format!("{} services failed", failed_services.len()),
                        diff,
                    );
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    info!("🛡 ServerGuard: state collector ended");
}

// ---------------------------------------------------------------------------
// Pipeline processor (main event loop)
// ---------------------------------------------------------------------------

async fn run_pipeline(mut rx: mpsc::Receiver<GuardEvent>, pipeline: &mut Pipeline) {
    info!("🛡 ServerGuard: pipeline processor started");

    while let Some(event) = rx.recv().await {
        let outcome = pipeline.process(event).await;

        match &outcome {
            PipelineOutcome::Escalated {
                severity,
                killswitch_mode,
            } => {
                warn!(
                    "🚨 ServerGuard: escalated event (severity={:?}, mode={:?})",
                    severity, killswitch_mode
                );
            }
            PipelineOutcome::AutoFixed {
                command,
                success: true,
            } => {
                info!("🔧 ServerGuard: auto-fixed via: {}", command);
            }
            PipelineOutcome::AutoFixed {
                command,
                success: false,
            } => {
                warn!("⚠️ ServerGuard: auto-fix failed via: {}", command);
            }
            PipelineOutcome::Dropped { reason } => {
                debug!("ServerGuard: dropped event: {}", reason);
            }
            _ => {}
        }
    }

    info!("🛡 ServerGuard: pipeline processor ended");
}

// ---------------------------------------------------------------------------
// Alert sender (fire-and-forget HTTP POST to relay)
// ---------------------------------------------------------------------------

async fn send_alert_to_relay(
    relay_url: &str,
    agent_id: &Option<String>,
    agent_token: &Option<String>,
    alert: &GuardAlert,
) {
    use reqwest::Client;

    let client = match Client::new()
        .post(format!(
            "{}/api/shield/ingest",
            relay_url.trim_end_matches('/')
        ))
        .header("Content-Type", "application/json")
        .json(&alert)
    {
        req => req,
    };

    // Add auth headers if available
    let client = if let (Some(id), Some(token)) = (agent_id, agent_token) {
        client
            .header("X-Agent-ID", id)
            .header("X-Agent-Token", token)
    } else {
        client
    };

    match client.send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                info!("🛡 ServerGuard: alert sent to relay ({})", alert.id);
            } else {
                warn!("🛡 ServerGuard: relay returned {}", resp.status());
            }
        }
        Err(e) => {
            warn!("🛡 ServerGuard: failed to send alert to relay: {}", e);
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
    fn test_default_config() {
        let config = ServerGuardConfig::default();
        assert!(!config.watch_paths.is_empty());
        assert!(config.watch_docker);
        assert!(config.watch_state);
    }

    #[test]
    fn test_create_guard() {
        let guard = ServerGuard::new(ServerGuardConfig::default());
        assert!(!guard.killswitch().is_paused());
        assert_eq!(guard.status().tasks_running, 0);
    }

    #[tokio::test]
    async fn test_submit_event_before_start() {
        let guard = ServerGuard::new(ServerGuardConfig::default());
        let event = GuardEvent::process_caught(1, 0, "test".into(), "".into(), false);
        // Should be able to submit before start (buffered in channel)
        guard.submit_event(event).await;
    }

    #[tokio::test]
    async fn test_start_stop() {
        let mut guard = ServerGuard::new(ServerGuardConfig {
            watch_paths: vec![], // no file watcher (needs real paths)
            watch_docker: false, // no docker (may not be available)
            watch_state: false,  // no state collector
            ..ServerGuardConfig::default()
        });

        guard.start().await.unwrap();
        assert!(guard.status().tasks_running >= 1);
        guard.stop().await;
        assert_eq!(guard.status().tasks_running, 0);
    }
}

// Config Hot Reload — watches relay config file and broadcasts updates to connected agents.
//
// Features:
// - File watcher via `notify` (auto-reload on save)
// - Debounce (500ms) to avoid duplicate reloads from editors
// - Manual reload via API endpoint
// - Broadcast ConfigUpdate to all connected agents via WebSocket
// - Atomic swap with RwLock for zero-downtime config access
// - Prometheus metrics integration

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::{info, warn, error};
use notify::{Event, EventKind, RecursiveMode, Watcher as NotifyWatcher};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};

use crate::handler::RelayHandler;
use crate::metrics::Metrics;
use flowlink_core::{config::RelayConfig, Message, MessageType, Priority};

/// Channel event from the file watcher.
#[derive(Debug)]
enum WatchEvent {
    /// Config file changed on disk.
    Changed,
    /// Watcher encountered an error.
    Error(String),
    /// Shutdown signal.
    Shutdown,
}

/// Hot-reload manager for relay configuration.
pub struct ConfigReloader {
    config_path: PathBuf,
    config: Arc<RwLock<RelayConfig>>,
    handler: Arc<RelayHandler>,
    metrics: Arc<Metrics>,
    /// Reload count for this session.
    reload_count: Arc<std::sync::atomic::AtomicU64>,
}

/// Response returned by reload operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadResult {
    pub ok: bool,
    pub message: String,
    pub reload_count: u64,
    pub timestamp: i64,
    pub connected_agents: usize,
}

/// Response for config push to agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    pub ok: bool,
    pub message: String,
    pub pushed_to: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub timestamp: i64,
}

impl ConfigReloader {
    /// Create a new config reloader.
    pub fn new(
        config_path: PathBuf,
        config: Arc<RwLock<RelayConfig>>,
        handler: Arc<RelayHandler>,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            config_path,
            config,
            handler,
            metrics,
            reload_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Start the file watcher task. Returns a shutdown handle.
    /// Spawns a background tokio task that watches the config file.
    pub fn start_watcher(self: Arc<Self>) -> Result<WatcherHandle> {
        let (tx, mut rx) = mpsc::channel::<WatchEvent>(16);
        let config_path = self.config_path.clone();

        // Create the notify watcher (runs on a dedicated thread)
        let tx_for_watcher = tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            let event = match res {
                Ok(e) => {
                    match e.kind {
                        EventKind::Create(_)
                        | EventKind::Modify(_)
                        | EventKind::Remove(_) => WatchEvent::Changed,
                        _ => return,
                    }
                }
                Err(e) => WatchEvent::Error(e.to_string()),
            };
            // Blocking send — if channel full, skip (debounce handles this)
            let _ = tx_for_watcher.blocking_send(event);
        })
        .context("failed to create file watcher for config")?;

        watcher.watch(&config_path, RecursiveMode::NonRecursive)
            .context("failed to watch config file")?;

        let reloader = self.clone();
        let _watcher = watcher; // Keep alive

        // Spawn the debounce + reload task
        tokio::spawn(async move {
            let mut pending = false;
            let debounce = tokio::time::Duration::from_millis(500);

            loop {
                tokio::select! {
                    event = rx.recv() => {
                        match event {
                            Some(WatchEvent::Changed) => {
                                if !pending {
                                    pending = true;
                                    // Debounce: wait before reloading
                                    let reloader_clone = reloader.clone();
                                    tokio::spawn(async move {
                                        tokio::time::sleep(debounce).await;
                                        if let Err(e) = reloader_clone.reload().await {
                                            warn!("Config auto-reload failed: {e}");
                                        }
                                    });
                                }
                            }
                            Some(WatchEvent::Error(e)) => {
                                error!("Config watcher error: {e}");
                            }
                            Some(WatchEvent::Shutdown) | None => {
                                info!("Config watcher shutting down");
                                break;
                            }
                        }
                    }
                }
            }
            drop(_watcher);
        });

        Ok(WatcherHandle {
            shutdown_tx: tx,
        })
    }

    /// Reload the config from disk and broadcast to all connected agents.
    pub async fn reload(&self) -> Result<ReloadResult> {
        let mut new_config = RelayConfig::load(self.config_path.to_str().unwrap())
            .context("failed to load config from disk")?;
        new_config.apply_env_overrides();

        let reload_num = self.reload_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        info!("Config reload #{reload_num}: reloading from {}", self.config_path.display());

        // Atomic swap
        {
            let mut cfg = self.config.write().await;
            *cfg = new_config.clone();
        }

        // Update metrics
        self.metrics.config_reload_total.with_label_values(&[]).inc();

        // Broadcast to all connected agents
        let push_result = self.push_to_all_agents(&new_config).await;

        let timestamp = chrono::Utc::now().timestamp();
        info!(
            "Config reload #{reload_num} complete: pushed to {} agents, {} failed",
            push_result.pushed_to.len(),
            push_result.failed.len(),
        );

        Ok(ReloadResult {
            ok: true,
            message: format!("Config reloaded (#{reload_num})"),
            reload_count: reload_num,
            timestamp,
            connected_agents: push_result.pushed_to.len() + push_result.failed.len(),
        })
    }

    /// Get current config (read-only snapshot).
    pub async fn get_config(&self) -> RelayConfig {
        self.config.read().await.clone()
    }

    /// Get reload count.
    pub fn reload_count(&self) -> u64 {
        self.reload_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Push config update to all connected agents via WebSocket.
    async fn push_to_all_agents(&self, config: &RelayConfig) -> PushResult {
        let agents = self.handler.connected_agents();
        let mut pushed_to = Vec::new();
        let mut failed = Vec::new();

        // Build the config update payload with relevant fields
        let payload = serde_json::json!({
            "relay_url": format!("wss://{}", config.wss_addr),
            "http_addr": config.http_addr.to_string(),
            "client_name": config.client_name,
            "llm_enabled": config.llm.enabled,
            "billing_enabled": config.billing.enabled,
        });

        for agent_id in agents {
            let msg = Message::new(MessageType::ConfigUpdate)
                .with_agent_id(&agent_id)
                .with_priority(Priority::System) // Bypass safety checks
                .with_payload(&payload);

            match self.handler.send_to_agent(&agent_id, msg).await {
                Ok(()) => pushed_to.push(agent_id),
                Err(e) => failed.push((agent_id, e.to_string())),
            }
        }

        PushResult {
            ok: failed.is_empty(),
            message: if failed.is_empty() {
                format!("Pushed to {} agents", pushed_to.len())
            } else {
                format!("Pushed to {}, {} failed", pushed_to.len(), failed.len())
            },
            pushed_to,
            failed,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// Push config to a specific agent.
    pub async fn push_to_agent(&self, agent_id: &str) -> Result<PushResult> {
        let config = self.config.read().await.clone();
        let payload = serde_json::json!({
            "relay_url": format!("wss://{}", config.wss_addr),
            "http_addr": config.http_addr.to_string(),
            "client_name": config.client_name,
            "llm_enabled": config.llm.enabled,
            "billing_enabled": config.billing.enabled,
        });

        let msg = Message::new(MessageType::ConfigUpdate)
            .with_agent_id(agent_id)
            .with_priority(Priority::System)
            .with_payload(&payload);

        match self.handler.send_to_agent(agent_id, msg).await {
            Ok(()) => Ok(PushResult {
                ok: true,
                message: format!("Config pushed to {agent_id}"),
                pushed_to: vec![agent_id.to_string()],
                failed: vec![],
                timestamp: chrono::Utc::now().timestamp(),
            }),
            Err(e) => Ok(PushResult {
                ok: false,
                message: format!("Failed to push to {agent_id}: {e}"),
                pushed_to: vec![],
                failed: vec![(agent_id.to_string(), e.to_string())],
                timestamp: chrono::Utc::now().timestamp(),
            }),
        }
    }
}

/// Handle to shut down the file watcher.
pub struct WatcherHandle {
    shutdown_tx: mpsc::Sender<WatchEvent>,
}

impl WatcherHandle {
    /// Signal the watcher task to stop.
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.blocking_send(WatchEvent::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::RelayHandler;
    use crate::auth::AuthManager;
    use crate::eventbus::EventBus;
    use crate::approval::ApprovalQueue;
    use crate::pool::AgentPool;
    use std::sync::Arc;

    fn test_config_json() -> String {
        r#"{
            "api_token": "test-token",
            "http_addr": "0.0.0.0:9090",
            "wss_addr": "0.0.0.0:9443",
            "client_name": "Test Relay"
        }"#.to_string()
    }

    fn test_reloader() -> (Arc<ConfigReloader>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, test_config_json()).unwrap();

        let config = RelayConfig::load(config_path.to_str().unwrap()).unwrap();
        let config = Arc::new(RwLock::new(config));
        let handler = Arc::new(RelayHandler::new(
            Arc::new(AgentPool::new()),
            Arc::new(AuthManager::new(None)),
            Arc::new(EventBus::new()),
            Arc::new(ApprovalQueue::new()),
        ));
        let metrics = Arc::new(Metrics::new());

        let reloader = Arc::new(ConfigReloader::new(config_path, config, handler, metrics));
        (reloader, dir)
    }

    #[tokio::test]
    async fn test_reload_from_disk() {
        let (reloader, _dir) = test_reloader();

        let result = reloader.reload().await.unwrap();
        assert!(result.ok);
        assert_eq!(result.reload_count, 1);
        assert_eq!(reloader.reload_count(), 1);
    }

    #[tokio::test]
    async fn test_get_config() {
        let (reloader, _dir) = test_reloader();
        let config = reloader.get_config().await;
        assert_eq!(config.api_token, "test-token");
        assert_eq!(config.http_addr.to_string(), "0.0.0.0:9090");
    }

    #[tokio::test]
    async fn test_reload_updates_config() {
        let (reloader, dir) = test_reloader();
        let config_path = dir.path().join("config.json");

        // Modify config on disk
        let new_config = r#"{
            "api_token": "new-token",
            "http_addr": "0.0.0.0:7070",
            "client_name": "Updated Relay"
        }"#;
        std::fs::write(&config_path, new_config).unwrap();

        // Reload
        reloader.reload().await.unwrap();

        // Verify new config
        let config = reloader.get_config().await;
        assert_eq!(config.api_token, "new-token");
        assert_eq!(config.http_addr.to_string(), "0.0.0.0:7070");
        assert_eq!(config.client_name, "Updated Relay");
    }

    #[tokio::test]
    async fn test_reload_invalid_config() {
        let (reloader, dir) = test_reloader();
        let config_path = dir.path().join("config.json");

        // Write invalid JSON
        std::fs::write(&config_path, "not json at all").unwrap();

        let result = reloader.reload().await;
        assert!(result.is_err());
        // Original config should still be intact
        let config = reloader.get_config().await;
        assert_eq!(config.api_token, "test-token");
    }

    #[tokio::test]
    async fn test_reload_multiple_times() {
        let (reloader, _dir) = test_reloader();

        for i in 1..=5 {
            let result = reloader.reload().await.unwrap();
            assert_eq!(result.reload_count, i);
        }
        assert_eq!(reloader.reload_count(), 5);
    }

    #[tokio::test]
    async fn test_push_to_nonexistent_agent() {
        let (reloader, _dir) = test_reloader();
        let result = reloader.push_to_agent("ghost-agent").await.unwrap();
        assert!(!result.ok);
        assert!(result.pushed_to.is_empty());
        assert_eq!(result.failed.len(), 1);
    }

    #[tokio::test]
    async fn test_reload_result_serialization() {
        let result = ReloadResult {
            ok: true,
            message: "Config reloaded (#1)".into(),
            reload_count: 1,
            timestamp: 1000,
            connected_agents: 3,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: ReloadResult = serde_json::from_str(&json).unwrap();
        assert!(back.ok);
        assert_eq!(back.reload_count, 1);
    }

    #[tokio::test]
    async fn test_push_result_serialization() {
        let result = PushResult {
            ok: true,
            message: "Pushed to 2 agents".into(),
            pushed_to: vec!["a1".into(), "a2".into()],
            failed: vec![],
            timestamp: 1000,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: PushResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pushed_to.len(), 2);
    }
}

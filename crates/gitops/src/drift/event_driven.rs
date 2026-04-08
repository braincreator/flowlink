//! Event-driven drift detection using file system watchers and Docker event streams.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{Context, Result};
use bollard::{Docker, system::EventsOptions};
use futures_util::StreamExt;
use notify::{Event, EventKind, RecursiveMode, Watcher as NotifyWatcher};
use tokio::sync::mpsc;
use tracing;

// ---------------------------------------------------------------------------
// FileWatcher
// ---------------------------------------------------------------------------

/// A file system event produced by `FileWatcher`.
#[derive(Debug, Clone)]
pub struct FileWatchEvent {
    /// The path that changed.
    pub path: PathBuf,
    /// Kind of change: `"create"`, `"modify"`, `"remove"`, or `"other"`.
    pub kind: String,
}

/// Watches the local file system for changes using `notify`.
///
/// # Lifecycle
///
/// ```ignore
/// let mut watcher = FileWatcher::new(vec!["/tmp/deploy".into()]);
/// watcher.start().await?;
/// while let Some(event) = watcher.next_event().await {
///     tracing::info!(?event, "file changed");
/// }
/// ```
pub struct FileWatcher {
    /// Paths being watched.
    watched_paths: Vec<String>,
    /// The underlying notify watcher (kept alive while watching).
    watcher: Option<notify::RecommendedWatcher>,
    /// Receiver for forwarded events.
    event_rx: Option<mpsc::Receiver<FileWatchEvent>>,
    /// Handle for the bridge task.
    task: Option<tokio::task::JoinHandle<()>>,
}

impl FileWatcher {
    /// Create a new `FileWatcher` that will watch `paths` recursively.
    pub fn new(paths: Vec<String>) -> Self {
        Self {
            watched_paths: paths,
            watcher: None,
            event_rx: None,
            task: None,
        }
    }

    /// Paths currently configured for watching.
    pub fn watched_paths(&self) -> &[String] {
        &self.watched_paths
    }

    /// Start watching. Consumes and replaces any previous state.
    ///
    /// Internally this creates a `notify::RecommendedWatcher`, registers all
    /// configured paths, and spawns a bridging tokio task that forwards
    /// filesystem events through a `tokio::sync::mpsc` channel.
    pub async fn start(&mut self) -> Result<()> {
        // Tear down any previous watcher first.
        self.stop();

        let (tx, rx) = mpsc::channel(256);
        self.event_rx = Some(rx);

        // Channel that the notify callback (running on a notify worker thread)
        // uses to deliver raw events to our bridging task.
        let (raw_tx, raw_rx) = std::sync::mpsc::channel::<Event>();

        // Wrap in Arc<Mutex<>> so the closure satisfies the Send + Sync bound
        // required by notify 7's RecommendedWatcher.
        let raw_tx = Arc::new(Mutex::new(raw_tx));

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let sender = raw_tx.lock().unwrap_or_else(|e| e.into_inner());
                let _ = sender.send(event);
            }
        })
        .context("failed to create notify watcher")?;

        // Register every path recursively.
        for path in &self.watched_paths {
            let pb = PathBuf::from(path);
            if pb.exists() {
                watcher
                    .watch(&pb, RecursiveMode::Recursive)
                    .with_context(|| format!("failed to watch path: {}", path))?;
                tracing::info!(path = %path, "watching path");
            } else {
                tracing::warn!(path = %path, "watch path does not exist, skipping");
            }
        }

        self.watcher = Some(watcher);

        // Spawn a blocking task that reads from the std channel (notify delivers
        // events on its own thread via a sync channel, so recv() would otherwise
        // block the async runtime).
        let tx_block = tx.clone();
        let bridge = tokio::task::spawn_blocking(move || {
            loop {
                match raw_rx.recv() {
                    Ok(event) => {
                        for path in &event.paths {
                            let kind = classify_event_kind(&event.kind);
                            let file_event = FileWatchEvent {
                                path: path.clone(),
                                kind: kind.to_owned(),
                            };
                            // Use blocking_send inside spawn_blocking — we're already off the async runtime.
                            if tx_block.blocking_send(file_event).is_err() {
                                tracing::info!("FileWatcher receiver dropped, stopping bridge");
                                return;
                            }
                        }
                    }
                    Err(_) => {
                        tracing::info!("FileWatcher raw channel closed, stopping bridge");
                        return;
                    }
                }
            }
        });

        // Wrap the blocking handle so it looks like a regular JoinHandle.
        self.task = Some(bridge);
        tracing::info!(
            paths = self.watched_paths.len(),
            "FileWatcher started"
        );
        Ok(())
    }

    /// Stop watching and cancel the bridge task.
    pub fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.watcher.take();
        self.event_rx.take();
        tracing::info!("FileWatcher stopped");
    }

    /// Receive the next file system event. Returns `None` when the watcher
    /// has been stopped or the channel is closed.
    pub async fn next_event(&mut self) -> Option<FileWatchEvent> {
        self.event_rx.as_mut()?.recv().await
    }

    /// Returns a borrowed receiver so callers can use `select!` or iterate
    /// directly.
    pub fn receiver(&mut self) -> Option<&mut mpsc::Receiver<FileWatchEvent>> {
        self.event_rx.as_mut()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

fn classify_event_kind(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "remove",
        EventKind::Access(_) => "access",
        _ => "other",
    }
}

// ---------------------------------------------------------------------------
// DockerEventWatcher
// ---------------------------------------------------------------------------

/// Wrapper around a Docker event from the daemon.
#[derive(Debug, Clone)]
pub struct DockerWatchEvent {
    /// The Docker action string (e.g. `"start"`, `"die"`, `"create"`).
    pub action: String,
    /// Container ID (if applicable).
    pub container_id: Option<String>,
    /// Container name (if applicable).
    pub container_name: Option<String>,
    /// Image name (if applicable).
    pub image: Option<String>,
    /// Full JSON string of the raw event for extensibility.
    pub raw_json: String,
}

/// Streams events from the local Docker daemon.
///
/// # Lifecycle
///
/// ```ignore
/// let mut watcher = DockerEventWatcher::new();
/// watcher.start().await?;
/// while let Some(event) = watcher.next_event().await {
///     tracing::info!(?event, "docker event");
/// }
/// ```
pub struct DockerEventWatcher {
    /// Receiver for forwarded Docker events.
    event_rx: Option<mpsc::Receiver<DockerWatchEvent>>,
    /// Handle for the stream-reading task.
    task: Option<tokio::task::JoinHandle<()>>,
}

impl DockerEventWatcher {
    /// Create a new `DockerEventWatcher` (not yet started).
    pub fn new() -> Self {
        Self {
            event_rx: None,
            task: None,
        }
    }

    /// Connect to the Docker daemon and start streaming events.
    ///
    /// Connects via the Unix socket at `/var/run/docker.sock`.
    pub async fn start(&mut self) -> Result<()> {
        self.stop();

        let (tx, rx) = mpsc::channel(256);
        self.event_rx = Some(rx);

        let docker = Docker::connect_with_unix_defaults()
            .context("failed to connect to Docker daemon via unix:///var/run/docker.sock")?;

        let task = tokio::spawn(async move {
            let mut stream = docker.events::<String>(None);

            tracing::info!("DockerEventWatcher connected, listening for events");

            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => {
                        let container_id = event.actor.as_ref().and_then(|a| a.id.clone());
                        let container_name = event.actor.as_ref().and_then(|a| {
                            a.attributes
                                .as_ref()
                                .and_then(|attrs: &std::collections::HashMap<String, String>| attrs.get("name").cloned())
                        });
                        let image = event.actor.as_ref().and_then(|a| {
                            a.attributes
                                .as_ref()
                                .and_then(|attrs: &std::collections::HashMap<String, String>| attrs.get("image").cloned())
                        });

                        let docker_event = DockerWatchEvent {
                            action: event.action.clone().unwrap_or_default(),
                            container_id,
                            container_name,
                            image,
                            raw_json: serde_json::to_string(&event).unwrap_or_default(),
                        };

                        if tx.send(docker_event).await.is_err() {
                            tracing::info!("DockerEventWatcher receiver dropped, stopping");
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "error reading Docker event stream");
                        // Continue reading — transient errors should not kill the watcher.
                    }
                }
            }

            tracing::info!("DockerEventWatcher stream ended");
        });

        self.task = Some(task);
        tracing::info!("DockerEventWatcher started");
        Ok(())
    }

    /// Stop the event stream and cancel the reading task.
    pub fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.event_rx.take();
        tracing::info!("DockerEventWatcher stopped");
    }

    /// Receive the next Docker event. Returns `None` when the watcher has
    /// been stopped or the channel is closed.
    pub async fn next_event(&mut self) -> Option<DockerWatchEvent> {
        self.event_rx.as_mut()?.recv().await
    }

    /// Returns a borrowed receiver for use with `select!` or iteration.
    pub fn receiver(&mut self) -> Option<&mut mpsc::Receiver<DockerWatchEvent>> {
        self.event_rx.as_mut()
    }
}

impl Default for DockerEventWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DockerEventWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    #[tokio::test]
    async fn test_file_watcher_create_and_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();

        let mut watcher = FileWatcher::new(vec![path.clone()]);
        watcher.start().await.unwrap();

        // Trigger a filesystem event.
        let test_file = dir.path().join("trigger.txt");
        fs::write(&test_file, "hello").unwrap();

        // Should receive the event (with a small timeout).
        let event = tokio::time::timeout(Duration::from_secs(3), watcher.next_event())
            .await
            .expect("timed out waiting for event")
            .expect("no event received");

        assert_eq!(event.kind, "create");
        assert!(event.path.ends_with("trigger.txt"));
    }

    #[tokio::test]
    async fn test_file_watcher_stop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();

        let mut watcher = FileWatcher::new(vec![path]);
        watcher.start().await.unwrap();
        watcher.stop();

        // After stopping, receiver should be None.
        assert!(watcher.receiver().is_none());
    }

    #[tokio::test]
    async fn test_file_watcher_drop_on_stop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();

        let mut watcher = FileWatcher::new(vec![path]);
        watcher.start().await.unwrap();

        // Dropping should be safe (no panic).
        drop(watcher);
    }

    #[test]
    fn test_classify_event_kind() {
        assert_eq!(classify_event_kind(&EventKind::Create(notify::event::CreateKind::File)), "create");
        assert_eq!(classify_event_kind(&EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Any))), "modify");
        assert_eq!(classify_event_kind(&EventKind::Remove(notify::event::RemoveKind::File)), "remove");
        assert_eq!(classify_event_kind(&EventKind::Access(notify::event::AccessKind::Close(notify::event::AccessMode::Any))), "access");
    }

    #[test]
    fn test_docker_event_watcher_default() {
        let _watcher = DockerEventWatcher::default();
    }

    // Note: DockerEventWatcher integration tests require a running Docker daemon.
    // They are not included here to avoid CI failures in non-Docker environments.
}

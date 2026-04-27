//! Prometheus metrics for ServerGuard
//!
//! Exposes guard metrics at a `/metrics` HTTP endpoint for Prometheus scraping.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Guard metrics — thread-safe counters and gauges
#[derive(Debug)]
pub struct GuardMetrics {
    /// Total events received from all sources
    pub events_received: AtomicU64,
    /// Events dropped by pipeline (filtered / debounced)
    pub events_dropped: AtomicU64,
    /// Events that triggered auto-fix
    pub auto_fixes_attempted: AtomicU64,
    /// Auto-fixes that succeeded
    pub auto_fixes_succeeded: AtomicU64,
    /// Events escalated (killswitch / alert)
    pub events_escalated: AtomicU64,
    /// Alerts sent to relay
    pub alerts_sent: AtomicU64,
    /// Alerts failed to send
    pub alerts_failed: AtomicU64,
    /// File change events
    pub file_changes: AtomicU64,
    /// Docker events
    pub docker_events: AtomicU64,
    /// State drift events
    pub state_drifts: AtomicU64,
    /// Process caught events (from eBPF/ES)
    pub processes_caught: AtomicU64,
    /// Currently running tasks
    pub tasks_running: AtomicU64,
}

impl Default for GuardMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl GuardMetrics {
    pub fn new() -> Self {
        Self {
            events_received: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
            auto_fixes_attempted: AtomicU64::new(0),
            auto_fixes_succeeded: AtomicU64::new(0),
            events_escalated: AtomicU64::new(0),
            alerts_sent: AtomicU64::new(0),
            alerts_failed: AtomicU64::new(0),
            file_changes: AtomicU64::new(0),
            docker_events: AtomicU64::new(0),
            state_drifts: AtomicU64::new(0),
            processes_caught: AtomicU64::new(0),
            tasks_running: AtomicU64::new(0),
        }
    }

    /// Increment a counter by 1
    pub fn inc(&self, counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Render metrics in Prometheus exposition format
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(2048);

        macro_rules! counter {
            ($name:expr, $help:expr, $val:expr) => {
                out.push_str(concat!("# HELP flowlink_guard_", $name, " ", $help, "\n"));
                out.push_str(concat!("# TYPE flowlink_guard_", $name, " counter\n"));
                out.push_str(&format!("flowlink_guard_{} {}\n", $name, $val.load(Ordering::Relaxed)));
            };
        }
        macro_rules! gauge {
            ($name:expr, $help:expr, $val:expr) => {
                out.push_str(concat!("# HELP flowlink_guard_", $name, " ", $help, "\n"));
                out.push_str(concat!("# TYPE flowlink_guard_", $name, " gauge\n"));
                out.push_str(&format!("flowlink_guard_{} {}\n", $name, $val.load(Ordering::Relaxed)));
            };
        }

        counter!("events_received_total", "Total events received", self.events_received);
        counter!("events_dropped_total", "Events dropped by pipeline", self.events_dropped);
        counter!("auto_fixes_attempted_total", "Auto-fix attempts", self.auto_fixes_attempted);
        counter!("auto_fixes_succeeded_total", "Successful auto-fixes", self.auto_fixes_succeeded);
        counter!("events_escalated_total", "Escalated events", self.events_escalated);
        counter!("alerts_sent_total", "Alerts sent to relay", self.alerts_sent);
        counter!("alerts_failed_total", "Failed alert sends", self.alerts_failed);
        counter!("file_changes_total", "File change events", self.file_changes);
        counter!("docker_events_total", "Docker events", self.docker_events);
        counter!("state_drifts_total", "State drift events", self.state_drifts);
        counter!("processes_caught_total", "Processes caught by shield", self.processes_caught);
        gauge!("tasks_running", "Currently running background tasks", self.tasks_running);

        out
    }
}

/// Spawn a tiny HTTP server for Prometheus metrics scraping
pub async fn spawn_metrics_server(addr: std::net::SocketAddr, metrics: Arc<GuardMetrics>) -> Result<(), std::io::Error> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(addr).await?;
    info!("🛡 ServerGuard metrics server listening on {}", addr);

    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let metrics = metrics.clone();
                tokio::spawn(async move {
                let mut stream = stream;
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 512];
                    let (mut read, mut write) = stream.split();
                    let _ = read.read(&mut buf).await;
                    let body = metrics.render();
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = write.write_all(resp.as_bytes()).await;
                });
            }
        }
    });

    Ok(())
}

use tracing::info;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_render() {
        let m = GuardMetrics::new();
        m.inc(&m.events_received);
        m.inc(&m.events_received);
        m.inc(&m.file_changes);

        let out = m.render();
        assert!(out.contains("flowlink_guard_events_received_total 2"));
        assert!(out.contains("flowlink_guard_file_changes_total 1"));
        assert!(out.contains("# TYPE flowlink_guard_events_received_total counter"));
    }

    #[test]
    fn test_metrics_default() {
        let m = GuardMetrics::new();
        assert_eq!(m.events_received.load(Ordering::Relaxed), 0);
        let out = m.render();
        assert!(out.contains("flowlink_guard_events_received_total 0"));
    }

    #[test]
    fn test_concurrent_increment() {
        let m = Arc::new(GuardMetrics::new());
        let mut handles = vec![];

        for _ in 0..100 {
            let m = m.clone();
            handles.push(std::thread::spawn(move || {
                m.inc(&m.events_received);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(m.events_received.load(Ordering::Relaxed), 100);
    }
}

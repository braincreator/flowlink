// Event Bus — pub/sub for SSE notifications
// Port of internal/relay/events.go

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct EventBus {
    channels: Arc<DashMap<String, broadcast::Sender<String>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
        }
    }

    /// Publish an event to a named channel.
    pub fn publish(&self, channel: &str, data: &str) {
        if let Some(tx) = self.channels.get(channel) {
            let _ = tx.send(data.to_string());
        }
    }

    /// Subscribe to a named channel. Returns a receiver.
    pub fn subscribe(&self, channel: &str) -> broadcast::Receiver<String> {
        if !self.channels.contains_key(channel) {
            let (tx, _) = broadcast::channel(256);
            self.channels.insert(channel.to_string(), tx);
        }
        self.channels.get(channel).unwrap().subscribe()
    }
}

// Event Bus — pub/sub for SSE notifications
// Port of internal/relay/events.go

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct EventBus {
    channels: Arc<DashMap<String, broadcast::Sender<String>>>,
    max_subscribers: usize, // future: reject subscribes above this
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
            max_subscribers: 100,
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
        self.channels
            .entry(channel.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .subscribe()
    }

    /// Number of active channels (monitoring).
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe("ch1");
        bus.publish("ch1", "hello");
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg, "hello");
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe("ch1");
        let mut rx2 = bus.subscribe("ch1");
        bus.publish("ch1", "hi");
        assert_eq!(rx1.recv().await.unwrap(), "hi");
        assert_eq!(rx2.recv().await.unwrap(), "hi");
    }

    #[tokio::test]
    async fn test_subscribe_creates_channel() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe("new-ch");
        bus.publish("new-ch", "data");
        assert_eq!(rx.recv().await.unwrap(), "data");
    }

    #[tokio::test]
    async fn test_publish_to_nonexistent_is_noop() {
        let bus = EventBus::new();
        bus.publish("ghost", "data"); // no panic, no error
    }

    #[tokio::test]
    async fn test_large_throughput() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe("ch");
        for i in 0..50 {
            bus.publish("ch", &i.to_string());
        }
        tokio::task::yield_now().await;
        let mut count = 0;
        while rx.try_recv().is_ok() { count += 1; }
        assert!(count > 0, "should receive at least some of 50 messages (got {})", count);
    }

    #[test]
    fn test_default() {
        let _bus = EventBus::default();
    }
}

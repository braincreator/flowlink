// Real-time Infrastructure Monitoring
// ================================
// Aggregates events from agents, shield, and audit trail into per-node health status.
// Streams updates via SSE for dashboard real-time visualization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Health status of an infrastructure node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,    // All good, no recent issues
    Degraded,   // Some warnings, still operational
    Alert,      // Active issues, needs attention
    Unknown,    // No data recently
}

impl HealthStatus {
    pub fn color(&self) -> &str {
        match self {
            HealthStatus::Healthy => "emerald",
            HealthStatus::Degraded => "amber",
            HealthStatus::Alert => "rose",
            HealthStatus::Unknown => "gray",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            HealthStatus::Healthy => "✅",
            HealthStatus::Degraded => "⚠️",
            HealthStatus::Alert => "🔴",
            HealthStatus::Unknown => "⚪",
        }
    }
}

/// Real-time event from infrastructure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfraEvent {
    pub id: String,
    pub timestamp: String,
    pub event_type: InfraEventType,
    pub org_id: String,
    pub node_id: Option<String>,
    pub agent_id: Option<String>,
    pub severity: EventSeverity,
    pub message: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InfraEventType {
    // Agent events
    AgentConnected,
    AgentDisconnected,
    AgentHeartbeat,

    // Command events (from Shield)
    CommandExecuted,
    CommandBlocked,
    CommandApproved,
    CommandRejected,

    // Service events
    ServiceStarted,
    ServiceStopped,
    ServiceCrashed,
    ServiceHealthCheck,

    // Security events
    AnomalousCommand,
    RateLimitHit,
    UnauthorizedAccess,

    // Discovery events
    DiscoveryStarted,
    DiscoveryCompleted,
    DiscoveryNewSecret,

    // Infrastructure events
    HostHighCpu,
    HostHighMemory,
    HostLowDisk,
    EndpointDown,
    EndpointSlow,

    // Custom
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl EventSeverity {
    pub fn as_str(&self) -> &str {
        match self {
            EventSeverity::Info => "info",
            EventSeverity::Warning => "warning",
            EventSeverity::Error => "error",
            EventSeverity::Critical => "critical",
        }
    }

    pub fn affects_health(&self) -> bool {
        matches!(self, EventSeverity::Warning | EventSeverity::Error | EventSeverity::Critical)
    }
}

/// Per-node health summary (computed from recent events)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node_id: String,
    pub node_type: String,
    pub node_name: String,
    pub status: HealthStatus,
    pub last_event: Option<String>,
    pub last_event_time: Option<String>,
    pub events_1h: u64,
    pub warnings_1h: u64,
    pub errors_1h: u64,
    pub blocked_commands_1h: u64,
    pub uptime_percent: Option<f64>,
}

/// Full infrastructure health snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfraHealthSnapshot {
    pub org_id: String,
    pub timestamp: String,
    pub total_nodes: u64,
    pub healthy: u64,
    pub degraded: u64,
    pub alert: u64,
    pub unknown: u64,
    pub nodes: Vec<NodeHealth>,
    pub recent_events: Vec<InfraEvent>,
}

/// In-memory health tracker (per org)
pub struct HealthTracker {
    /// node_id → health
    health: Arc<RwLock<HashMap<String, NodeHealth>>>,
    /// Recent events (last 100 per org)
    recent_events: Arc<RwLock<Vec<InfraEvent>>>,
    /// SSE subscribers
    subscribers: Arc<RwLock<Vec<tokio::sync::mpsc::Sender<InfraEvent>>>>,
}

impl HealthTracker {
    pub fn new() -> Self {
        Self {
            health: Arc::new(RwLock::new(HashMap::new())),
            recent_events: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record an event and update node health
    pub async fn record_event(&self, event: InfraEvent) {
        // Update node health
        if let Some(node_id) = &event.node_id {
            let mut health = self.health.write().await;
            let node = health.entry(node_id.clone()).or_insert_with(|| NodeHealth {
                node_id: node_id.clone(),
                node_type: "unknown".into(),
                node_name: node_id.clone(),
                status: HealthStatus::Unknown,
                last_event: None,
                last_event_time: None,
                events_1h: 0,
                warnings_1h: 0,
                errors_1h: 0,
                blocked_commands_1h: 0,
                uptime_percent: None,
            });

            node.events_1h += 1;
            node.last_event = Some(event.event_type.event_type_str().to_string());
            node.last_event_time = Some(event.timestamp.clone());

            match &event.severity {
                EventSeverity::Warning => {
                    node.warnings_1h += 1;
                    if node.status != HealthStatus::Alert {
                        node.status = HealthStatus::Degraded;
                    }
                }
                EventSeverity::Error | EventSeverity::Critical => {
                    node.errors_1h += 1;
                    node.status = HealthStatus::Alert;
                }
                EventSeverity::Info => {
                    // If no recent warnings/errors, mark healthy
                    if node.warnings_1h == 0 && node.errors_1h == 0 {
                        node.status = HealthStatus::Healthy;
                    }
                }
            }

            if event.event_type == InfraEventType::CommandBlocked {
                node.blocked_commands_1h += 1;
            }
        }

        // Store recent events (keep last 100)
        {
            let mut events = self.recent_events.write().await;
            events.push(event.clone());
            if events.len() > 100 {
                let excess = events.len() - 100;
                events.drain(0..excess);
            }
        }

        // Notify SSE subscribers
        let subs = self.subscribers.read().await;
        for tx in subs.iter() {
            let _ = tx.send(event.clone()).await;
        }
    }

    /// Subscribe to real-time events (SSE)
    pub async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<InfraEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        self.subscribers.write().await.push(tx);
        rx
    }

    /// Get current health snapshot
    pub async fn snapshot(&self, org_id: &str) -> InfraHealthSnapshot {
        let health = self.health.read().await;
        let events = self.recent_events.read().await;

        let nodes: Vec<NodeHealth> = health.values().cloned().collect();
        let healthy = nodes.iter().filter(|n| n.status == HealthStatus::Healthy).count() as u64;
        let degraded = nodes.iter().filter(|n| n.status == HealthStatus::Degraded).count() as u64;
        let alert = nodes.iter().filter(|n| n.status == HealthStatus::Alert).count() as u64;
        let unknown = nodes.iter().filter(|n| n.status == HealthStatus::Unknown).count() as u64;

        InfraHealthSnapshot {
            org_id: org_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_nodes: nodes.len() as u64,
            healthy,
            degraded,
            alert,
            unknown,
            recent_events: events.iter().rev().take(20).cloned().collect::<Vec<_>>().into_iter().rev().collect(),
            nodes,
        }
    }

    /// Decay old health states (call periodically)
    pub async fn decay(&self) {
        let mut health = self.health.write().await;
        for node in health.values_mut() {
            // Decay counters (simple approach: halve every tick)
            node.events_1h /= 2;
            node.warnings_1h /= 2;
            node.errors_1h /= 2;
            node.blocked_commands_1h /= 2;

            // If no recent issues, promote back to healthy
            if node.warnings_1h == 0 && node.errors_1h == 0 && node.status != HealthStatus::Unknown {
                node.status = HealthStatus::Healthy;
            }
        }
    }
}

impl InfraEventType {
    pub fn event_type_str(&self) -> &str {
        match self {
            InfraEventType::AgentConnected => "agent_connected",
            InfraEventType::AgentDisconnected => "agent_disconnected",
            InfraEventType::AgentHeartbeat => "agent_heartbeat",
            InfraEventType::CommandExecuted => "command_executed",
            InfraEventType::CommandBlocked => "command_blocked",
            InfraEventType::CommandApproved => "command_approved",
            InfraEventType::CommandRejected => "command_rejected",
            InfraEventType::ServiceStarted => "service_started",
            InfraEventType::ServiceStopped => "service_stopped",
            InfraEventType::ServiceCrashed => "service_crashed",
            InfraEventType::ServiceHealthCheck => "service_health_check",
            InfraEventType::AnomalousCommand => "anomalous_command",
            InfraEventType::RateLimitHit => "rate_limit_hit",
            InfraEventType::UnauthorizedAccess => "unauthorized_access",
            InfraEventType::DiscoveryStarted => "discovery_started",
            InfraEventType::DiscoveryCompleted => "discovery_completed",
            InfraEventType::DiscoveryNewSecret => "discovery_new_secret",
            InfraEventType::HostHighCpu => "host_high_cpu",
            InfraEventType::HostHighMemory => "host_high_memory",
            InfraEventType::HostLowDisk => "host_low_disk",
            InfraEventType::EndpointDown => "endpoint_down",
            InfraEventType::EndpointSlow => "endpoint_slow",
            InfraEventType::Custom(s) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_colors() {
        assert_eq!(HealthStatus::Healthy.color(), "emerald");
        assert_eq!(HealthStatus::Degraded.color(), "amber");
        assert_eq!(HealthStatus::Alert.color(), "rose");
        assert_eq!(HealthStatus::Unknown.color(), "gray");
    }

    #[test]
    fn test_health_status_icons() {
        assert_eq!(HealthStatus::Healthy.icon(), "✅");
        assert_eq!(HealthStatus::Alert.icon(), "🔴");
    }

    #[test]
    fn test_severity_affects_health() {
        assert!(!EventSeverity::Info.affects_health());
        assert!(EventSeverity::Warning.affects_health());
        assert!(EventSeverity::Error.affects_health());
        assert!(EventSeverity::Critical.affects_health());
    }

    #[test]
    fn test_infra_event_serialization() {
        let event = InfraEvent {
            id: "evt-1".into(),
            timestamp: "2026-04-25T20:00:00Z".into(),
            event_type: InfraEventType::CommandBlocked,
            org_id: "org-123".into(),
            node_id: Some("svc-billing".into()),
            agent_id: Some("agent-1".into()),
            severity: EventSeverity::Warning,
            message: "Blocked destructive command".into(),
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("command_blocked"));
        assert!(json.contains("billing"));
    }

    #[test]
    fn test_event_type_str() {
        assert_eq!(InfraEventType::AgentConnected.event_type_str(), "agent_connected");
        assert_eq!(InfraEventType::CommandBlocked.event_type_str(), "command_blocked");
        assert_eq!(InfraEventType::Custom("test".into()).event_type_str(), "test");
    }

    #[tokio::test]
    async fn test_health_tracker_record() {
        let tracker = HealthTracker::new();

        tracker.record_event(InfraEvent {
            id: "e1".into(),
            timestamp: "2026-04-25T20:00:00Z".into(),
            event_type: InfraEventType::CommandBlocked,
            org_id: "org-1".into(),
            node_id: Some("svc-test".into()),
            agent_id: Some("agent-1".into()),
            severity: EventSeverity::Warning,
            message: "test".into(),
            metadata: HashMap::new(),
        }).await;

        let snap = tracker.snapshot("org-1").await;
        assert_eq!(snap.total_nodes, 1);
        assert_eq!(snap.nodes[0].status, HealthStatus::Degraded);
        assert_eq!(snap.nodes[0].warnings_1h, 1);
        assert_eq!(snap.nodes[0].blocked_commands_1h, 1);
    }

    #[tokio::test]
    async fn test_health_tracker_decay() {
        let tracker = HealthTracker::new();

        // Alert event
        tracker.record_event(InfraEvent {
            id: "e1".into(),
            timestamp: "2026-04-25T20:00:00Z".into(),
            event_type: InfraEventType::ServiceCrashed,
            org_id: "org-1".into(),
            node_id: Some("svc-test".into()),
            agent_id: None,
            severity: EventSeverity::Critical,
            message: "crash".into(),
            metadata: HashMap::new(),
        }).await;

        let snap = tracker.snapshot("org-1").await;
        assert_eq!(snap.nodes[0].status, HealthStatus::Alert);

        // Decay until healthy
        tracker.decay().await;
        tracker.decay().await;

        let snap = tracker.snapshot("org-1").await;
        assert_eq!(snap.nodes[0].status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_snapshot() {
        let tracker = HealthTracker::new();

        for i in 0..3 {
            tracker.record_event(InfraEvent {
                id: format!("e{i}"),
                timestamp: format!("2026-04-25T20:0{i}:00Z"),
                event_type: InfraEventType::AgentHeartbeat,
                org_id: "org-1".into(),
                node_id: Some(format!("agent-{i}")),
                agent_id: Some(format!("agent-{i}")),
                severity: EventSeverity::Info,
                message: "heartbeat".into(),
                metadata: HashMap::new(),
            }).await;
        }

        let snap = tracker.snapshot("org-1").await;
        assert_eq!(snap.total_nodes, 3);
        assert_eq!(snap.healthy, 3);
        assert_eq!(snap.alert, 0);
        assert_eq!(snap.recent_events.len(), 3);
    }
}

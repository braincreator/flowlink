use axum::http::StatusCode;
use axum::response::IntoResponse;
use prometheus::{core::Collector, CounterVec, Encoder, Gauge, GaugeVec, HistogramVec, Opts, Registry, TextEncoder};

/// Prometheus metrics for FlowLink Relay.
#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,

    // Shield metrics
    pub interceptions_total: CounterVec,
    pub interception_duration_ns: HistogramVec,
    pub false_positives: CounterVec,
    pub active_pending_approvals: Gauge,

    // Agent metrics
    pub agents_registered: Gauge,
    pub agent_commands_total: CounterVec,
    pub agent_heartbeat_lag_ms: HistogramVec,

    // Relay metrics
    pub http_requests_total: CounterVec,
    pub http_request_duration_ms: HistogramVec,
    pub sse_connections: Gauge,
    pub eventbus_events_total: CounterVec,
    pub ws_connections: Gauge,

    // Crypto metrics
    pub crypto_operations_total: CounterVec,
    pub crypto_duration_ns: HistogramVec,

    // System
    pub uptime_seconds: Gauge,
    pub config_reload_total: GaugeVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let interceptions_total = CounterVec::new(
            Opts::new("flowlink_interceptions_total", "Total number of shield interceptions"),
            &["level", "action", "origin"],
        ).unwrap();

        let interception_duration_ns = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_interception_duration_seconds",
                "Duration of shield interception analysis",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["level"],
        ).unwrap();

        let false_positives = CounterVec::new(
            Opts::new("flowlink_false_positives_total", "Total number of false positive interceptions"),
            &["level"],
        ).unwrap();

        let active_pending_approvals = Gauge::new(
            "flowlink_active_pending_approvals",
            "Number of currently pending approvals",
        ).unwrap();

        let agents_registered = Gauge::new(
            "flowlink_agents_registered",
            "Number of currently registered agents",
        ).unwrap();

        let agent_commands_total = CounterVec::new(
            Opts::new("flowlink_agent_commands_total", "Total agent commands processed"),
            &["status"],
        ).unwrap();

        let agent_heartbeat_lag_ms = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_agent_heartbeat_lag_seconds",
                "Agent heartbeat lag in seconds",
            )
            .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]),
            &[],
        ).unwrap();

        let http_requests_total = CounterVec::new(
            Opts::new("flowlink_http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        ).unwrap();

        let http_request_duration_ms = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0]),
            &["method", "path"],
        ).unwrap();

        let sse_connections = Gauge::new(
            "flowlink_sse_connections",
            "Number of active SSE connections",
        ).unwrap();

        let eventbus_events_total = CounterVec::new(
            Opts::new("flowlink_eventbus_events_total", "Total events published on the event bus"),
            &["event_type"],
        ).unwrap();

        let ws_connections = Gauge::new(
            "flowlink_ws_connections",
            "Number of active WebSocket connections",
        ).unwrap();

        let crypto_operations_total = CounterVec::new(
            Opts::new("flowlink_crypto_operations_total", "Total cryptographic operations"),
            &["operation"],
        ).unwrap();

        let crypto_duration_ns = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_crypto_operation_duration_seconds",
                "Duration of cryptographic operations",
            )
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05]),
            &["operation"],
        ).unwrap();

        let uptime_seconds = Gauge::new(
            "flowlink_uptime_seconds",
            "Relay uptime in seconds",
        ).unwrap();

        let config_reload_total = GaugeVec::new(
            Opts::new("flowlink_config_reload_total", "Total configuration reloads"),
            &[],
        ).unwrap();

        registry.register(Box::new(interceptions_total.clone())).unwrap();
        registry.register(Box::new(interception_duration_ns.clone())).unwrap();
        registry.register(Box::new(false_positives.clone())).unwrap();
        registry.register(Box::new(active_pending_approvals.clone())).unwrap();
        registry.register(Box::new(agents_registered.clone())).unwrap();
        registry.register(Box::new(agent_commands_total.clone())).unwrap();
        registry.register(Box::new(agent_heartbeat_lag_ms.clone())).unwrap();
        registry.register(Box::new(http_requests_total.clone())).unwrap();
        registry.register(Box::new(http_request_duration_ms.clone())).unwrap();
        registry.register(Box::new(sse_connections.clone())).unwrap();
        registry.register(Box::new(eventbus_events_total.clone())).unwrap();
        registry.register(Box::new(ws_connections.clone())).unwrap();
        registry.register(Box::new(crypto_operations_total.clone())).unwrap();
        registry.register(Box::new(crypto_duration_ns.clone())).unwrap();
        registry.register(Box::new(uptime_seconds.clone())).unwrap();
        registry.register(Box::new(config_reload_total.clone())).unwrap();

        Self {
            registry,
            interceptions_total,
            interception_duration_ns,
            false_positives,
            active_pending_approvals,
            agents_registered,
            agent_commands_total,
            agent_heartbeat_lag_ms,
            http_requests_total,
            http_request_duration_ms,
            sse_connections,
            eventbus_events_total,
            ws_connections,
            crypto_operations_total,
            crypto_duration_ns,
            uptime_seconds,
            config_reload_total,
        }
    }

    /// Render metrics in Prometheus text format.
    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap_or_default()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Axum handler for GET /metrics — extracts Metrics from AppState.
pub async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<super::server::AppState>,
) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(()) => (StatusCode::OK, String::from_utf8(buffer).unwrap_or_default()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Metrics {
        Metrics::new()
    }

    #[test]
    fn test_counter_increment() {
        let m = setup();
        m.interceptions_total
            .with_label_values(&["l2", "block", "agent-1"])
            .inc();
        m.interceptions_total
            .with_label_values(&["l1", "allow", "local"])
            .inc();
        assert_eq!(m.interceptions_total.with_label_values(&["l2", "block", "agent-1"]).get(), 1.0);
        assert_eq!(m.interceptions_total.with_label_values(&["l1", "allow", "local"]).get(), 1.0);
    }

    #[test]
    fn test_gauge_set_get() {
        let m = setup();
        m.agents_registered.set(5.0);
        assert_eq!(m.agents_registered.get(), 5.0);
        m.agents_registered.set(0.0);
        assert_eq!(m.agents_registered.get(), 0.0);
    }

    #[test]
    fn test_histogram_observation() {
        let m = setup();
        m.http_request_duration_ms
            .with_label_values(&["GET", "/health"])
            .observe(0.05);
        m.http_request_duration_ms
            .with_label_values(&["GET", "/health"])
            .observe(0.1);
        m.http_request_duration_ms
            .with_label_values(&["GET", "/health"])
            .observe(0.25);
        // If we get here without panic, observation works.
        let mf = m.http_request_duration_ms.collect();
        assert_eq!(mf[0].get_metric().len(), 1);
        assert!(mf[0].get_metric()[0].get_sample_count() >= 3);
    }

    #[test]
    fn test_prometheus_text_output() {
        let m = setup();
        m.agents_registered.set(3.0);
        m.http_requests_total
            .with_label_values(&["GET", "/health", "200"])
            .inc();

        let output = m.render();

        assert!(output.contains("flowlink_agents_registered 3"));
        assert!(output.contains("flowlink_http_requests_total{"));
        assert!(output.contains("GET"));
        assert!(output.contains("/health"));
        assert!(output.contains("200"));
    }

    #[test]
    fn test_metric_names_valid() {
        let m = setup();
        let output = m.render();

        let re = regex::Regex::new(r"^[a-zA-Z_:][a-zA-Z0-9_:]*$").unwrap();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let name = line.split(|c: char| c == '{' || c == ' ').next().unwrap();
            assert!(re.is_match(name), "Invalid metric name: {name}");
        }
    }

    #[test]
    fn test_false_positives_counter() {
        let m = setup();
        m.false_positives.with_label_values(&["l1"]).inc();
        m.false_positives.with_label_values(&["l2"]).inc_by(3.0);
        assert_eq!(m.false_positives.with_label_values(&["l1"]).get(), 1.0);
        assert_eq!(m.false_positives.with_label_values(&["l2"]).get(), 3.0);
    }

    #[test]
    fn test_crypto_metrics() {
        let m = setup();
        m.crypto_operations_total
            .with_label_values(&["encrypt"])
            .inc();
        m.crypto_duration_ns
            .with_label_values(&["encrypt"])
            .observe(0.002);
        assert_eq!(m.crypto_operations_total.with_label_values(&["encrypt"]).get(), 1.0);
    }

    #[test]
    fn test_eventbus_counter() {
        let m = setup();
        m.eventbus_events_total
            .with_label_values(&["heartbeat"])
            .inc();
        m.eventbus_events_total
            .with_label_values(&["shield_alert"])
            .inc();
        assert_eq!(m.eventbus_events_total.with_label_values(&["heartbeat"]).get(), 1.0);
        assert_eq!(m.eventbus_events_total.with_label_values(&["shield_alert"]).get(), 1.0);
    }
}

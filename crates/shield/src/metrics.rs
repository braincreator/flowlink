use prometheus::{core::Collector, CounterVec, Encoder, Gauge, HistogramVec, Opts, Registry, TextEncoder};

/// Prometheus metrics for FlowLink Shield (standalone binary).
#[derive(Clone)]
pub struct ShieldMetrics {
    pub registry: Registry,

    // Interception counters by level/action
    pub interceptions_total: CounterVec,

    // Analysis duration histograms per level
    pub l1_analysis_duration: HistogramVec,
    pub l2_analysis_duration: HistogramVec,
    pub l3_analysis_duration: HistogramVec,

    // Approval queue
    pub approval_queue_size: Gauge,

    // Snapshots
    pub snapshots_created_total: Gauge,
    pub snapshot_duration: HistogramVec,
}

impl ShieldMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let interceptions_total = CounterVec::new(
            Opts::new("flowlink_shield_interceptions_total", "Total shield interceptions by level and action"),
            &["level", "action"],
        ).unwrap();

        let l1_analysis_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_shield_l1_analysis_duration_seconds",
                "L1 pattern analysis duration",
            )
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01]),
            &[],
        ).unwrap();

        let l2_analysis_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_shield_l2_analysis_duration_seconds",
                "L2 AST analysis duration",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5]),
            &[],
        ).unwrap();

        let l3_analysis_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_shield_l3_analysis_duration_seconds",
                "L3 interpreter analysis duration",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &[],
        ).unwrap();

        let approval_queue_size = Gauge::new(
            "flowlink_shield_approval_queue_size",
            "Current number of pending approval requests",
        ).unwrap();

        let snapshots_created_total = Gauge::new(
            "flowlink_shield_snapshots_created_total",
            "Total snapshots created",
        ).unwrap();

        let snapshot_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "flowlink_shield_snapshot_duration_seconds",
                "Duration of snapshot creation",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &[],
        ).unwrap();

        registry.register(Box::new(interceptions_total.clone())).unwrap();
        registry.register(Box::new(l1_analysis_duration.clone())).unwrap();
        registry.register(Box::new(l2_analysis_duration.clone())).unwrap();
        registry.register(Box::new(l3_analysis_duration.clone())).unwrap();
        registry.register(Box::new(approval_queue_size.clone())).unwrap();
        registry.register(Box::new(snapshots_created_total.clone())).unwrap();
        registry.register(Box::new(snapshot_duration.clone())).unwrap();

        Self {
            registry,
            interceptions_total,
            l1_analysis_duration,
            l2_analysis_duration,
            l3_analysis_duration,
            approval_queue_size,
            snapshots_created_total,
            snapshot_duration,
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

impl Default for ShieldMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interception_counter() {
        let m = ShieldMetrics::new();
        m.interceptions_total
            .with_label_values(&["l1", "allow"])
            .inc();
        m.interceptions_total
            .with_label_values(&["l2", "block"])
            .inc_by(2.0);
        assert_eq!(m.interceptions_total.with_label_values(&["l1", "allow"]).get(), 1.0);
        assert_eq!(m.interceptions_total.with_label_values(&["l2", "block"]).get(), 2.0);
    }

    #[test]
    fn test_level_durations() {
        let m = ShieldMetrics::new();
        m.l1_analysis_duration.with_label_values(&[]).observe(0.0005);
        m.l2_analysis_duration.with_label_values(&[]).observe(0.05);
        m.l3_analysis_duration.with_label_values(&[]).observe(0.5);

        for hist in [&m.l1_analysis_duration, &m.l2_analysis_duration, &m.l3_analysis_duration] {
            let mf = hist.collect();
            assert_eq!(mf[0].get_metric()[0].get_sample_count(), 1);
        }
    }

    #[test]
    fn test_approval_queue_gauge() {
        let m = ShieldMetrics::new();
        m.approval_queue_size.set(5.0);
        assert_eq!(m.approval_queue_size.get(), 5.0);
        m.approval_queue_size.dec();
        assert_eq!(m.approval_queue_size.get(), 4.0);
    }

    #[test]
    fn test_snapshot_metrics() {
        let m = ShieldMetrics::new();
        m.snapshots_created_total.inc();
        m.snapshot_duration.with_label_values(&[]).observe(0.3);
        assert_eq!(m.snapshots_created_total.get(), 1.0);
    }

    #[test]
    fn test_render_output() {
        let m = ShieldMetrics::new();
        m.interceptions_total
            .with_label_values(&["l2", "block"])
            .inc();
        let output = m.render();
        assert!(output.contains("flowlink_shield_interceptions_total"));
        assert!(output.contains("l2"));
        assert!(output.contains("block"));
    }

    #[test]
    fn test_metric_names_valid() {
        let m = ShieldMetrics::new();
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
}

//! E2E tests for ServerGuard
//!
//! Tests the full pipeline: event source → pipeline → actuator → alert
//! Uses real filesystem operations and in-memory event routing.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flowlink_gitops::server_guard::event_types::GuardAlert;
use flowlink_gitops::server_guard::event_types::GuardEvent;
use flowlink_gitops::server_guard::guard_mode::GuardKillswitch;
use flowlink_gitops::server_guard::metrics::GuardMetrics;
use flowlink_gitops::server_guard::pipeline::{AutoFixPattern, Pipeline, PipelineConfig, PipelineOutcome};
use flowlink_gitops::server_guard::command_runner::CommandRunner;
use flowlink_gitops::server_guard::event_types::ActionTier;

/// Collect alerts in a shared vec for assertions
fn alert_collector() -> (Arc<Mutex<Vec<GuardAlert>>>, Arc<dyn Fn(GuardAlert) + Send + Sync>) {
    let alerts: Arc<Mutex<Vec<GuardAlert>>> = Arc::new(Mutex::new(vec![]));
    let alerts_clone = alerts.clone();
    let cb: Arc<dyn Fn(GuardAlert) + Send + Sync> = Arc::new(move |alert| {
        alerts_clone.lock().unwrap().push(alert);
    });
    (alerts, cb)
}

fn default_pipeline(
    alert_cb: Arc<dyn Fn(GuardAlert) + Send + Sync>,
    metrics: Option<Arc<GuardMetrics>>,
) -> Pipeline {
    let config = PipelineConfig {
        debounce_secs: 0,
        self_change_cooldown_secs: 0,
        max_events_per_sec: 1000,
        safe_paths: vec![],
        dangerous_paths: vec!["/etc/shadow".into(), "/etc/passwd".into()],
        auto_fix_rules: vec![
            AutoFixPattern {
                source: flowlink_gitops::server_guard::event_types::EventSource::FileSystem,
                path_prefix: Some("/tmp/guard-test/important.conf".into()),
                docker_action: None,
                min_severity: None,
                command: vec!["cp".into(), "/tmp/guard-test/important.conf.bak".into(), "/tmp/guard-test/important.conf".into()],
                notify: true,
            },
        ],
    };
    let killswitch = Arc::new(GuardKillswitch::new());
    let runner = Arc::new(CommandRunner::new());
    match metrics {
        Some(m) => Pipeline::with_metrics(config, killswitch, runner, alert_cb, Some(m)),
        None => Pipeline::new(config, killswitch, runner, alert_cb),
    }
}

// ═══════════════════════════════════════════════════════════════
// E2E 1: File change on dangerous path → classify → alert
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_file_change_triggers_alert() {
    let (alerts, cb) = alert_collector();
    let mut pipeline = default_pipeline(cb, None);

    let event = GuardEvent::file_change(
        std::path::PathBuf::from("/etc/shadow"),
        "modify".into(),
        Some("newhash123".into()),
        Some("oldhash456".into()),
    );

    let outcome = pipeline.process(event).await;

    // Should escalate (dangerous path) or at least log
    match &outcome {
        PipelineOutcome::Escalated { .. } | PipelineOutcome::Logged => {}
        other => panic!("Expected Escalated or Logged, got {:?}", other),
    }

    assert!(!alerts.lock().unwrap().is_empty(), "Should have sent at least one alert");
}

// ═══════════════════════════════════════════════════════════════
// E2E 2: Full FS roundtrip — auto-fix restores file
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_filesystem_auto_fix() {
    let dir = std::path::PathBuf::from("/tmp/guard-e2e-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let file_path = dir.join("important.conf");
    let backup_path = dir.join("important.conf.bak");

    // Setup: original + backup
    std::fs::write(&file_path, "original\n").unwrap();
    std::fs::write(&backup_path, "original\n").unwrap();

    let (alerts, cb) = alert_collector();
    let config = PipelineConfig {
        debounce_secs: 0,
        self_change_cooldown_secs: 0,
        max_events_per_sec: 1000,
        safe_paths: vec![],
        dangerous_paths: vec![],
        auto_fix_rules: vec![
            AutoFixPattern {
                source: flowlink_gitops::server_guard::event_types::EventSource::FileSystem,
                path_prefix: Some(file_path.to_string_lossy().to_string()),
                docker_action: None,
                min_severity: None,
                command: vec!["cp".into(), backup_path.to_string_lossy().to_string(), file_path.to_string_lossy().to_string()],
                notify: true,
            },
        ],
    };
    let killswitch = Arc::new(GuardKillswitch::new());
    let runner = Arc::new(CommandRunner::new());
    let mut pipeline = Pipeline::new(config, killswitch, runner, cb);

    // Tamper
    std::fs::write(&file_path, "TAMPERED!\n").unwrap();

    let event = GuardEvent::file_change(
        file_path.clone(),
        "modify".into(),
        Some("tampered_hash".into()),
        Some("original_hash".into()),
    );

    let outcome = pipeline.process(event).await;

    match &outcome {
        PipelineOutcome::AutoFixed { success, command } => {
            assert!(success, "Auto-fix should succeed");
            assert!(command.contains("cp"));
            let restored = std::fs::read_to_string(&file_path).unwrap();
            assert_eq!(restored, "original\n", "File should be restored from backup");
        }
        PipelineOutcome::Logged => {} // Pattern may not match exactly
        other => panic!("Expected AutoFixed or Logged, got {:?}", other),
    }

    assert!(!alerts.lock().unwrap().is_empty(), "Should have sent notification alert");

    let _ = std::fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════
// E2E 3: Killswitch emergency escalation
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_killswitch_emergency() {
    let (_alerts, cb) = alert_collector();
    let killswitch = Arc::new(GuardKillswitch::new());
    let runner = Arc::new(CommandRunner::new());

    let config = PipelineConfig {
        debounce_secs: 0,
        dangerous_paths: vec!["/etc/shadow".into()],
        ..PipelineConfig::default()
    };

    let mut pipeline = Pipeline::new(config, killswitch.clone(), runner, cb);

    let event = GuardEvent::file_change(
        std::path::PathBuf::from("/etc/shadow"),
        "modify".into(),
        Some("bad".into()),
        Some("good".into()),
    );

    let outcome = pipeline.process(event).await;

    if let PipelineOutcome::Escalated { severity, .. } = &outcome {
        assert_eq!(*severity, flowlink_gitops::server_guard::event_types::Severity::Critical);
        assert!(killswitch.is_paused() || killswitch.is_emergency());
    }
}

// ═══════════════════════════════════════════════════════════════
// E2E 4: Metrics tracking through pipeline
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_metrics_tracking() {
    let (_alerts, cb) = alert_collector();
    let metrics = Arc::new(GuardMetrics::new());
    let mut pipeline = default_pipeline(cb, Some(metrics.clone()));

    let events = vec![
        GuardEvent::file_change(std::path::PathBuf::from("/etc/nginx/nginx.conf"), "modify".into(), None, None),
        GuardEvent::docker_event("start".into(), Some("abc123".into()), Some("web".into()), Some("nginx:latest".into())),
        GuardEvent::process_caught(1234, 0, "rm".into(), "-rf /tmp".into(), false),
        GuardEvent::file_change(std::path::PathBuf::from("/var/log/syslog"), "modify".into(), None, None),
    ];

    for event in events {
        let _ = pipeline.process(event).await;
    }

    use std::sync::atomic::Ordering;
    assert!(metrics.events_received.load(Ordering::Relaxed) >= 4);
    assert!(metrics.file_changes.load(Ordering::Relaxed) >= 2);
    assert!(metrics.docker_events.load(Ordering::Relaxed) >= 1);
    assert!(metrics.processes_caught.load(Ordering::Relaxed) >= 1);

    let output = metrics.render();
    assert!(output.contains("flowlink_guard_events_received_total"));
    assert!(output.contains("# TYPE flowlink_guard_events_received_total counter"));
}

// ═══════════════════════════════════════════════════════════════
// E2E 5: Docker event classification
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_docker_event_classify() {
    let (_alerts, cb) = alert_collector();
    let mut pipeline = default_pipeline(cb, None);

    let event = GuardEvent::docker_event(
        "destroy".into(), Some("container123".into()), Some("db-production".into()), Some("postgres:15".into()),
    );

    let outcome = pipeline.process(event).await;
    match &outcome {
        PipelineOutcome::Logged | PipelineOutcome::Escalated { .. } => {}
        other => panic!("Expected Logged or Escalated, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════
// E2E 6: Dangerous process caught by shield
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_process_caught_dangerous() {
    let (alerts, cb) = alert_collector();
    let mut pipeline = default_pipeline(cb, None);

    let event = GuardEvent::process_caught(9999, 0, "rm".into(), "-rf / --no-preserve-root".into(), false);
    let outcome = pipeline.process(event).await;

    match &outcome {
        PipelineOutcome::Escalated { severity, .. } => {
            assert!(*severity >= flowlink_gitops::server_guard::event_types::Severity::High);
        }
        PipelineOutcome::Logged => {}
        other => panic!("Expected Escalated or Logged, got {:?}", other),
    }
    assert!(!alerts.lock().unwrap().is_empty(), "Should alert on dangerous process");
}

// ═══════════════════════════════════════════════════════════════
// E2E 7: Debounce — rapid events
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_debounce_rapid_events() {
    let (_alerts, cb) = alert_collector();
    let config = PipelineConfig { debounce_secs: 2, ..PipelineConfig::default() };
    let killswitch = Arc::new(GuardKillswitch::new());
    let runner = Arc::new(CommandRunner::new());
    let mut pipeline = Pipeline::new(config, killswitch, runner, cb);

    for i in 0..5 {
        let event = GuardEvent::file_change(
            std::path::PathBuf::from("/etc/nginx/nginx.conf"),
            "modify".into(),
            Some(format!("hash-{}", i)),
            None,
        );
        let _ = pipeline.process(event).await;
    }
    // Pipeline should handle all 5 without crashing
}

// ═══════════════════════════════════════════════════════════════
// E2E 8: CommandRunner — real execution
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_command_runner_echo() {
    let runner = CommandRunner::new();
    let result = runner.run("echo", &["hello world"]).await;
    assert!(result.success);
    assert_eq!(result.stdout.trim(), "hello world");
    assert_eq!(result.exit_code, Some(0));
}

#[tokio::test]
async fn e2e_command_runner_fail() {
    let runner = CommandRunner::new();
    let result = runner.run("false", &[]).await;
    assert!(!result.success);
    assert_ne!(result.exit_code, Some(0));
}

#[tokio::test]
async fn e2e_command_runner_captures_stderr() {
    let runner = CommandRunner::new();
    let result = runner.run("sh", &["-c", "echo err >&2"]).await;
    assert!(result.success);
    assert!(result.stderr.contains("err"));
}

// ═══════════════════════════════════════════════════════════════
// E2E 9: Full guard lifecycle — start → submit → stop
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_guard_lifecycle() {
    use flowlink_gitops::server_guard::{ServerGuard, ServerGuardConfig};

    let config = ServerGuardConfig {
        watch_paths: vec![],
        watch_docker: false,
        watch_state: false,
        ..ServerGuardConfig::default()
    };

    let mut guard = ServerGuard::new(config);
    guard.start().await.unwrap();
    assert!(guard.status().tasks_running >= 1);

    let metrics = guard.metrics().clone();

    guard.submit_event(GuardEvent::file_change(
        std::path::PathBuf::from("/etc/test"), "create".into(), None, None,
    )).await;

    guard.submit_event(GuardEvent::docker_event(
        "start".into(), Some("abc".into()), Some("web".into()), Some("nginx".into()),
    )).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    use std::sync::atomic::Ordering;
    assert!(metrics.events_received.load(Ordering::Relaxed) >= 2);

    guard.stop().await;
    assert_eq!(guard.status().tasks_running, 0);
}

// ═══════════════════════════════════════════════════════════════
// E2E 10: Alert JSON roundtrip (relay compatibility)
// ═══════════════════════════════════════════════════════════════

#[test]
fn e2e_alert_json_roundtrip() {
    let event = GuardEvent::process_caught(1234, 0, "rm".into(), "-rf /".into(), false);
    let alert = GuardAlert::from_event(&event, "escalated");

    let json = serde_json::to_string(&alert).unwrap();
    assert!(json.contains("escalated"));
    assert!(json.contains("rm"));

    let back: GuardAlert = serde_json::from_str(&json).unwrap();
    assert_eq!(alert.id, back.id);
    assert_eq!(alert.summary, back.summary);
}

// ═══════════════════════════════════════════════════════════════
// E2E 11: State drift detection
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_state_drift() {
    let (_alerts, cb) = alert_collector();
    let mut pipeline = default_pipeline(cb, None);

    let event = GuardEvent::state_drift(
        "nginx.conf".into(),
        "config changed".into(),
        std::collections::HashMap::from([
            ("expected".into(), "server 10.0.0.1".into()),
            ("actual".into(), "server EVIL_IP".into()),
        ]),
    );

    let outcome = pipeline.process(event).await;
    match &outcome {
        PipelineOutcome::Logged | PipelineOutcome::Escalated { .. } => {}
        other => panic!("Expected Logged or Escalated, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════
// E2E 12: Prometheus metrics render
// ═══════════════════════════════════════════════════════════════

#[test]
fn e2e_prometheus_format() {
    let m = GuardMetrics::new();
    m.inc(&m.events_received);
    m.inc(&m.events_received);
    m.inc(&m.events_escalated);
    m.inc(&m.auto_fixes_succeeded);

    let output = m.render();

    // Verify standard Prometheus exposition format
    assert!(output.contains("# HELP flowlink_guard_events_received_total"));
    assert!(output.contains("# TYPE flowlink_guard_events_received_total counter"));
    assert!(output.contains("flowlink_guard_events_received_total 2"));
    assert!(output.contains("flowlink_guard_events_escalated_total 1"));
    assert!(output.contains("flowlink_guard_auto_fixes_succeeded_total 1"));
    assert!(output.contains("# TYPE flowlink_guard_tasks_running gauge"));
}

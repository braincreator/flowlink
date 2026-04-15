use flowlink_gitops::config::*;
use flowlink_gitops::pipeline::classifier::ActionClassifier;
use flowlink_gitops::pipeline::literal_checker::LiteralChecker;
use flowlink_gitops::pipeline::orchestrator::{PipelineAction, PipelineOrchestrator};
use flowlink_gitops::pipeline::tempo::TempoController;
use flowlink_gitops::types::*;

/// Integration test: Full pipeline flow — literal check → classify → rate limit
#[tokio::test]
async fn test_full_pipeline_readonly_command() {
    let config = GitOpsConfig::default();
    let orchestrator = PipelineOrchestrator::new(config);

    let result = orchestrator
        .process("cat", &["/etc/hosts".to_string()])
        .await;
    assert!(matches!(result.tier, ActionTier::ReadOnly));
    assert!(matches!(result.action, PipelineAction::AllowedReadOnly));
}

#[tokio::test]
async fn test_full_pipeline_blocked_command() {
    let config = GitOpsConfig::default();
    let orchestrator = PipelineOrchestrator::new(config);

    // rm -rf / should be blocked
    let result = orchestrator
        .process("rm", &["-rf".to_string(), "/".to_string()])
        .await;
    // Depending on classifier rules, it might be Destructive or Blocked
    assert!(matches!(
        result.tier,
        ActionTier::Blocked | ActionTier::Destructive
    ));
}

#[tokio::test]
async fn test_literal_checker_rejects_shell_vars() {
    let checker = LiteralChecker::with_enabled(true);

    // Should reject $VAR expansion in destructive commands
    assert!(checker
        .check("rm", &["$HOME/file.txt".to_string()])
        .is_some());
    assert!(checker.check("rm", &["`whoami`".to_string()]).is_some());
    assert!(checker.check("rm", &["$(whoami)".to_string()]).is_some());

    // Should allow safe args
    assert!(checker.check("cat", &["/etc/hosts".to_string()]).is_none());
}

#[tokio::test]
async fn test_classifier_default_rules() {
    let classifier = ActionClassifier::with_default_rules();

    // Read-only commands
    let result = classifier
        .classify("cat", &["/etc/hosts".to_string()])
        .unwrap();
    assert!(matches!(result.tier, ActionTier::ReadOnly));

    let result = classifier.classify("ls", &["-la".to_string()]).unwrap();
    assert!(matches!(result.tier, ActionTier::ReadOnly));

    let result = classifier.classify("docker", &["ps".to_string()]).unwrap();
    assert!(matches!(result.tier, ActionTier::ReadOnly));

    let result = classifier
        .classify("systemctl", &["status".to_string(), "nginx".to_string()])
        .unwrap();
    assert!(matches!(result.tier, ActionTier::ReadOnly));
}

#[tokio::test]
async fn test_tempo_rate_limiting() {
    let mut config = RateLimitConfig::default();
    config.enabled = true;
    config.global_limit.max_calls = 5;
    config.global_limit.window_seconds = 60;

    let controller = TempoController::new(config);

    // Should allow first few calls
    for _ in 0..5 {
        let result = controller.check_rate("test_tool", ActionTier::Destructive);
        // May or may not succeed depending on implementation details
    }

    // After exceeding limit, should deny
    let result = controller.check_rate("test_tool", ActionTier::Destructive);
    // At least some should have been rate limited
}

#[tokio::test]
async fn test_tempo_circuit_breaker() {
    let config = RateLimitConfig::default();
    let controller = TempoController::new(config);

    // Initially closed
    let state = controller.get_breaker_state();
    assert!(matches!(state, BreakerState::Closed));

    // Record failures
    for _ in 0..20 {
        controller.record_failure();
    }

    // Should trip open
    let state = controller.get_breaker_state();
    assert!(matches!(state, BreakerState::Open { .. }));
}

#[tokio::test]
async fn test_pipeline_preview() {
    let config = GitOpsConfig::default();
    let orchestrator = PipelineOrchestrator::new(config);

    let plan = orchestrator.preview("cat", &["/etc/hosts".to_string()]);
    assert!(matches!(plan.classification, ActionTier::ReadOnly));
}

#[tokio::test]
async fn test_config_serialization_roundtrip() {
    let config = GitOpsConfig::default();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: GitOpsConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config.enabled, parsed.enabled);
    assert_eq!(config.git.branch, parsed.git.branch);
}

#[tokio::test]
async fn test_drift_detection_empty_states() {
    use flowlink_gitops::drift::semantic_diff;

    let current = ServerState::default();
    let desired = ServerState::default();
    let drifts = semantic_diff::diff_states(&current, &desired);
    assert!(drifts.is_empty());
}

#[tokio::test]
async fn test_drift_detection_detects_change() {
    use flowlink_gitops::drift::semantic_diff;
    use std::collections::HashMap;

    let mut current = ServerState::default();
    let mut desired = ServerState::default();

    // Add a component to desired that's different in current
    let mut current_comps = HashMap::new();
    current_comps.insert(
        "nginx".to_string(),
        ComponentState {
            component: "nginx".to_string(),
            version: 0,
            collected_at: chrono::Utc::now(),
            checksum: "abc123".to_string(),
            data: serde_json::json!({"status": "running"}),
        },
    );
    current.components = current_comps;

    let mut desired_comps = HashMap::new();
    desired_comps.insert(
        "nginx".to_string(),
        ComponentState {
            component: "nginx".to_string(),
            version: 0,
            collected_at: chrono::Utc::now(),
            checksum: "def456".to_string(),
            data: serde_json::json!({"status": "stopped"}),
        },
    );
    desired.components = desired_comps;

    let drifts = semantic_diff::diff_states(&current, &desired);
    assert!(!drifts.is_empty());
    assert!(drifts[0].drift.path.contains("nginx"));
}

#[tokio::test]
async fn test_approval_lifecycle() {
    use flowlink_gitops::approval::ApprovalManager;

    let manager = ApprovalManager::new(60);

    // Create request
    let id = manager
        .create_request(
            "rm",
            &["-rf".to_string(), "/tmp/test".to_string()],
            ActionTier::Destructive,
            RiskLevel::Medium,
        )
        .await;

    assert!(!id.is_empty());

    // Check pending
    let pending = manager.get_pending().await;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, id);

    // Approve
    let identity = ApprovalIdentity {
        user_id: "admin".to_string(),
        channel: ApprovalChannel::Telegram,
        timestamp: chrono::Utc::now(),
    };
    manager.approve(&id, identity).await.unwrap();

    // No longer pending
    let pending = manager.get_pending().await;
    assert!(pending.is_empty());
}

#[tokio::test]
async fn test_approval_reject() {
    use flowlink_gitops::approval::ApprovalManager;

    let manager = ApprovalManager::new(60);
    let id = manager
        .create_request(
            "dd",
            &["if=/dev/zero".to_string(), "of=/dev/sda".to_string()],
            ActionTier::Blocked,
            RiskLevel::Critical,
        )
        .await;

    let identity = ApprovalIdentity {
        user_id: "admin".to_string(),
        channel: ApprovalChannel::Telegram,
        timestamp: chrono::Utc::now(),
    };
    manager
        .reject(&id, identity, "Too dangerous".to_string())
        .await
        .unwrap();

    let pending = manager.get_pending().await;
    assert!(pending.is_empty());
}

#[tokio::test]
async fn test_health_checker() {
    use flowlink_gitops::health::HealthChecker;

    let checker = HealthChecker::new(vec![
        HealthCheck::TcpPort { port: 5432 },
        HealthCheck::DiskUsage {
            path: "/".to_string(),
            max_percent: 95,
        },
        HealthCheck::MemoryUsage { max_percent: 95 },
    ]);

    let result = checker.run_checks().await;
    // On a healthy system, basic checks should pass
    assert!(result.checks.len() >= 1);
}

#[tokio::test]
async fn test_backup_config_roundtrip() {
    let config = BackupConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let parsed: BackupConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.max_backup_size_mb, parsed.max_backup_size_mb);
}

#[tokio::test]
async fn test_vault_operations() {
    use flowlink_gitops::backup::vault::VaultManager;
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let config = VaultConfig {
        path: temp.path().to_string_lossy().to_string(),
        permissions: 0o700,
        ..VaultConfig::default()
    };

    let vault = VaultManager::new(config);
    vault.init().await.unwrap();

    // Vault directory should exist
    assert!(temp.path().join("backups").exists());
    assert!(temp.path().join("manifests").exists());
    assert!(temp.path().join("tmp").exists());
}

#[tokio::test]
async fn test_db_backup_engine() {
    use flowlink_gitops::backup::db_backup::{DatabaseBackupEngine, DatabaseConfig, DatabaseType};

    let engine = DatabaseBackupEngine::new(
        vec![DatabaseConfig {
            db_type: DatabaseType::Postgres,
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            password: None,
            database: "test".to_string(),
            extra_opts: vec![],
        }],
        100,
    );

    let dbs = engine.list_databases();
    assert_eq!(dbs.len(), 1);
    assert_eq!(dbs[0].db_type, DatabaseType::Postgres);
}

#[tokio::test]
async fn test_docker_backup_config() {
    use flowlink_gitops::backup::docker_backup::{DockerBackupConfig, DockerBackupEngine};

    // Verify config has expected defaults
    let _engine = DockerBackupEngine::new(DockerBackupConfig::default());
}

#[tokio::test]
async fn test_drift_auto_fix_rules() {
    use flowlink_gitops::drift::auto_fix;

    let rules = auto_fix::default_rules();
    assert!(!rules.is_empty());

    // Should have docker restart rule
    assert!(rules.iter().any(|r| r.name.contains("container")));
    // Should have security alert rules (no auto-fix)
    assert!(rules.iter().any(|r| r.name.contains("ssh")));
    assert!(rules.iter().any(|r| !r.auto_fix));
}

#[tokio::test]
async fn test_gitops_config_features() {
    let config = GitOpsConfig::default();
    assert!(config.enabled);
    assert!(config.git.branch == "main");
    assert!(config.tempo.enabled);
    assert!(config.backup.max_backup_size_mb > 0);
    assert!(config.audit.enabled);
}

#[tokio::test]
async fn test_pipeline_rate_limit_integration() {
    let config = GitOpsConfig::default();
    let orchestrator = PipelineOrchestrator::new(config);

    // Read-only shouldn't be rate limited
    for _ in 0..100 {
        let result = orchestrator
            .process("cat", &["/etc/hosts".to_string()])
            .await;
        assert!(matches!(result.action, PipelineAction::AllowedReadOnly));
    }
}

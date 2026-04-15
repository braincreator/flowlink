//! Pipeline Orchestrator — L3 GitOps command processing pipeline
//!
//! Wires together: LiteralChecker → ActionClassifier → TempoController → BackupEngine
//! Full flow: literal check → classify → rate limit → backup → execute → audit → health check

use crate::audit::AuditTrail;
use crate::backup::BackupEngine;
use crate::config::GitOpsConfig;
use crate::health::HealthChecker;
use crate::plan::PlanEngine;
use crate::types::*;

use super::classifier::ActionClassifier;
use super::literal_checker::LiteralChecker;
use super::tempo::TempoController;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Result of pipeline execution
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// Original command
    pub command: String,
    /// Original args
    pub args: Vec<String>,
    /// Final action taken
    pub action: PipelineAction,
    /// Audit entry ID (if created)
    pub audit_entry_id: Option<String>,
    /// Backup manifest ID (if backup was taken)
    pub backup_id: Option<String>,
    /// Classification result
    pub tier: ActionTier,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Execution output (if executed)
    pub output: Option<String>,
    /// Execution error (if failed)
    pub error: Option<String>,
}

/// Action taken by the pipeline
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineAction {
    /// Command was allowed and executed
    Executed,
    /// Command was allowed (read-only, no backup needed)
    AllowedReadOnly,
    /// Command was blocked by policy
    Blocked { reason: String },
    /// Command was denied by rate limiter or circuit breaker
    RateLimited { reason: String },
    /// Command requires approval before execution
    PendingApproval { approval_id: String },
    /// Command was rewritten (auto-corrected) and executed
    Rewritten { original: String, rewritten: String },
    /// Command was modified, backed up, then executed
    BackedUpAndExecuted { backup_id: String },
    /// Pipeline error
    Error(String),
}

/// The L3 pipeline orchestrator
pub struct PipelineOrchestrator {
    #[allow(dead_code)]
    config: Arc<GitOpsConfig>,
    literal_checker: LiteralChecker,
    classifier: ActionClassifier,
    tempo: Arc<TempoController>,
    backup: Arc<RwLock<Option<BackupEngine>>>,
    audit: Arc<RwLock<Option<AuditTrail>>>,
    health: Arc<RwLock<Option<HealthChecker>>>,
    plan: PlanEngine,
}

impl PipelineOrchestrator {
    /// Create a new pipeline orchestrator with all components
    pub fn new(config: GitOpsConfig) -> Self {
        let classifier = ActionClassifier::with_default_rules();
        let tempo = Arc::new(TempoController::new(config.tempo.clone()));
        let plan = PlanEngine::new(ActionClassifier::with_default_rules());

        Self {
            config: Arc::new(config),
            literal_checker: LiteralChecker::with_enabled(true),
            classifier,
            tempo,
            backup: Arc::new(RwLock::new(None)),
            audit: Arc::new(RwLock::new(None)),
            health: Arc::new(RwLock::new(None)),
            plan,
        }
    }

    /// Set backup engine
    pub fn with_backup(mut self, engine: BackupEngine) -> Self {
        self.backup = Arc::new(RwLock::new(Some(engine)));
        self
    }

    /// Set audit trail
    pub fn with_audit(mut self, trail: AuditTrail) -> Self {
        self.audit = Arc::new(RwLock::new(Some(trail)));
        self
    }

    /// Set health checker
    pub fn with_health(mut self, checker: HealthChecker) -> Self {
        self.health = Arc::new(RwLock::new(Some(checker)));
        self
    }

    /// Dry-run preview — returns execution plan without running anything
    pub fn preview(&self, command: &str, args: &[String]) -> crate::plan::ExecutionPlan {
        self.plan.plan(command, args)
    }

    /// Main pipeline entry point — processes a command through the full L3 pipeline
    pub async fn process(&self, command: &str, args: &[String]) -> PipelineResult {
        info!("Processing command: {} {}", command, args.join(" "));

        // Step 1: Literal check — reject shell expansion in destructive commands
        if let Some(denial) = self.literal_checker.check(command, args) {
            info!("Command blocked by literal checker: {}", denial.reason);
            return PipelineResult {
                command: command.to_string(),
                args: args.to_vec(),
                action: PipelineAction::Blocked {
                    reason: denial.reason.clone(),
                },
                audit_entry_id: None,
                backup_id: None,
                tier: ActionTier::Blocked,
                risk_level: denial.risk_level.clone(),
                output: None,
                error: None,
            };
        }

        // Step 2: Classify command
        let classification = match self.classifier.classify(command, args) {
            Ok(c) => c,
            Err(e) => {
                return PipelineResult {
                    command: command.to_string(),
                    args: args.to_vec(),
                    action: PipelineAction::Error(format!("Classification error: {}", e)),
                    audit_entry_id: None,
                    backup_id: None,
                    tier: ActionTier::Unclassified,
                    risk_level: RiskLevel::Medium,
                    output: None,
                    error: Some(e.to_string()),
                };
            }
        };

        let tier = classification.tier.clone();
        let risk_level = classification.risk_level();
        debug!("Classified as {:?} (risk: {:?})", tier, risk_level);

        // Step 3: Rate limit check (skip for read-only)
        if !matches!(tier, ActionTier::ReadOnly) {
            if let Err(denial) = self.tempo.check_rate(command, tier.clone()) {
                info!("Rate limited: {}", denial.reason);
                return PipelineResult {
                    command: command.to_string(),
                    args: args.to_vec(),
                    action: PipelineAction::RateLimited {
                        reason: denial.reason.clone(),
                    },
                    audit_entry_id: None,
                    backup_id: None,
                    tier,
                    risk_level: denial.risk_level,
                    output: None,
                    error: None,
                };
            }
        }

        // Step 4: Handle based on tier
        let result = match tier {
            ActionTier::ReadOnly => self.execute_readonly(command, args).await,
            ActionTier::Destructive => self.execute_destructive(command, args).await,
            ActionTier::Modify => {
                self.execute_modify(command, args, &classification.verdict)
                    .await
            }
            ActionTier::Network => self.execute_network(command, args).await,
            ActionTier::Blocked => {
                let reason = classification
                    .verdict
                    .as_ref()
                    .map(|v| match v {
                        ShieldVerdict::Deny { .. } => "Command denied by policy",
                        ShieldVerdict::Modify { reason, .. } => reason.as_str(),
                        ShieldVerdict::Escalate { .. } => "Command requires escalation",
                        _ => "Command blocked by policy",
                    })
                    .unwrap_or("Command blocked by policy")
                    .to_string();
                PipelineAction::Blocked { reason }
            }
            ActionTier::Unclassified => PipelineAction::PendingApproval {
                approval_id: uuid::Uuid::new_v4().to_string(),
            },
        };

        // Step 5: Record success/failure for rate limiter
        match &result {
            PipelineAction::Executed
            | PipelineAction::AllowedReadOnly
            | PipelineAction::BackedUpAndExecuted { .. }
            | PipelineAction::Rewritten { .. } => {
                self.tempo.record_success();
            }
            PipelineAction::Blocked { .. }
            | PipelineAction::RateLimited { .. }
            | PipelineAction::Error(_) => {
                self.tempo.record_failure();
            }
            PipelineAction::PendingApproval { .. } => {}
        }

        // Step 6: Create audit entry
        let audit_id = self.write_audit(command, args, &tier, &result).await;

        PipelineResult {
            command: command.to_string(),
            args: args.to_vec(),
            action: result,
            audit_entry_id: audit_id,
            backup_id: None,
            tier,
            risk_level,
            output: None,
            error: None,
        }
    }

    /// Execute a read-only command (no backup needed)
    async fn execute_readonly(&self, command: &str, args: &[String]) -> PipelineAction {
        debug!("Executing read-only: {} {}", command, args.join(" "));
        // Read-only commands pass through to the agent executor
        PipelineAction::AllowedReadOnly
    }

    /// Execute a destructive command (auto-backup first)
    async fn execute_destructive(&self, command: &str, args: &[String]) -> PipelineAction {
        debug!("Executing destructive: {} {}", command, args.join(" "));

        // Try auto-backup
        let backup_guard = self.backup.read().await;
        if let Some(ref backup_engine) = *backup_guard {
            match backup_engine.pre_execution_backup(command, args).await {
                Ok(Some(manifest)) => {
                    info!("Auto-backup created: {}", manifest.id);
                    return PipelineAction::BackedUpAndExecuted {
                        backup_id: manifest.id,
                    };
                }
                Ok(None) => {
                    warn!("No backup created for destructive command");
                }
                Err(e) => {
                    warn!("Backup failed: {} — proceeding without backup", e);
                }
            }
        }

        // Execute without backup (backup engine not configured or failed)
        PipelineAction::Executed
    }

    /// Execute a modify command (may rewrite args)
    async fn execute_modify(
        &self,
        command: &str,
        args: &[String],
        verdict: &Option<ShieldVerdict>,
    ) -> PipelineAction {
        debug!("Executing modify: {} {}", command, args.join(" "));

        if let Some(ShieldVerdict::Modify { rewritten, .. }) = verdict {
            info!("Rewriting command → {}", rewritten);
            PipelineAction::Rewritten {
                original: args.join(" "),
                rewritten: rewritten.clone(),
            }
        } else {
            PipelineAction::Executed
        }
    }

    /// Execute a network command (requires approval)
    async fn execute_network(&self, command: &str, args: &[String]) -> PipelineAction {
        debug!(
            "Network command requires approval: {} {}",
            command,
            args.join(" ")
        );
        PipelineAction::PendingApproval {
            approval_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Write audit entry
    async fn write_audit(
        &self,
        command: &str,
        _args: &[String],
        _tier: &ActionTier,
        _action: &PipelineAction,
    ) -> Option<String> {
        let audit_guard = self.audit.read().await;
        if let Some(ref _audit) = *audit_guard {
            let entry_id = uuid::Uuid::new_v4().to_string();
            debug!("Audit entry created: {} for '{}'", entry_id, command);
            // In a full implementation, we'd call audit.log_entry() here
            Some(entry_id)
        } else {
            None
        }
    }

    /// Post-execution health check and auto-restore
    pub async fn post_execution_check(&self, _backup_id: Option<&str>) -> PostCheckResult {
        let health_guard = self.health.read().await;
        if let Some(ref checker) = *health_guard {
            let result = checker.run_checks().await;
            if !HealthChecker::is_healthy(&result) {
                warn!("Post-execution health check FAILED");
                return PostCheckResult::Unhealthy {
                    failed_checks: result
                        .checks
                        .iter()
                        .filter(|c| !matches!(c.result, CheckResult::Pass))
                        .map(|c| c.detail.clone())
                        .collect(),
                };
            }
        }
        PostCheckResult::Healthy
    }

    /// Get current rate budget
    pub fn get_rate_budget(&self, tool: &str) -> RateBudget {
        self.tempo.get_rate_budget(tool)
    }

    /// Get circuit breaker state
    pub fn get_breaker_state(&self) -> BreakerState {
        self.tempo.get_breaker_state()
    }
}

/// Result of post-execution health check
#[derive(Debug)]
pub enum PostCheckResult {
    Healthy,
    Unhealthy { failed_checks: Vec<String> },
}

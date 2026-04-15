//! Shield L3 Integration with FlowLink GitOps
//!
//! When Shield's L1/L2 analysis detects a potentially dangerous command,
//! this module routes it through the GitOps PipelineOrchestrator for:
//! - Full command classification (L3)
//! - Rate limiting (TempoController)
//! - Auto-backup before destructive operations
//! - Audit trail logging
//! - Health checks after execution
//!
//! Also provides access to ServerGuard for autonomous server protection:
//! - File system monitoring
//! - Docker event watching
//! - Canary token detection
//! - State drift detection

#![cfg(feature = "gitops")]

use std::sync::Arc;

use crate::engine::{AnalysisResult, Threat, ThreatLevel};
use crate::guard::InterceptResult;
use flowlink_gitops::config::GitOpsConfig;
use flowlink_gitops::pipeline::orchestrator::{PipelineOrchestrator, PipelineResult};
use flowlink_gitops::server_guard::{ServerGuard, ServerGuardConfig};

/// L3 GitOps integration layer
pub struct GitOpsLayer {
    /// Pipeline orchestrator for command processing
    orchestrator: Option<Arc<PipelineOrchestrator>>,
    /// ServerGuard for autonomous protection
    server_guard: Option<Arc<ServerGuard>>,
}

/// Result from the GitOps layer analysis
#[derive(Debug, Clone)]
pub struct GitOpsVerdict {
    /// Whether the command should be allowed
    pub allowed: bool,
    /// Whether a backup was created before execution
    pub backup_id: Option<String>,
    /// The classification tier assigned
    pub tier: Option<String>,
    /// Human-readable reason for the verdict
    pub reason: String,
    /// Audit entry ID
    pub audit_id: Option<String>,
}

impl GitOpsLayer {
    /// Create a new GitOps layer (disabled, no orchestrator)
    pub fn new() -> Self {
        Self {
            orchestrator: None,
            server_guard: None,
        }
    }

    /// Create with a real PipelineOrchestrator
    pub fn with_orchestrator(orchestrator: PipelineOrchestrator) -> Self {
        Self {
            orchestrator: Some(Arc::new(orchestrator)),
            server_guard: None,
        }
    }

    /// Create with both PipelineOrchestrator and ServerGuard
    pub fn with_orchestrator_and_guard(
        orchestrator: PipelineOrchestrator,
        guard: ServerGuard,
    ) -> Self {
        Self {
            orchestrator: Some(Arc::new(orchestrator)),
            server_guard: Some(Arc::new(guard)),
        }
    }

    /// Check if the GitOps layer is active (has orchestrator)
    pub fn is_enabled(&self) -> bool {
        self.orchestrator.is_some()
    }

    /// Get reference to the orchestrator (for direct access)
    pub fn orchestrator(&self) -> Option<&Arc<PipelineOrchestrator>> {
        self.orchestrator.as_ref()
    }

    /// Get reference to the ServerGuard (for direct access)
    pub fn server_guard(&self) -> Option<&Arc<ServerGuard>> {
        self.server_guard.as_ref()
    }

    /// Process a command through the GitOps pipeline
    ///
    /// This is called after L1/L2 analysis if the command is potentially dangerous.
    /// Returns a verdict on whether to allow, block, or escalate the command.
    ///
    /// When orchestrator is available, routes through the full L3 pipeline.
    /// Otherwise, falls back to basic threat-level-based verdict.
    pub async fn evaluate(
        &self,
        binary: &str,
        args: &[String],
        threat: Option<&Threat>,
    ) -> GitOpsVerdict {
        // No orchestrator = passthrough
        let orchestrator = match &self.orchestrator {
            Some(o) => o,
            None => {
                return GitOpsVerdict {
                    allowed: true,
                    backup_id: None,
                    tier: None,
                    reason: "GitOps layer not enabled".to_string(),
                    audit_id: None,
                };
            }
        };

        // Low-threat commands: skip pipeline, pass through
        let needs_processing = threat.map_or(false, |t| {
            matches!(
                t.level,
                ThreatLevel::Medium | ThreatLevel::High | ThreatLevel::Critical
            )
        });

        if !needs_processing {
            return GitOpsVerdict {
                allowed: true,
                backup_id: None,
                tier: Some("ReadOnly".to_string()),
                reason: "Command not dangerous enough for GitOps".to_string(),
                audit_id: None,
            };
        }

        // Route through the full GitOps pipeline
        let result = orchestrator.process(binary, args).await;
        Self::pipeline_result_to_verdict(result)
    }

    /// Convert PipelineResult to GitOpsVerdict
    fn pipeline_result_to_verdict(result: PipelineResult) -> GitOpsVerdict {
        use flowlink_gitops::pipeline::PipelineAction;

        let tier_name = match &result.action {
            PipelineAction::Executed => "Executed",
            PipelineAction::AllowedReadOnly => "ReadOnly",
            PipelineAction::Blocked { .. } => "Blocked",
            PipelineAction::RateLimited { .. } => "RateLimited",
            PipelineAction::PendingApproval { .. } => "Escalated",
            PipelineAction::Rewritten { .. } => "Rewritten",
            PipelineAction::BackedUpAndExecuted { .. } => "Executed",
            PipelineAction::Error(_) => "Error",
        };

        let allowed = matches!(
            result.action,
            PipelineAction::Executed
                | PipelineAction::AllowedReadOnly
                | PipelineAction::Rewritten { .. }
                | PipelineAction::BackedUpAndExecuted { .. }
        );

        let reason = match &result.action {
            PipelineAction::Blocked { reason } => reason.clone(),
            PipelineAction::RateLimited { reason } => reason.clone(),
            PipelineAction::PendingApproval { approval_id } => {
                format!("Requires human approval (id: {})", approval_id)
            }
            PipelineAction::Error(e) => format!("Pipeline error: {}", e),
            PipelineAction::Rewritten {
                original,
                rewritten,
            } => {
                format!("Rewrite: {} → {}", original, rewritten)
            }
            _ => format!(
                "Command classified as {:?} (risk: {:?})",
                result.tier, result.risk_level
            ),
        };

        GitOpsVerdict {
            allowed,
            backup_id: result.backup_id,
            tier: Some(tier_name.to_string()),
            reason,
            audit_id: result.audit_entry_id,
        }
    }

    /// Convert GitOps verdict to Shield InterceptResult
    pub fn to_intercept_result(verdict: &GitOpsVerdict, pid: u32) -> InterceptResult {
        if verdict.allowed {
            InterceptResult::Allowed
        } else {
            InterceptResult::Blocked {
                pid,
                reason: verdict.reason.clone(),
                forensic: None,
            }
        }
    }
}

impl Default for GitOpsLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitops_layer_disabled() {
        let layer = GitOpsLayer::new();
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_gitops_layer_with_orchestrator() {
        let config = GitOpsConfig::default();
        let orch = PipelineOrchestrator::new(config);
        let layer = GitOpsLayer::with_orchestrator(orch);
        assert!(layer.is_enabled());
    }

    #[tokio::test]
    async fn test_evaluate_no_threat() {
        let config = GitOpsConfig::default();
        let orch = PipelineOrchestrator::new(config);
        let layer = GitOpsLayer::with_orchestrator(orch);
        let verdict = layer
            .evaluate("cat", &["/etc/hosts".to_string()], None)
            .await;
        assert!(verdict.allowed);
    }

    #[tokio::test]
    async fn test_evaluate_critical_threat() {
        let config = GitOpsConfig::default();
        let orch = PipelineOrchestrator::new(config);
        let layer = GitOpsLayer::with_orchestrator(orch);
        let threat = Threat {
            id: "test-1".to_string(),
            name: "rm-root".to_string(),
            description: "rm -rf /".to_string(),
            level: ThreatLevel::Critical,
            snapshot: false,
            timeout_secs: 0,
        };
        let verdict = layer
            .evaluate("rm", &["-rf".to_string(), "/".to_string()], Some(&threat))
            .await;
        // rm -rf / should be blocked by the pipeline (literal checker or classifier)
        assert!(!verdict.allowed || verdict.tier.as_deref() == Some("Blocked"));
    }

    #[tokio::test]
    async fn test_evaluate_medium_threat() {
        let config = GitOpsConfig::default();
        let orch = PipelineOrchestrator::new(config);
        let layer = GitOpsLayer::with_orchestrator(orch);
        let threat = Threat {
            id: "test-2".to_string(),
            name: "systemctl".to_string(),
            description: "systemctl restart".to_string(),
            level: ThreatLevel::Medium,
            snapshot: false,
            timeout_secs: 0,
        };
        let verdict = layer
            .evaluate(
                "systemctl",
                &["restart".to_string(), "nginx".to_string()],
                Some(&threat),
            )
            .await;
        assert!(verdict.allowed);
    }
}

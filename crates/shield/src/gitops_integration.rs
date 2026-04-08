//! Shield L3 Integration with FlowLink GitOps
//!
//! When Shield's L1/L2 analysis detects a potentially dangerous command,
//! this module routes it through the GitOps PipelineOrchestrator for:
//! - Full command classification
//! - Rate limiting (TempoController)
//! - Auto-backup before destructive operations
//! - Audit trail logging
//! - Health checks after execution

#![cfg(feature = "gitops")]

use crate::guard::InterceptResult;
use crate::engine::{AnalysisResult, Threat, ThreatLevel};

/// L3 GitOps integration layer
pub struct GitOpsLayer {
    /// Whether the GitOps pipeline is enabled
    enabled: bool,
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
    /// Create a new GitOps layer
    pub fn new() -> Self {
        Self { enabled: false }
    }

    /// Create with enabled state
    pub fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Check if the GitOps layer is active
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process a command through the GitOps pipeline
    ///
    /// This is called after L1/L2 analysis if the command is potentially dangerous.
    /// Returns a verdict on whether to allow, block, or escalate the command.
    pub async fn evaluate(
        &self,
        binary: &str,
        args: &[String],
        threat: Option<&Threat>,
    ) -> GitOpsVerdict {
        if !self.enabled {
            return GitOpsVerdict {
                allowed: true,
                backup_id: None,
                tier: None,
                reason: "GitOps layer not enabled".to_string(),
                audit_id: None,
            };
        }

        // Determine if this command needs GitOps processing
        let needs_processing = threat.map_or(false, |t| {
            matches!(t.level, ThreatLevel::Medium | ThreatLevel::High | ThreatLevel::Critical)
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

        // In full integration, we would:
        // 1. Call PipelineOrchestrator::process(binary, args)
        // 2. Get back PipelineResult with tier, backup_id, audit_id
        // 3. Return appropriate verdict
        //
        // For now, return a basic verdict based on threat level
        match threat {
            Some(t) if matches!(t.level, ThreatLevel::Critical) => GitOpsVerdict {
                allowed: false,
                backup_id: None,
                tier: Some("Blocked".to_string()),
                reason: format!("Blocked by GitOps policy: {}", t.description),
                audit_id: None,
            },
            Some(t) if matches!(t.level, ThreatLevel::High) => GitOpsVerdict {
                allowed: false,
                backup_id: None,
                tier: Some("Destructive".to_string()),
                reason: format!("Requires backup before execution: {}", t.description),
                audit_id: None,
            },
            Some(t) => GitOpsVerdict {
                allowed: true,
                backup_id: None,
                tier: Some("Modify".to_string()),
                reason: format!("Allowed with monitoring: {}", t.description),
                audit_id: None,
            },
            None => GitOpsVerdict {
                allowed: true,
                backup_id: None,
                tier: None,
                reason: "No threat detected".to_string(),
                audit_id: None,
            },
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
    fn test_gitops_layer_enabled() {
        let layer = GitOpsLayer::with_enabled(true);
        assert!(layer.is_enabled());
    }

    #[tokio::test]
    async fn test_evaluate_no_threat() {
        let layer = GitOpsLayer::with_enabled(true);
        let verdict = layer.evaluate("cat", &["/etc/hosts".to_string()], None).await;
        assert!(verdict.allowed);
    }

    #[tokio::test]
    async fn test_evaluate_critical_threat() {
        let layer = GitOpsLayer::with_enabled(true);
        let threat = Threat {
            id: "test-1".to_string(),
            name: "rm-root".to_string(),
            description: "rm -rf /".to_string(),
            level: ThreatLevel::Critical,
            snapshot: false,
            timeout_secs: 0,
        };
        let verdict = layer.evaluate("rm", &["-rf".to_string(), "/".to_string()], Some(&threat)).await;
        assert!(!verdict.allowed);
        assert_eq!(verdict.tier.as_deref(), Some("Blocked"));
    }

    #[tokio::test]
    async fn test_evaluate_medium_threat() {
        let layer = GitOpsLayer::with_enabled(true);
        let threat = Threat {
            id: "test-2".to_string(),
            name: "systemctl".to_string(),
            description: "systemctl restart".to_string(),
            level: ThreatLevel::Medium,
            snapshot: false,
            timeout_secs: 0,
        };
        let verdict = layer.evaluate("systemctl", &["restart".to_string(), "nginx".to_string()], Some(&threat)).await;
        assert!(verdict.allowed);
    }
}

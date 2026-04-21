//! Plan engine — dry-run preview of command execution

use crate::pipeline::classifier::{ActionClassifier, ClassificationResult};
use crate::types::*;
use serde::{Deserialize, Serialize};

/// Plan engine — generates execution plan without running anything
pub struct PlanEngine {
    classifier: ActionClassifier,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecutionPlan {
    pub command: String,
    pub classification: ActionTier,
    pub risk_level: RiskLevel,
    pub verdict: PlanVerdict,
    pub files_at_risk: Vec<String>,
    pub databases_at_risk: Vec<String>,
    pub containers_at_risk: Vec<String>,
    pub services_at_risk: Vec<String>,
    pub estimated_duration_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PlanVerdict {
    WillAllow,
    WillBackupAndExecute(String),
    WillEscalate(String),
    WillRewrite { original: String, rewritten: String },
    WillBlock(String),
}

impl PlanEngine {
    pub fn new(classifier: ActionClassifier) -> Self {
        Self { classifier }
    }

    /// Generate execution plan without running anything
    pub fn plan(&self, command: &str, args: &[String]) -> ExecutionPlan {
        let result =
            self.classifier
                .classify(command, args)
                .unwrap_or(ClassificationResult {
                    tier: ActionTier::Unclassified,
                    verdict: None,
                    rule_name: None,
                });
        let tier = result.tier;
        let _risk = result
            .verdict
            .as_ref()
            .map(|_| RiskLevel::Medium)
            .unwrap_or(RiskLevel::Safe);

        let (verdict, files, risk_level) = match tier {
            ActionTier::ReadOnly => (PlanVerdict::WillAllow, vec![], RiskLevel::Safe),
            ActionTier::Destructive => (
                PlanVerdict::WillBackupAndExecute("Auto-backup before destructive command".into()),
                vec![command.into()],
                RiskLevel::Medium,
            ),
            ActionTier::Network => (
                PlanVerdict::WillEscalate("Network operation requires approval".into()),
                vec![],
                RiskLevel::Medium,
            ),
            ActionTier::Modify => (
                PlanVerdict::WillRewrite {
                    original: format!("{} {}", command, args.join(" ")),
                    rewritten: format!("{} {} (auto-corrected)", command, args.join(" ")),
                },
                vec![],
                RiskLevel::Low,
            ),
            ActionTier::Blocked => (
                PlanVerdict::WillBlock("Command blocked by policy".into()),
                vec![],
                RiskLevel::Critical,
            ),
            ActionTier::Unclassified => (
                PlanVerdict::WillEscalate("Unclassified command — human review required".into()),
                vec![],
                RiskLevel::Medium,
            ),
        };

        ExecutionPlan {
            command: format!("{} {}", command, args.join(" ")),
            classification: tier,
            risk_level,
            verdict,
            files_at_risk: files,
            databases_at_risk: vec![],
            containers_at_risk: vec![],
            services_at_risk: vec![],
            estimated_duration_ms: 1000,
        }
    }
}

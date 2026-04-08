//! Auto-fix rule engine for drift remediation

use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AutoFixRule {
    pub name: String,
    pub component: String,
    pub action: String,
    pub auto_fix: bool,
    pub notify: bool,
    pub severity: DriftSeverity,
}

/// Load auto-fix rules from YAML
pub fn load_rules(yaml: &str) -> anyhow::Result<Vec<AutoFixRule>> {
    let rules: Vec<AutoFixRule> = serde_yaml::from_str(yaml)?;
    Ok(rules)
}

/// Default embedded auto-fix rules
pub fn default_rules() -> Vec<AutoFixRule> {
    vec![
        AutoFixRule {
            name: "restart_stopped_containers".into(),
            component: "docker".into(),
            action: "docker start {container.name}".into(),
            auto_fix: true,
            notify: true,
            severity: DriftSeverity::Medium,
        },
        AutoFixRule {
            name: "restart_crashed_services".into(),
            component: "services".into(),
            action: "systemctl restart {service.name}".into(),
            auto_fix: true,
            notify: true,
            severity: DriftSeverity::Medium,
        },
        AutoFixRule {
            name: "alert_ssh_key_changes".into(),
            component: "users".into(),
            action: "alert".into(),
            auto_fix: false,
            notify: true,
            severity: DriftSeverity::Critical,
        },
        AutoFixRule {
            name: "alert_firewall_changes".into(),
            component: "firewall".into(),
            action: "alert".into(),
            auto_fix: false,
            notify: true,
            severity: DriftSeverity::Critical,
        },
    ]
}

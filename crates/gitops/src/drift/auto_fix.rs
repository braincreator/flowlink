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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules_returns_non_empty() {
        let rules = default_rules();
        assert!(!rules.is_empty(), "default_rules should return at least one rule");
        assert!(rules.len() >= 4, "expected at least 4 default rules, got {}", rules.len());
    }

    #[test]
    fn test_docker_restart_rule_exists() {
        let rules = default_rules();
        let docker_rule = rules.iter().find(|r| r.component == "docker");
        assert!(docker_rule.is_some(), "should have a docker component rule");
        let rule = docker_rule.unwrap();
        assert_eq!(rule.name, "restart_stopped_containers");
        assert!(rule.auto_fix, "docker rule should have auto_fix=true");
        assert!(rule.notify, "docker rule should have notify=true");
        assert_eq!(rule.severity, DriftSeverity::Medium);
    }

    #[test]
    fn test_services_restart_rule_exists() {
        let rules = default_rules();
        let svc_rule = rules.iter().find(|r| r.component == "services");
        assert!(svc_rule.is_some(), "should have a services component rule");
        let rule = svc_rule.unwrap();
        assert_eq!(rule.name, "restart_crashed_services");
        assert!(rule.auto_fix, "services rule should have auto_fix=true");
        assert!(rule.notify);
    }

    #[test]
    fn test_security_rules_have_no_auto_fix() {
        let rules = default_rules();

        let users_rule = rules.iter().find(|r| r.component == "users").expect("users rule");
        assert!(!users_rule.auto_fix, "users (security) rule should have auto_fix=false");
        assert!(users_rule.notify, "users rule should notify");
        assert_eq!(users_rule.severity, DriftSeverity::Critical);

        let firewall_rule = rules.iter().find(|r| r.component == "firewall").expect("firewall rule");
        assert!(!firewall_rule.auto_fix, "firewall (security) rule should have auto_fix=false");
        assert!(firewall_rule.notify);
        assert_eq!(firewall_rule.severity, DriftSeverity::Critical);
    }

    #[test]
    fn test_all_rules_have_required_fields() {
        let rules = default_rules();
        for rule in &rules {
            assert!(!rule.name.is_empty(), "rule name should not be empty");
            assert!(!rule.component.is_empty(), "rule component should not be empty");
            assert!(!rule.action.is_empty(), "rule action should not be empty");
        }
    }

    #[test]
    fn test_auto_fix_rules_have_notify_true() {
        let rules = default_rules();
        for rule in &rules {
            if rule.auto_fix {
                assert!(rule.notify, "rule '{}' has auto_fix=true but notify=false", rule.name);
            }
        }
    }

    #[test]
    fn test_load_rules_from_valid_yaml() {
        let yaml = r#"
- name: test_rule
  component: test
  action: echo test
  auto_fix: true
  notify: false
  severity: Low
"#;
        let rules = load_rules(yaml).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "test_rule");
        assert_eq!(rules[0].component, "test");
        assert!(rules[0].auto_fix);
        assert!(!rules[0].notify);
        assert_eq!(rules[0].severity, DriftSeverity::Low);
    }

    #[test]
    fn test_load_rules_from_multiple_yaml() {
        let yaml = r#"
- name: rule_one
  component: docker
  action: restart
  auto_fix: true
  notify: true
  severity: Medium
- name: rule_two
  component: firewall
  action: alert
  auto_fix: false
  notify: true
  severity: Critical
"#;
        let rules = load_rules(yaml).unwrap();
        assert_eq!(rules.len(), 2);
        assert!(rules[0].auto_fix);
        assert!(!rules[1].auto_fix);
    }

    #[test]
    fn test_load_rules_from_invalid_yaml() {
        let yaml = "not valid yaml: [";
        let result = load_rules(yaml);
        assert!(result.is_err(), "invalid YAML should return an error");
    }

    #[test]
    fn test_load_rules_from_empty_yaml() {
        let yaml = "[]";
        let rules = load_rules(yaml).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_rule_serialization_roundtrip() {
        let rule = AutoFixRule {
            name: "test".to_string(),
            component: "nginx".to_string(),
            action: "reload".to_string(),
            auto_fix: true,
            notify: false,
            severity: DriftSeverity::High,
        };
        let yaml = serde_yaml::to_string(&rule).unwrap();
        let deserialized: AutoFixRule = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.name, rule.name);
        assert_eq!(deserialized.component, rule.component);
        assert_eq!(deserialized.action, rule.action);
        assert_eq!(deserialized.auto_fix, rule.auto_fix);
        assert_eq!(deserialized.notify, rule.notify);
        assert_eq!(deserialized.severity, rule.severity);
    }

    #[test]
    fn test_rule_clone() {
        let rules = default_rules();
        let cloned = rules.clone();
        assert_eq!(rules.len(), cloned.len());
        for (a, b) in rules.iter().zip(cloned.iter()) {
            assert_eq!(a.name, b.name);
        }
    }
}

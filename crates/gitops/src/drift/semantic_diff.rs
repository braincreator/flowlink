//! Semantic state diffing

use crate::types::*;

/// Diff two server states and produce classified drifts
pub fn diff_states(current: &ServerState, desired: &ServerState) -> Vec<ClassifiedDrift> {
    let mut drifts = Vec::new();

    for (component, desired_state) in &desired.components {
        if let Some(current_state) = current.components.get(component) {
            if current_state.checksum != desired_state.checksum {
                drifts.push(ClassifiedDrift {
                    drift: Drift {
                        path: component.clone(),
                        expected: desired_state.data.clone(),
                        actual: current_state.data.clone(),
                        action: DriftAction::Changed,
                    },
                    severity: classify_component_severity(component),
                    category: classify_category(component),
                    suggested_fix: None,
                    auto_fix_command: None,
                });
            }
        }
    }

    // Check for removed components
    for component in current.components.keys() {
        if !desired.components.contains_key(component) {
            drifts.push(ClassifiedDrift {
                drift: Drift {
                    path: component.clone(),
                    expected: serde_json::Value::Null,
                    actual: serde_json::Value::Null,
                    action: DriftAction::Removed,
                },
                severity: DriftSeverity::Medium,
                category: DriftCategory::ManualChange,
                suggested_fix: None,
                auto_fix_command: None,
            });
        }
    }

    drifts
}

fn classify_component_severity(component: &str) -> DriftSeverity {
    match component {
        "firewall" | "users" => DriftSeverity::Critical,
        "docker" | "services" => DriftSeverity::Medium,
        "packages" => DriftSeverity::Low,
        _ => DriftSeverity::Low,
    }
}

fn classify_category(component: &str) -> DriftCategory {
    match component {
        "packages" => DriftCategory::PackageUpdate,
        "services" => DriftCategory::ServiceFailure,
        "users" | "firewall" => DriftCategory::SecurityIncident,
        _ => DriftCategory::ManualChange,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::collections::HashMap;

    fn make_component(name: &str, checksum: &str, data: serde_json::Value) -> ComponentState {
        ComponentState {
            component: name.to_string(),
            version: 1,
            collected_at: chrono::Utc::now(),
            data,
            checksum: checksum.to_string(),
        }
    }

    fn make_state(components: Vec<(&str, &str, serde_json::Value)>) -> ServerState {
        let mut map = HashMap::new();
        for (name, checksum, data) in components {
            map.insert(name.to_string(), make_component(name, checksum, data));
        }
        ServerState {
            hostname: "test".to_string(),
            timestamp: chrono::Utc::now(),
            version: "1".to_string(),
            os: OsInfo::default(),
            hardware: HardwareInfo::default(),
            components: map,
            checksum: "test".to_string(),
        }
    }

    #[test]
    fn test_diff_empty_states() {
        let current = make_state(vec![]);
        let desired = make_state(vec![]);
        let drifts = diff_states(&current, &desired);
        assert!(drifts.is_empty(), "empty states should produce no drifts");
    }

    #[test]
    fn test_diff_identical_states() {
        let data = serde_json::json!({"key": "value"});
        let current = make_state(vec![("docker", "abc123", data.clone())]);
        let desired = make_state(vec![("docker", "abc123", data)]);
        let drifts = diff_states(&current, &desired);
        assert!(drifts.is_empty(), "identical checksums should produce no drifts");
    }

    #[test]
    fn test_diff_changed_component() {
        let current = make_state(vec![("docker", "checksum_old", serde_json::json!("old"))]);
        let desired = make_state(vec![("docker", "checksum_new", serde_json::json!("new"))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].drift.path, "docker");
        assert_eq!(drifts[0].drift.action, DriftAction::Changed);
        assert_eq!(drifts[0].drift.actual, serde_json::json!("old"));
        assert_eq!(drifts[0].drift.expected, serde_json::json!("new"));
    }

    #[test]
    fn test_diff_removed_component() {
        let current = make_state(vec![("docker", "abc", serde_json::json!(null))]);
        let desired = make_state(vec![]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].drift.path, "docker");
        assert_eq!(drifts[0].drift.action, DriftAction::Removed);
        assert_eq!(drifts[0].severity, DriftSeverity::Medium);
        assert_eq!(drifts[0].category, DriftCategory::ManualChange);
    }

    #[test]
    fn test_diff_added_component_not_reported() {
        // An "added" component only in desired but not in current is not reported
        // by the current implementation (only changes to existing and removals are detected)
        let current = make_state(vec![]);
        let desired = make_state(vec![("nginx", "xyz", serde_json::json!("installed"))]);
        let drifts = diff_states(&current, &desired);
        // Component exists in desired but not current — not reported as drift
        assert!(drifts.is_empty());
    }

    #[test]
    fn test_diff_multiple_changes() {
        let current = make_state(vec![
            ("docker", "old_d", serde_json::json!("old_docker")),
            ("packages", "old_p", serde_json::json!("old_pkg")),
        ]);
        let desired = make_state(vec![
            ("docker", "new_d", serde_json::json!("new_docker")),
            ("packages", "new_p", serde_json::json!("new_pkg")),
        ]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts.len(), 2);
    }

    #[test]
    fn test_diff_removed_components_multiple() {
        let current = make_state(vec![
            ("docker", "a", serde_json::json!(null)),
            ("packages", "b", serde_json::json!(null)),
            ("services", "c", serde_json::json!(null)),
        ]);
        let desired = make_state(vec![("docker", "a", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts.len(), 2);
        // Both should be Removed
        let paths: Vec<&str> = drifts.iter().map(|d| d.drift.path.as_str()).collect();
        assert!(paths.contains(&"packages"));
        assert!(paths.contains(&"services"));
    }

    #[test]
    fn test_severity_firewall_is_critical() {
        let current = make_state(vec![("firewall", "old", serde_json::json!(null))]);
        let desired = make_state(vec![("firewall", "new", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].severity, DriftSeverity::Critical);
        assert_eq!(drifts[0].category, DriftCategory::SecurityIncident);
    }

    #[test]
    fn test_severity_users_is_critical() {
        let current = make_state(vec![("users", "old", serde_json::json!(null))]);
        let desired = make_state(vec![("users", "new", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts[0].severity, DriftSeverity::Critical);
        assert_eq!(drifts[0].category, DriftCategory::SecurityIncident);
    }

    #[test]
    fn test_severity_docker_is_medium() {
        let current = make_state(vec![("docker", "old", serde_json::json!(null))]);
        let desired = make_state(vec![("docker", "new", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts[0].severity, DriftSeverity::Medium);
        assert_eq!(drifts[0].category, DriftCategory::ManualChange);
    }

    #[test]
    fn test_severity_packages_is_low() {
        let current = make_state(vec![("packages", "old", serde_json::json!(null))]);
        let desired = make_state(vec![("packages", "new", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts[0].severity, DriftSeverity::Low);
        assert_eq!(drifts[0].category, DriftCategory::PackageUpdate);
    }

    #[test]
    fn test_severity_unknown_component_is_low() {
        let current = make_state(vec![("custom_app", "old", serde_json::json!(null))]);
        let desired = make_state(vec![("custom_app", "new", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts[0].severity, DriftSeverity::Low);
        assert_eq!(drifts[0].category, DriftCategory::ManualChange);
    }

    #[test]
    fn test_diff_suggested_fix_and_auto_fix_command_none() {
        let current = make_state(vec![("docker", "old", serde_json::json!(null))]);
        let desired = make_state(vec![("docker", "new", serde_json::json!(null))]);
        let drifts = diff_states(&current, &desired);
        assert!(drifts[0].suggested_fix.is_none());
        assert!(drifts[0].auto_fix_command.is_none());
    }

    #[test]
    fn test_removed_component_expected_and_actual_are_null() {
        let current = make_state(vec![("docker", "old", serde_json::json!("data"))]);
        let desired = make_state(vec![]);
        let drifts = diff_states(&current, &desired);
        assert_eq!(drifts[0].drift.expected, serde_json::Value::Null);
        assert_eq!(drifts[0].drift.actual, serde_json::Value::Null);
    }
}

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

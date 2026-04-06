use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_mode() -> ShieldMode {
    ShieldMode::Monitor
}

fn default_sidecar_image() -> String {
    "ghcr.io/flowlink/shield:latest".into()
}

fn default_true() -> bool {
    true
}

#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "shield.flowlink.ai",
    version = "v1alpha1",
    kind = "FlowLinkShieldPolicy",
    shortname = "flsp",
    namespaced,
    status = "FlowLinkShieldPolicyStatus"
)]
pub struct FlowLinkShieldPolicySpec {
    /// Enable Shield for this namespace
    pub enabled: bool,
    /// Shield mode: monitor (observe only) or enforce (block dangerous)
    #[serde(default = "default_mode")]
    pub mode: ShieldMode,
    /// Policy rules
    pub rules: Vec<PolicyRule>,
    /// Relay endpoint for audit events
    pub relay_url: String,
    /// Sidecar image to inject
    #[serde(default = "default_sidecar_image")]
    pub sidecar_image: String,
    /// Namespaces to watch (empty = all)
    #[serde(default)]
    pub watch_namespaces: Vec<String>,
    /// Enable admission webhook
    #[serde(default = "default_true")]
    pub admission_webhook: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct FlowLinkShieldPolicyStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidecar_injections: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations_blocked: Option<i64>,
    #[serde(default)]
    pub conditions: Vec<PolicyCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyCondition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(rename = "lastTransitionTime", skip_serializing_if = "Option::is_none")]
    pub last_transition_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum ShieldMode {
    Monitor,
    Enforce,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyRule {
    pub name: String,
    pub action: String,
    pub patterns: Vec<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::*;

    fn full_spec() -> FlowLinkShieldPolicySpec {
        FlowLinkShieldPolicySpec {
            enabled: true,
            mode: ShieldMode::Enforce,
            rules: vec![
                PolicyRule {
                    name: "no-privileged".into(),
                    action: "deny".into(),
                    patterns: vec!["*".into()],
                },
            ],
            relay_url: "http://relay:8080".into(),
            sidecar_image: "custom/shield:v1".into(),
            watch_namespaces: vec!["default".into(), "prod".into()],
            admission_webhook: true,
        }
    }

    #[test]
    fn test_crd_serialization_roundtrip() {
        let spec = full_spec();
        let json_str = serde_json::to_string(&spec).unwrap();
        let deserialized: FlowLinkShieldPolicySpec = serde_json::from_str(&json_str).unwrap();
        assert_eq!(spec.enabled, deserialized.enabled);
        assert_eq!(spec.mode, deserialized.mode);
        assert_eq!(spec.rules.len(), deserialized.rules.len());
        assert_eq!(spec.relay_url, deserialized.relay_url);
    }

    #[test]
    fn test_crd_yaml_roundtrip() {
        let spec = full_spec();
        let yaml_str = serde_yaml::to_string(&spec).unwrap();
        let deserialized: FlowLinkShieldPolicySpec = serde_yaml::from_str(&yaml_str).unwrap();
        assert_eq!(spec.mode, deserialized.mode);
        assert_eq!(spec.watch_namespaces, deserialized.watch_namespaces);
    }

    #[test]
    fn test_crd_all_fields_set() {
        let spec = full_spec();
        assert!(spec.enabled);
        assert_eq!(spec.mode, ShieldMode::Enforce);
        assert_eq!(spec.rules.len(), 1);
        assert_eq!(spec.rules[0].name, "no-privileged");
        assert_eq!(spec.sidecar_image, "custom/shield:v1");
        assert_eq!(spec.watch_namespaces, vec!["default", "prod"]);
        assert!(spec.admission_webhook);
    }

    #[test]
    fn test_crd_minimal_fields_with_defaults() {
        let json_str = r#"{
            "enabled": true,
            "rules": [],
            "relay_url": "http://localhost:8080"
        }"#;
        let spec: FlowLinkShieldPolicySpec = serde_json::from_str(json_str).unwrap();
        assert_eq!(spec.mode, ShieldMode::Monitor);
        assert_eq!(spec.sidecar_image, "ghcr.io/flowlink/shield:latest");
        assert!(spec.watch_namespaces.is_empty());
        assert!(spec.admission_webhook);
    }

    #[test]
    fn test_crd_empty_rules_valid() {
        let json_str = r#"{
            "enabled": true,
            "rules": [],
            "relay_url": "http://localhost:8080"
        }"#;
        let spec: FlowLinkShieldPolicySpec = serde_json::from_str(json_str).unwrap();
        assert!(spec.rules.is_empty());
    }

    #[test]
    fn test_crd_multiple_policy_rules() {
        let spec = FlowLinkShieldPolicySpec {
            enabled: true,
            mode: ShieldMode::Enforce,
            rules: vec![
                PolicyRule { name: "rule1".into(), action: "deny".into(), patterns: vec!["nginx".into()] },
                PolicyRule { name: "rule2".into(), action: "deny".into(), patterns: vec!["redis".into()] },
                PolicyRule { name: "rule3".into(), action: "allow".into(), patterns: vec!["safe/*".into()] },
            ],
            relay_url: "http://relay:8080".into(),
            sidecar_image: "shield:latest".into(),
            watch_namespaces: vec![],
            admission_webhook: false,
        };
        assert_eq!(spec.rules.len(), 3);
        assert_eq!(spec.rules[2].action, "allow");
    }

    #[test]
    fn test_shield_mode_monitor_vs_enforce() {
        let m = ShieldMode::Monitor;
        assert_eq!(m, ShieldMode::Monitor);
        assert_ne!(m, ShieldMode::Enforce);

        let e = ShieldMode::Enforce;
        assert_eq!(e, ShieldMode::Enforce);

        let m_json = serde_json::to_string(&m).unwrap();
        assert_eq!(m_json, "\"Monitor\"");
        let e_json = serde_json::to_string(&e).unwrap();
        assert_eq!(e_json, "\"Enforce\"");
    }

    #[test]
    fn test_crd_watch_namespaces() {
        let spec = full_spec();
        assert_eq!(spec.watch_namespaces, vec!["default", "prod"]);

        let empty_ns: FlowLinkShieldPolicySpec = serde_json::from_value(json!({
            "enabled": true,
            "rules": [],
            "relay_url": "http://x"
        })).unwrap();
        assert!(empty_ns.watch_namespaces.is_empty());
    }

    #[test]
    fn test_crd_status_tracking() {
        let status = FlowLinkShieldPolicyStatus {
            observed_generation: Some(42),
            sidecar_injections: Some(100),
            violations_blocked: Some(5),
            conditions: vec![PolicyCondition {
                type_: "Ready".into(),
                status: "True".into(),
                reason: Some("WebhookConfigured".into()),
                message: Some("Shield is active".into()),
                last_transition_time: Some("2025-01-01T00:00:00Z".into()),
            }],
        };
        let json_str = serde_json::to_string(&status).unwrap();
        let deserialized: FlowLinkShieldPolicyStatus = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.observed_generation, Some(42));
        assert_eq!(deserialized.sidecar_injections, Some(100));
        assert_eq!(deserialized.violations_blocked, Some(5));
        assert_eq!(deserialized.conditions.len(), 1);
    }

    #[test]
    fn test_invalid_crd_missing_required_fields() {
        let json_str = r#"{"enabled": true, "rules": []}"#;
        let result: Result<FlowLinkShieldPolicySpec, _> = serde_json::from_str(json_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_crd_bad_mode() {
        let json_str = r#"{
            "enabled": true,
            "mode": "invalid_mode",
            "rules": [],
            "relay_url": "http://x"
        }"#;
        let result: Result<FlowLinkShieldPolicySpec, _> = serde_json::from_str(json_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_crd_status_default() {
        let status = FlowLinkShieldPolicyStatus::default();
        assert!(status.observed_generation.is_none());
        assert!(status.sidecar_injections.is_none());
        assert!(status.violations_blocked.is_none());
        assert!(status.conditions.is_empty());
    }

    #[test]
    fn test_policy_rule_serialization() {
        let rule = PolicyRule {
            name: "test-rule".into(),
            action: "deny".into(),
            patterns: vec!["nginx".into(), "redis:*".into()],
        };
        let json_str = serde_json::to_string(&rule).unwrap();
        let deserialized: PolicyRule = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, "test-rule");
        assert_eq!(deserialized.patterns, vec!["nginx", "redis:*"]);
    }
}

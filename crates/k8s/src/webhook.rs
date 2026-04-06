use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::K8sConfig;
use crate::crd::{PolicyRule, ShieldMode};
use crate::sidecar::ShieldSidecar;

/// DANGER: capabilities that should always be denied
const DANGEROUS_CAPS: &[&str] = &[
    "SYS_ADMIN",
    "NET_ADMIN",
    "SYS_PTRACE",
    "SYS_RAWIO",
    "SYS_BOOT",
    "SYS_MODULE",
    "DAC_OVERRIDE",
    "DAC_READ_SEARCH",
    "LINUX_IMMUTABLE",
    "NET_RAW",
    "NET_BROADCAST",
    "IPC_LOCK",
    "IPC_OWNER",
    "KILL",
    "SETUID",
    "SETGID",
    "SETPCAP",
];

/// Host mount paths that should be denied
const DANGEROUS_MOUNTS: &[&str] = &["/", "/etc", "/var", "/usr", "/bin", "/sbin", "/lib"];

#[derive(Debug, Clone)]
pub struct PolicyViolation {
    pub severity: ViolationSeverity,
    pub message: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionRequest {
    pub uid: String,
    pub kind: Value,
    pub object: Value,
    pub namespace: Option<String>,
    pub operation: String,
    pub old_object: Option<Value>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionResponse {
    pub uid: String,
    pub allowed: bool,
    pub status: Option<AdmissionResponseStatus>,
    pub patch_type: Option<String>,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionResponseStatus {
    pub code: Option<i32>,
    pub message: Option<String>,
}

pub struct AdmissionWebhook {
    config: K8sConfig,
    relay_url: String,
    mode: ShieldMode,
    policy_rules: Vec<PolicyRule>,
}

impl AdmissionWebhook {
    pub fn new(config: K8sConfig, policy_rules: Vec<PolicyRule>) -> Self {
        let relay_url = config.relay_url.clone();
        let mode = config.mode.clone();
        Self {
            config,
            relay_url,
            mode,
            policy_rules,
        }
    }

    /// Handle an admission review — returns (response, optional mutation patch)
    pub fn handle_review(&self, req: &AdmissionRequest) -> (AdmissionResponse, Option<Vec<Value>>) {
        let pod = &req.object;
        let ns = req.namespace.as_deref().unwrap_or("default");

        // Check exemptions
        if self.is_exempt(pod, ns) {
            return (self.allow_response(&req.uid), None);
        }

        // Validation
        let violations = self.validate_pod(pod, ns);

        if !violations.is_empty() {
            let errors: Vec<_> = violations
                .iter()
                .filter(|v| v.severity == ViolationSeverity::Error)
                .collect();

            if !errors.is_empty() {
                let msg: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
                return (
                    self.deny_response(&req.uid, &msg.join("; ")),
                    None,
                );
            }
        }

        // Mutation (sidecar injection)
        let patch = if req.operation == "CREATE" {
            self.maybe_inject_sidecar(pod)
        } else {
            None
        };

        (self.allow_response(&req.uid), patch)
    }

    fn validate_pod(&self, pod: &Value, ns: &str) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();

        violations.extend(self.check_security_context(pod));
        violations.extend(self.check_container_security(pod));
        violations.extend(self.check_host_mounts(pod));
        violations.extend(self.check_policy_rules(pod));

        violations
    }

    fn check_security_context(&self, pod: &Value) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();

        let spec = pod.get("spec");
        if let Some(true) = spec.and_then(|s| s.get("hostPID")).and_then(|v| v.as_bool()) {
            violations.push(PolicyViolation {
                severity: ViolationSeverity::Error,
                message: "hostPID is not allowed".into(),
                field: "spec.hostPID".into(),
            });
        }

        if let Some(true) = spec.and_then(|s| s.get("hostNetwork")).and_then(|v| v.as_bool()) {
            violations.push(PolicyViolation {
                severity: ViolationSeverity::Error,
                message: "hostNetwork is not allowed".into(),
                field: "spec.hostNetwork".into(),
            });
        }

        if let Some(true) = spec.and_then(|s| s.get("hostIPC")).and_then(|v| v.as_bool()) {
            violations.push(PolicyViolation {
                severity: ViolationSeverity::Error,
                message: "hostIPC is not allowed".into(),
                field: "spec.hostIPC".into(),
            });
        }

        violations
    }

    fn check_container_security(&self, pod: &Value) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();
        let containers = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(|c| c.as_array());

        if let Some(containers) = containers {
            for (i, container) in containers.iter().enumerate() {
                let name = container
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");

                // Check privileged
                if let Some(true) = container
                    .get("securityContext")
                    .and_then(|sc| sc.get("privileged"))
                    .and_then(|v| v.as_bool())
                {
                    violations.push(PolicyViolation {
                        severity: ViolationSeverity::Error,
                        message: format!("container '{}' is privileged", name),
                        field: format!("spec.containers[{}].securityContext.privileged", i),
                    });
                }

                // Check dangerous capabilities
                if let Some(add_caps) = container
                    .get("securityContext")
                    .and_then(|sc| sc.get("capabilities"))
                    .and_then(|c| c.get("add"))
                    .and_then(|a| a.as_array())
                {
                    let caps: HashSet<&str> = add_caps
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect();

                    for &cap in DANGEROUS_CAPS {
                        if caps.contains(cap) {
                            violations.push(PolicyViolation {
                                severity: ViolationSeverity::Error,
                                message: format!(
                                    "container '{}' adds dangerous capability {}",
                                    name, cap
                                ),
                                field: format!(
                                    "spec.containers[{}].securityContext.capabilities.add",
                                    i
                                ),
                            });
                        }
                    }
                }

                // Check allowPrivilegeEscalation
                if let Some(true) = container
                    .get("securityContext")
                    .and_then(|sc| sc.get("allowPrivilegeEscalation"))
                    .and_then(|v| v.as_bool())
                {
                    violations.push(PolicyViolation {
                        severity: ViolationSeverity::Error,
                        message: format!(
                            "container '{}' has allowPrivilegeEscalation enabled",
                            name
                        ),
                        field: format!(
                            "spec.containers[{}].securityContext.allowPrivilegeEscalation",
                            i
                        ),
                    });
                }
            }
        }

        violations
    }

    fn check_host_mounts(&self, pod: &Value) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();
        let volumes = pod
            .get("spec")
            .and_then(|s| s.get("volumes"))
            .and_then(|v| v.as_array());

        if let Some(volumes) = volumes {
            for (i, vol) in volumes.iter().enumerate() {
                let host_path = vol.get("hostPath").and_then(|hp| hp.get("path"));
                if let Some(path) = host_path.and_then(|p| p.as_str()) {
                    for &dangerous in DANGEROUS_MOUNTS {
                        if path == dangerous || path.starts_with(&format!("{}/", dangerous)) {
                            violations.push(PolicyViolation {
                                severity: ViolationSeverity::Error,
                                message: format!(
                                    "volume '{}' mounts dangerous host path: {}",
                                    vol.get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("?"),
                                    path
                                ),
                                field: format!("spec.volumes[{}].hostPath.path", i),
                            });
                            break;
                        }
                    }
                }
            }
        }

        violations
    }

    fn check_policy_rules(&self, pod: &Value) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();
        let containers = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(|c| c.as_array());

        if let Some(containers) = containers {
            for container in containers {
                let name = container
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                let image = container
                    .get("image")
                    .and_then(|i| i.as_str())
                    .unwrap_or("");

                for rule in &self.policy_rules {
                    if self.rule_matches(rule, name, image) && rule.action == "deny" {
                        violations.push(PolicyViolation {
                            severity: ViolationSeverity::Error,
                            message: format!(
                                "container '{}' blocked by policy rule '{}'",
                                name, rule.name
                            ),
                            field: format!("spec.containers.{}", name),
                        });
                    }
                }
            }
        }

        violations
    }

    fn rule_matches(&self, rule: &PolicyRule, name: &str, image: &str) -> bool {
        for pattern in &rule.patterns {
            if name == pattern || image.contains(pattern) || pattern == "*" {
                return true;
            }
        }
        false
    }

    fn maybe_inject_sidecar(&self, pod: &Value) -> Option<Vec<Value>> {
        // Skip if already has sidecar
        let containers = pod
            .get("spec")
            .and_then(|s| s.get("containers"))
            .and_then(|c| c.as_array());

        if let Some(containers) = containers {
            for c in containers {
                if c.get("name").and_then(|n| n.as_str()) == Some("flowlink-shield") {
                    return None; // already injected
                }
            }
        }

        // Check for opt-out label
        let labels = pod.get("metadata").and_then(|m| m.get("labels"));
        if let Some(labels) = labels {
            if labels
                .get("shield.flowlink.ai/inject")
                .and_then(|v| v.as_str())
                == Some("disabled")
            {
                return None;
            }
        }

        let policy_yaml = serde_yaml::to_string(&self.policy_rules).unwrap_or_default();
        let sidecar = ShieldSidecar::from_config(&self.config, &policy_yaml);
        Some(sidecar.mutation_patch("default-policy"))
    }

    fn is_exempt(&self, pod: &Value, ns: &str) -> bool {
        // Exempt namespaces
        if self.config.exempt_namespaces.iter().any(|e| e == ns) {
            return true;
        }

        // Exempt labels
        let labels = pod
            .get("metadata")
            .and_then(|m| m.get("labels"))
            .and_then(|l| l.as_object());

        if let Some(labels) = labels {
            for (key, val) in &self.config.exempt_labels {
                if labels.get(key).and_then(|v| v.as_str()) == Some(val.as_str()) {
                    return true;
                }
            }
        }

        false
    }

    fn allow_response(&self, uid: &str) -> AdmissionResponse {
        AdmissionResponse {
            uid: uid.into(),
            allowed: true,
            status: None,
            patch_type: None,
            patch: None,
        }
    }

    fn deny_response(&self, uid: &str, message: &str) -> AdmissionResponse {
        AdmissionResponse {
            uid: uid.into(),
            allowed: false,
            status: Some(AdmissionResponseStatus {
                code: Some(403),
                message: Some(format!("FlowLink Shield: {}", message)),
            }),
            patch_type: None,
            patch: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> K8sConfig {
        K8sConfig::default()
    }

    fn default_webhook() -> AdmissionWebhook {
        AdmissionWebhook::new(default_config(), vec![])
    }

    fn make_request(pod: Value, ns: &str) -> AdmissionRequest {
        AdmissionRequest {
            uid: "test-uid".into(),
            kind: json!({"kind": "Pod"}),
            object: pod,
            namespace: Some(ns.into()),
            operation: "CREATE".into(),
            old_object: None,
            dry_run: None,
        }
    }

    fn safe_pod() -> Value {
        json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "safe-pod", "namespace": "default" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx:latest",
                    "securityContext": {
                        "runAsNonRoot": true,
                        "allowPrivilegeEscalation": false,
                        "capabilities": { "drop": ["ALL"] }
                    }
                }]
            }
        })
    }

    #[test]
    fn test_safe_pod_allowed() {
        let wh = default_webhook();
        let req = make_request(safe_pod(), "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(resp.allowed);
    }

    #[test]
    fn test_privileged_pod_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": { "privileged": true }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        assert!(resp.status.as_ref().unwrap().message.as_ref().unwrap().contains("privileged"));
    }

    #[test]
    fn test_host_pid_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "hostPID": true,
                "containers": [{ "name": "app", "image": "nginx" }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }

    #[test]
    fn test_host_network_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "hostNetwork": true,
                "containers": [{ "name": "app", "image": "nginx" }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }

    #[test]
    fn test_dangerous_capabilities_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": {
                        "capabilities": { "add": ["SYS_ADMIN", "NET_RAW"] }
                    }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }

    #[test]
    fn test_exempt_namespace_allowed() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "sys-pod" },
            "spec": {
                "hostPID": true,
                "containers": [{ "name": "app", "image": "nginx" }]
            }
        });
        let req = make_request(pod, "kube-system");
        let (resp, _) = wh.handle_review(&req);
        assert!(resp.allowed);
    }

    #[test]
    fn test_sidecar_injection() {
        let wh = default_webhook();
        let req = make_request(safe_pod(), "default");
        let (_, patch) = wh.handle_review(&req);
        assert!(patch.is_some());
        let patch = patch.unwrap();
        assert!(patch.len() >= 3);
    }

    #[test]
    fn test_sidecar_not_double_injected() {
        let wh = default_webhook();
        let mut pod = safe_pod();
        let containers = pod["spec"]["containers"].as_array_mut().unwrap();
        containers.push(json!({"name": "flowlink-shield", "image": "test"}));
        let req = make_request(pod, "default");
        let (_, patch) = wh.handle_review(&req);
        assert!(patch.is_none());
    }

    #[test]
    fn test_policy_rule_deny() {
        let rules = vec![PolicyRule {
            name: "no-nginx".into(),
            action: "deny".into(),
            patterns: vec!["nginx".into()],
        }];
        let wh = AdmissionWebhook::new(default_config(), rules);
        let req = make_request(safe_pod(), "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }

    #[test]
    fn test_host_mount_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "containers": [{ "name": "app", "image": "nginx" }],
                "volumes": [{ "name": "etc", "hostPath": { "path": "/etc" } }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }

    #[test]
    fn test_allow_privilege_escalation_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": { "allowPrivilegeEscalation": true }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }
}

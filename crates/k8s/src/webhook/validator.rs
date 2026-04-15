use serde_json::Value;
use std::collections::HashSet;

use crate::crd::PolicyRule;

use super::{types::*, AdmissionWebhook};

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

impl AdmissionWebhook {
    pub(crate) fn validate_pod(&self, pod: &Value, _ns: &str) -> Vec<PolicyViolation> {
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
        if let Some(true) = spec
            .and_then(|s| s.get("hostPID"))
            .and_then(|v| v.as_bool())
        {
            violations.push(PolicyViolation {
                severity: ViolationSeverity::Error,
                message: "hostPID is not allowed".into(),
                field: "spec.hostPID".into(),
            });
        }

        if let Some(true) = spec
            .and_then(|s| s.get("hostNetwork"))
            .and_then(|v| v.as_bool())
        {
            violations.push(PolicyViolation {
                severity: ViolationSeverity::Error,
                message: "hostNetwork is not allowed".into(),
                field: "spec.hostNetwork".into(),
            });
        }

        if let Some(true) = spec
            .and_then(|s| s.get("hostIPC"))
            .and_then(|v| v.as_bool())
        {
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
                    let caps: HashSet<&str> = add_caps.iter().filter_map(|v| v.as_str()).collect();

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
                                    vol.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
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

    pub(crate) fn is_exempt(&self, pod: &Value, ns: &str) -> bool {
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
}

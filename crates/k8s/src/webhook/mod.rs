mod types;
mod validator;

pub use types::*;
use validator::*;

use serde_json::{Value, json};

use crate::config::K8sConfig;
use crate::crd::{PolicyRule, ShieldMode};
use crate::sidecar::ShieldSidecar;

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
    use crate::config::K8sConfig;

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

    // --- New comprehensive tests ---

    #[test]
    fn test_all_security_violations_reported() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "mega-bad" },
            "spec": {
                "hostPID": true,
                "hostNetwork": true,
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": {
                        "privileged": true,
                        "capabilities": { "add": ["SYS_ADMIN", "NET_RAW"] }
                    }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        let msg = resp.status.unwrap().message.unwrap();
        assert!(msg.contains("hostPID"), "missing hostPID: {msg}");
        assert!(msg.contains("hostNetwork"), "missing hostNetwork: {msg}");
        assert!(msg.contains("privileged"), "missing privileged: {msg}");
        assert!(msg.contains("SYS_ADMIN"), "missing SYS_ADMIN: {msg}");
    }

    #[test]
    fn test_run_as_user_zero_with_priv_escalation() {
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
                        "runAsUser": 0,
                        "allowPrivilegeEscalation": true
                    }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        assert!(resp.status.unwrap().message.unwrap().contains("allowPrivilegeEscalation"));
    }

    #[test]
    fn test_sys_ptrace_capability_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "ptrace-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": {
                        "capabilities": { "add": ["SYS_PTRACE"] }
                    }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        assert!(resp.status.unwrap().message.unwrap().contains("SYS_PTRACE"));
    }

    #[test]
    fn test_net_admin_and_sys_admin_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "cap-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": {
                        "capabilities": { "add": ["NET_ADMIN", "SYS_ADMIN"] }
                    }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        let msg = resp.status.unwrap().message.unwrap();
        assert!(msg.contains("NET_ADMIN") && msg.contains("SYS_ADMIN"));
    }

    #[test]
    fn test_docker_sock_mount_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "docker-pod" },
            "spec": {
                "containers": [{ "name": "app", "image": "nginx" }],
                "volumes": [{
                    "name": "docker-sock",
                    "hostPath": { "path": "/var/run/docker.sock" }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        assert!(resp.status.unwrap().message.unwrap().contains("/var/run/docker.sock"));
    }

    #[test]
    fn test_empty_capabilities_allowed() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "safe-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": {
                        "capabilities": { "drop": ["ALL"] }
                    }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(resp.allowed);
    }

    #[test]
    fn test_run_as_non_root_allowed() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "safe-pod" },
            "spec": {
                "securityContext": { "runAsNonRoot": true },
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": { "allowPrivilegeEscalation": false }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(resp.allowed);
    }

    #[test]
    fn test_kube_system_exempt() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "sys-pod" },
            "spec": {
                "hostPID": true,
                "hostNetwork": true,
                "hostIPC": true,
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": { "privileged": true }
                }]
            }
        });
        let req = make_request(pod, "kube-system");
        let (resp, _) = wh.handle_review(&req);
        assert!(resp.allowed);
    }

    #[test]
    fn test_exempt_label_allowed() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "exempt-pod",
                "labels": { "shield.flowlink.ai/exempt": "true" }
            },
            "spec": {
                "hostPID": true,
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": { "privileged": true }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        // Note: default config has empty exempt_labels, so this won't be exempt
        // unless config is set. Test that without the label in config, it's denied.
        assert!(!resp.allowed);
    }

    #[test]
    fn test_exempt_label_with_config() {
        let mut cfg = default_config();
        cfg.exempt_labels.insert(
            "shield.flowlink.ai/exempt".into(),
            "true".into(),
        );
        let wh = AdmissionWebhook::new(cfg, vec![]);
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "exempt-pod",
                "labels": { "shield.flowlink.ai/exempt": "true" }
            },
            "spec": {
                "hostPID": true,
                "containers": [{
                    "name": "app",
                    "image": "nginx",
                    "securityContext": { "privileged": true }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(resp.allowed);
    }

    #[test]
    fn test_multiple_containers_all_checked() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "multi-pod" },
            "spec": {
                "containers": [
                    {
                        "name": "app",
                        "image": "nginx",
                        "securityContext": { "privileged": true }
                    },
                    {
                        "name": "sidecar",
                        "image": "redis",
                        "securityContext": {
                            "capabilities": { "add": ["SYS_PTRACE"] }
                        }
                    },
                    {
                        "name": "logger",
                        "image": "busybox"
                    }
                ]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        let msg = resp.status.unwrap().message.unwrap();
        assert!(msg.contains("app"), "app not in message: {msg}");
        assert!(msg.contains("sidecar"), "sidecar not in message: {msg}");
    }

    #[test]
    fn test_init_container_with_dangerous_command() {
        let wh = default_webhook();
        // Init containers are NOT checked by current implementation — only regular containers.
        // This test documents the current behavior: init containers bypass security checks.
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "init-pod" },
            "spec": {
                "initContainers": [{
                    "name": "init-hack",
                    "image": "busybox",
                    "securityContext": { "privileged": true }
                }],
                "containers": [{
                    "name": "app",
                    "image": "nginx"
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        // Init containers are not checked, so this should be allowed
        assert!(resp.allowed);
    }

    #[test]
    fn test_ephemeral_container_privileged() {
        let wh = default_webhook();
        // Ephemeral containers are NOT in the regular containers array.
        // Current implementation only checks spec.containers, so ephemeral containers
        // bypass security checks.
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "ephemeral-pod" },
            "spec": {
                "containers": [{
                    "name": "app",
                    "image": "nginx"
                }],
                "ephemeralContainers": [{
                    "name": "debugger",
                    "image": "busybox",
                    "securityContext": { "privileged": true }
                }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        // Ephemeral containers not checked — allowed
        assert!(resp.allowed);
    }

    #[test]
    fn test_sidecar_injection_patch_structure() {
        let wh = default_webhook();
        let req = make_request(safe_pod(), "default");
        let (_, patch) = wh.handle_review(&req);
        let patch = patch.unwrap();

        // Check shareProcessNamespace
        let spn = patch.iter().find(|p| p["path"] == "/spec/shareProcessNamespace");
        assert!(spn.is_some());
        assert_eq!(spn.unwrap()["value"], true);

        // Check init container added
        let init = patch.iter().find(|p| p["path"] == "/spec/initContainers");
        assert!(init.is_some());
        let inits = init.unwrap()["value"].as_array().unwrap();
        assert_eq!(inits[0]["name"], "flowlink-shield-init");

        // Check sidecar container added
        let container = patch.iter().find(|p| p["path"] == "/spec/containers/-");
        assert!(container.is_some());
        assert_eq!(container.unwrap()["value"]["name"], "flowlink-shield");

        // Check volumes added
        let volumes: Vec<_> = patch.iter().filter(|p| p["path"] == "/spec/volumes/-").collect();
        assert_eq!(volumes.len(), 2);
        let vol_names: Vec<_> = volumes.iter().map(|v| v["value"]["name"].as_str().unwrap()).collect();
        assert!(vol_names.contains(&"flowlink-shared"));
        assert!(vol_names.contains(&"flowlink-shield-data"));
    }

    #[test]
    fn test_sidecar_custom_resource_limits() {
        let mut cfg = default_config();
        cfg.sidecar_image = "custom/shield:v2".into();
        let wh = AdmissionWebhook::new(cfg, vec![]);
        let req = make_request(safe_pod(), "default");
        let (_, patch) = wh.handle_review(&req);
        let patch = patch.unwrap();

        // Find the sidecar container
        let container = patch.iter().find(|p| p["path"] == "/spec/containers/-").unwrap();
        assert_eq!(container["value"]["image"], "custom/shield:v2");

        // Check resource limits exist
        let resources = &container["value"]["resources"];
        assert!(resources["limits"].is_object());
        assert!(resources["requests"].is_object());
    }

    #[test]
    fn test_sidecar_opt_out_label() {
        let wh = default_webhook();
        let mut pod = safe_pod();
        pod["metadata"]["labels"] = json!({ "shield.flowlink.ai/inject": "disabled" });
        let req = make_request(pod, "default");
        let (_, patch) = wh.handle_review(&req);
        assert!(patch.is_none());
    }

    #[test]
    fn test_no_sidecar_on_update() {
        let wh = default_webhook();
        let mut req = make_request(safe_pod(), "default");
        req.operation = "UPDATE".into();
        let (_, patch) = wh.handle_review(&req);
        assert!(patch.is_none());
    }

    #[test]
    fn test_host_ipc_denied() {
        let wh = default_webhook();
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "bad-pod" },
            "spec": {
                "hostIPC": true,
                "containers": [{ "name": "app", "image": "nginx" }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
        assert!(resp.status.unwrap().message.unwrap().contains("hostIPC"));
    }

    #[test]
    fn test_deny_response_status_code() {
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
        assert_eq!(resp.status.unwrap().code, Some(403));
    }

    #[test]
    fn test_response_uid_preserved() {
        let wh = default_webhook();
        let req = make_request(safe_pod(), "default");
        let (resp, _) = wh.handle_review(&req);
        assert_eq!(resp.uid, "test-uid");
    }

    #[test]
    fn test_default_namespace() {
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
        let mut req = make_request(pod, "default");
        req.namespace = None;
        let (resp, _) = wh.handle_review(&req);
        // Falls through to "default" namespace, not exempt
        assert!(!resp.allowed);
    }

    #[test]
    fn test_policy_rule_wildcard() {
        let rules = vec![PolicyRule {
            name: "block-all".into(),
            action: "deny".into(),
            patterns: vec!["*".into()],
        }];
        let wh = AdmissionWebhook::new(default_config(), rules);
        let req = make_request(safe_pod(), "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }

    #[test]
    fn test_policy_rule_image_match() {
        let rules = vec![PolicyRule {
            name: "no-redis".into(),
            action: "deny".into(),
            patterns: vec!["redis".into()],
        }];
        let wh = AdmissionWebhook::new(default_config(), rules);
        let pod = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "redis-pod" },
            "spec": {
                "containers": [{ "name": "cache", "image": "redis:7" }]
            }
        });
        let req = make_request(pod, "default");
        let (resp, _) = wh.handle_review(&req);
        assert!(!resp.allowed);
    }
}

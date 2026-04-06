use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::K8sConfig;
use crate::crd::ShieldMode;

pub struct ShieldSidecar {
    pub image: String,
    pub mode: ShieldMode,
    pub relay_url: String,
    pub policy_config_b64: String,
}

impl ShieldSidecar {
    pub fn from_config(config: &K8sConfig, policy_yaml: &str) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD.encode(policy_yaml);
        Self {
            image: config.sidecar_image.clone(),
            mode: config.mode.clone(),
            relay_url: config.relay_url.clone(),
            policy_config_b64: b64,
        }
    }

    /// Generate the sidecar container JSON for mutation patch
    pub fn container_json(&self, policy_name: &str) -> Value {
        let mode_str = match &self.mode {
            ShieldMode::Monitor => "monitor",
            ShieldMode::Enforce => "enforce",
        };

        json!({
            "name": "flowlink-shield",
            "image": self.image,
            "imagePullPolicy": "IfNotPresent",
            "command": ["flowlink-shield", "sidecar"],
            "env": [
                { "name": "FLOWLINK_MODE", "value": mode_str },
                { "name": "FLOWLINK_RELAY_URL", "value": self.relay_url },
                { "name": "FLOWLINK_POLICY_B64", "value": self.policy_config_b64 },
                { "name": "FLOWLINK_POLICY_NAME", "value": policy_name }
            ],
            "resources": {
                "requests": { "cpu": "50m", "memory": "32Mi" },
                "limits": { "cpu": "200m", "memory": "128Mi" }
            },
            "securityContext": {
                "runAsNonRoot": true,
                "readOnlyRootFilesystem": true,
                "allowPrivilegeEscalation": false,
                "capabilities": { "drop": ["ALL"] }
            },
            "volumeMounts": [
                { "name": "flowlink-shield-data", "mountPath": "/var/lib/flowlink" }
            ]
        })
    }

    /// Generate the init container JSON for setting up shared resources
    pub fn init_container_json(&self) -> Value {
        json!({
            "name": "flowlink-shield-init",
            "image": self.image,
            "imagePullPolicy": "IfNotPresent",
            "command": ["/bin/sh", "-c", "cp /usr/local/bin/flowlink-shield /flowlink-shared/ && chmod +x /flowlink-shared/flowlink-shield || true"],
            "resources": {
                "requests": { "cpu": "10m", "memory": "16Mi" },
                "limits": { "cpu": "50m", "memory": "32Mi" }
            },
            "volumeMounts": [
                { "name": "flowlink-shared", "mountPath": "/flowlink-shared" }
            ]
        })
    }

    /// Generate the full mutation patch for a pod
    pub fn mutation_patch(&self, policy_name: &str) -> Vec<Value> {
        vec![
            // Set shareProcessNamespace
            json!({
                "op": "add",
                "path": "/spec/shareProcessNamespace",
                "value": true
            }),
            // Add init container
            json!({
                "op": "add",
                "path": "/spec/initContainers",
                "value": [self.init_container_json()]
            }),
            // Add sidecar container
            json!({
                "op": "add",
                "path": "/spec/containers/-",
                "value": self.container_json(policy_name)
            }),
            // Add shared volume
            json!({
                "op": "add",
                "path": "/spec/volumes/-",
                "value": {
                    "name": "flowlink-shared",
                    "emptyDir": {}
                }
            }),
            // Add data volume
            json!({
                "op": "add",
                "path": "/spec/volumes/-",
                "value": {
                    "name": "flowlink-shield-data",
                    "emptyDir": {}
                }
            }),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_sidecar() -> ShieldSidecar {
        ShieldSidecar {
            image: "ghcr.io/flowlink/shield:latest".into(),
            mode: ShieldMode::Monitor,
            relay_url: "http://relay:8080".into(),
            policy_config_b64: base64::engine::general_purpose::STANDARD.encode("rules: []"),
        }
    }

    #[test]
    fn test_sidecar_resource_defaults() {
        let sc = default_sidecar();
        let container = sc.container_json("test-policy");
        let resources = &container["resources"];
        assert_eq!(resources["requests"]["cpu"], "50m");
        assert_eq!(resources["requests"]["memory"], "32Mi");
        assert_eq!(resources["limits"]["cpu"], "200m");
        assert_eq!(resources["limits"]["memory"], "128Mi");
    }

    #[test]
    fn test_sidecar_custom_image() {
        let mut sc = default_sidecar();
        sc.image = "custom/shield:v2".into();
        let container = sc.container_json("policy");
        assert_eq!(container["image"], "custom/shield:v2");
        assert_eq!(container["imagePullPolicy"], "IfNotPresent");
    }

    #[test]
    fn test_sidecar_volume_mount_structure() {
        let sc = default_sidecar();
        let container = sc.container_json("policy");
        let mounts = container["volumeMounts"].as_array().unwrap();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0]["name"], "flowlink-shield-data");
        assert_eq!(mounts[0]["mountPath"], "/var/lib/flowlink");
    }

    #[test]
    fn test_init_container_command_generation() {
        let sc = default_sidecar();
        let init = sc.init_container_json();
        assert_eq!(init["name"], "flowlink-shield-init");
        let cmd = init["command"].as_array().unwrap();
        assert_eq!(cmd[0], "/bin/sh");
        assert_eq!(cmd[1], "-c");
        assert!(cmd[2].as_str().unwrap().contains("cp /usr/local/bin/flowlink-shield"));
    }

    #[test]
    fn test_init_container_resources() {
        let sc = default_sidecar();
        let init = sc.init_container_json();
        assert_eq!(init["resources"]["requests"]["cpu"], "10m");
        assert_eq!(init["resources"]["limits"]["cpu"], "50m");
    }

    #[test]
    fn test_shield_config_as_base64() {
        let sc = default_sidecar();
        let container = sc.container_json("policy");
        let env = container["env"].as_array().unwrap();
        let b64_env = env.iter().find(|e| e["name"] == "FLOWLINK_POLICY_B64").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64_env["value"].as_str().unwrap())
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "rules: []");
    }

    #[test]
    fn test_sidecar_env_vars() {
        let sc = default_sidecar();
        let container = sc.container_json("my-policy");
        let env = container["env"].as_array().unwrap();
        assert_eq!(env.len(), 4);
        let names: Vec<_> = env.iter().map(|e| e["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"FLOWLINK_MODE"));
        assert!(names.contains(&"FLOWLINK_RELAY_URL"));
        assert!(names.contains(&"FLOWLINK_POLICY_B64"));
        assert!(names.contains(&"FLOWLINK_POLICY_NAME"));
    }

    #[test]
    fn test_sidecar_enforce_mode() {
        let mut sc = default_sidecar();
        sc.mode = ShieldMode::Enforce;
        let container = sc.container_json("policy");
        let env = container["env"].as_array().unwrap();
        let mode_env = env.iter().find(|e| e["name"] == "FLOWLINK_MODE").unwrap();
        assert_eq!(mode_env["value"], "enforce");
    }

    #[test]
    fn test_sidecar_monitor_mode() {
        let sc = default_sidecar();
        let container = sc.container_json("policy");
        let env = container["env"].as_array().unwrap();
        let mode_env = env.iter().find(|e| e["name"] == "FLOWLINK_MODE").unwrap();
        assert_eq!(mode_env["value"], "monitor");
    }

    #[test]
    fn test_sidecar_security_context() {
        let sc = default_sidecar();
        let container = sc.container_json("policy");
        let sc_val = &container["securityContext"];
        assert_eq!(sc_val["runAsNonRoot"], true);
        assert_eq!(sc_val["readOnlyRootFilesystem"], true);
        assert_eq!(sc_val["allowPrivilegeEscalation"], false);
    }

    #[test]
    fn test_mutation_patch_count() {
        let sc = default_sidecar();
        let patch = sc.mutation_patch("policy");
        assert_eq!(patch.len(), 5);
    }

    #[test]
    fn test_from_config() {
        let cfg = K8sConfig::default();
        let sc = ShieldSidecar::from_config(&cfg, "policy: test");
        assert_eq!(sc.image, cfg.sidecar_image);
        assert_eq!(sc.mode, cfg.mode);
        assert_eq!(sc.relay_url, cfg.relay_url);
        assert!(!sc.policy_config_b64.is_empty());
    }

    #[test]
    fn test_init_container_volume_mount() {
        let sc = default_sidecar();
        let init = sc.init_container_json();
        let mounts = init["volumeMounts"].as_array().unwrap();
        assert_eq!(mounts[0]["name"], "flowlink-shared");
        assert_eq!(mounts[0]["mountPath"], "/flowlink-shared");
    }
}

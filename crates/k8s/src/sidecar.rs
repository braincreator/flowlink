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

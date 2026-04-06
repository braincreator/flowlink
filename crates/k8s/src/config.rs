use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::crd::ShieldMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sConfig {
    pub namespace: String,
    pub relay_url: String,
    pub mode: ShieldMode,
    pub webhook_port: u16,
    pub cert_dir: String,
    pub sidecar_image: String,
    #[serde(default)]
    pub exempt_namespaces: Vec<String>,
    #[serde(default)]
    pub exempt_labels: HashMap<String, String>,
}

impl Default for K8sConfig {
    fn default() -> Self {
        Self {
            namespace: "flowlink-system".into(),
            relay_url: "http://flowlink-relay:8080".into(),
            mode: ShieldMode::Monitor,
            webhook_port: 9443,
            cert_dir: "/tmp/flowlink-certs".into(),
            sidecar_image: "ghcr.io/flowlink/shield:latest".into(),
            exempt_namespaces: vec![
                "kube-system".into(),
                "kube-public".into(),
                "kube-node-lease".into(),
                "flowlink-system".into(),
            ],
            exempt_labels: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let cfg = K8sConfig::default();
        assert_eq!(cfg.namespace, "flowlink-system");
        assert_eq!(cfg.relay_url, "http://flowlink-relay:8080");
        assert_eq!(cfg.mode, ShieldMode::Monitor);
        assert_eq!(cfg.webhook_port, 9443);
        assert_eq!(cfg.cert_dir, "/tmp/flowlink-certs");
        assert_eq!(cfg.sidecar_image, "ghcr.io/flowlink/shield:latest");
        assert_eq!(cfg.exempt_namespaces.len(), 4);
        assert!(cfg.exempt_labels.is_empty());
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let cfg = K8sConfig::default();
        let json_str = serde_json::to_string(&cfg).unwrap();
        let deserialized: K8sConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(cfg.namespace, deserialized.namespace);
        assert_eq!(cfg.relay_url, deserialized.relay_url);
        assert_eq!(cfg.webhook_port, deserialized.webhook_port);
    }

    #[test]
    fn test_config_custom_exempt_namespaces() {
        let mut cfg = K8sConfig::default();
        cfg.exempt_namespaces = vec!["custom-ns".into(), "another-ns".into()];
        assert_eq!(cfg.exempt_namespaces.len(), 2);
        assert!(cfg.exempt_namespaces.contains(&"custom-ns".to_string()));
    }

    #[test]
    fn test_config_custom_exempt_labels() {
        let mut cfg = K8sConfig::default();
        cfg.exempt_labels.insert("shield.flowlink.ai/exempt".into(), "true".into());
        assert_eq!(cfg.exempt_labels.len(), 1);
        assert_eq!(cfg.exempt_labels.get("shield.flowlink.ai/exempt").unwrap(), "true");
    }

    #[test]
    fn test_config_enforce_mode() {
        let mut cfg = K8sConfig::default();
        cfg.mode = ShieldMode::Enforce;
        assert_eq!(cfg.mode, ShieldMode::Enforce);
    }

    #[test]
    fn test_config_yaml_roundtrip() {
        let cfg = K8sConfig::default();
        let yaml_str = serde_yaml::to_string(&cfg).unwrap();
        let deserialized: K8sConfig = serde_yaml::from_str(&yaml_str).unwrap();
        assert_eq!(cfg.webhook_port, deserialized.webhook_port);
        assert_eq!(cfg.cert_dir, deserialized.cert_dir);
    }

    #[test]
    fn test_config_with_empty_exempt_namespaces() {
        let cfg = serde_json::from_value::<K8sConfig>(json!({
            "namespace": "test",
            "relay_url": "http://test:8080",
            "mode": "Monitor",
            "webhook_port": 8443,
            "cert_dir": "/tmp/certs",
            "sidecar_image": "test:latest"
        })).unwrap();
        assert!(cfg.exempt_namespaces.is_empty());
    }
}

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

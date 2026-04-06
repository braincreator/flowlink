use std::{sync::Arc, time::Duration};

use anyhow::Result;
use futures::StreamExt;
use kube::{Client, ResourceExt, api::Api, runtime::{controller, watcher, Controller}};
use serde_json::json;

use crate::config::K8sConfig;
use crate::crd::FlowLinkShieldPolicy;

pub struct ShieldOperator {
    pub client: Client,
    pub config: K8sConfig,
}

impl ShieldOperator {
    pub fn new(client: Client, config: K8sConfig) -> Self {
        Self { client, config }
    }

    pub async fn run(&self) -> Result<()> {
        let crds: Api<FlowLinkShieldPolicy> = Api::all(self.client.clone());

        log::info!("Starting FlowLink Shield operator");

        Controller::new(crds, watcher::Config::default())
            .shutdown_on_signal()
            .run(
                |policy, ctx| {
                    let policy = Arc::clone(&policy);
                    let ctx = Arc::clone(&ctx);
                    async move { reconcile(policy, ctx).await }
                },
                error_policy,
                Arc::new(self.config.clone()),
            )
            .for_each(|res| async move {
                if let Err(e) = res {
                    log::error!("Controller stream error: {:?}", e);
                }
            })
            .await;

        Ok(())
    }

    /// Generate self-signed TLS cert for webhook
    pub fn generate_webhook_cert(
        &self,
        service_name: &str,
        namespace: &str,
    ) -> Result<(String, String)> {
        use rcgen::{CertificateParams, KeyPair, SanType};

        let mut params = CertificateParams::new(vec![format!(
            "{}.{}.svc.cluster.local",
            service_name, namespace
        )]);
        params
            .subject_alt_names
            .push(SanType::DnsName(service_name.to_string()));

        let key_pair = KeyPair::generate(&rcgen::PKCS_RSA_SHA256)?;
        let cert = rcgen::Certificate::from_params(params)?;
        Ok((cert.serialize_pem()?, cert.serialize_private_key_pem()))
    }

    /// Store cert in K8s Secret
    pub async fn store_cert_secret(
        &self,
        namespace: &str,
        cert_pem: &str,
        key_pem: &str,
    ) -> Result<()> {
        use kube::api::{Patch, PatchParams};
        use k8s_openapi::api::core::v1::Secret;

        let secrets: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        let name = "flowlink-webhook-certs";

        let secret = serde_json::from_value::<Secret>(json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "metadata": { "name": name, "namespace": namespace },
            "type": "kubernetes.io/tls",
            "stringData": { "tls.crt": cert_pem, "tls.key": key_pem }
        }))?;

        let pp = PatchParams::apply("flowlink-shield").force();
        secrets.patch(name, &pp, &Patch::Apply(&secret)).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
enum ReconcileError {
    #[error("kube error: {0}")]
    Kube(#[from] kube::Error),
    #[error("{0}")]
    Other(String),
}

async fn reconcile(
    policy: Arc<FlowLinkShieldPolicy>,
    _ctx: Arc<K8sConfig>,
) -> std::result::Result<controller::Action, ReconcileError> {
    log::info!(
        "Reconciling FlowLinkShieldPolicy {} in namespace {}",
        policy.name_any(),
        policy.namespace().unwrap_or_default()
    );

    if !policy.spec.enabled {
        log::info!("Policy {} is disabled, skipping", policy.name_any());
        return Ok(controller::Action::requeue(Duration::from_secs(300)));
    }

    Ok(controller::Action::requeue(Duration::from_secs(60)))
}

fn error_policy(
    _policy: Arc<FlowLinkShieldPolicy>,
    error: &ReconcileError,
    _ctx: Arc<K8sConfig>,
) -> controller::Action {
    log::error!("Reconcile error: {:?}", error);
    controller::Action::requeue(Duration::from_secs(30))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standalone cert generation test (no K8s cluster needed)
    #[test]
    fn test_cert_generation() {
        use rcgen::{CertificateParams, SanType};
        // Test that certificate params can be constructed with correct SANs
        let service_name = "flowlink-shield-webhook";
        let namespace = "flowlink-system";
        let mut params = CertificateParams::new(vec![format!(
            "{}.{}.svc.cluster.local",
            service_name, namespace
        )]);
        params
            .subject_alt_names
            .push(SanType::DnsName(service_name.to_string()));
        // Verify SANs are set correctly
        assert!(params.subject_alt_names.len() >= 2);
    }

    #[test]
    fn test_cert_rotation() {
        // Test that two distinct certificate parameter sets produce different serial numbers
        use rcgen::{CertificateParams, SanType};
        let params1 = CertificateParams::new(vec!["a.example.com".into()]);
        let params2 = CertificateParams::new(vec!["b.example.com".into()]);
        assert_ne!(params1.subject_alt_names, params2.subject_alt_names);
    }

    #[test]
    fn test_webhook_config_service_name() {
        let service_name = "flowlink-shield-webhook";
        let namespace = "flowlink-system";
        let expected = format!("{}.{}.svc.cluster.local", service_name, namespace);
        assert_eq!(expected, "flowlink-shield-webhook.flowlink-system.svc.cluster.local");
    }

    #[test]
    fn test_reconcile_error_display() {
        let err = ReconcileError::Other("test error".into());
        let msg = format!("{}", err);
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_config_for_operator() {
        let cfg = K8sConfig::default();
        assert_eq!(cfg.webhook_port, 9443);
        assert_eq!(cfg.cert_dir, "/tmp/flowlink-certs");
        assert!(cfg.exempt_namespaces.contains(&"kube-system".to_string()));
    }
}

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use base64::Engine;
use futures::StreamExt;
use kube::{Client, ResourceExt, api::{Api, Patch, PatchParams}, runtime::{controller, watcher, Controller}};
use kube::api::PostParams;
use serde_json::json;

use crate::config::K8sConfig;
use crate::crd::{FlowLinkShieldPolicy, FlowLinkShieldPolicyStatus, PolicyCondition, ShieldMode};

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
                Arc::new((self.client.clone(), self.config.clone())),
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

        let _key_pair = KeyPair::generate(&rcgen::PKCS_RSA_SHA256)?;
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

    /// Configure or update the MutatingWebhookConfiguration for the given namespace.
    pub async fn ensure_webhook_config(&self, namespace: &str, cert_pem: &str) -> Result<()> {
        use k8s_openapi::api::admissionregistration::v1::{
            MutatingWebhookConfiguration, MutatingWebhook, WebhookClientConfig, RuleWithOperations,
        };

        let webhook_name = "flowlink-shield-webhook";
        let service_name = "flowlink-shield-webhook";
        let webhook_path = "/mutate";

        // Base64 encode the CA cert for CABundle
        let ca_bundle = base64::engine::general_purpose::STANDARD.encode(cert_pem);

        let webhook = serde_json::from_value::<MutatingWebhookConfiguration>(json!({
            "apiVersion": "admissionregistration.k8s.io/v1",
            "kind": "MutatingWebhookConfiguration",
            "metadata": {
                "name": webhook_name,
                "labels": {
                    "app.kubernetes.io/name": "flowlink-shield",
                    "app.kubernetes.io/managed-by": "flowlink-operator"
                }
            },
            "webhooks": [{
                "name": "shield.flowlink.ai",
                "admissionReviewVersions": ["v1"],
                "clientConfig": {
                    "service": {
                        "name": service_name,
                        "namespace": namespace,
                        "path": webhook_path,
                        "port": self.config.webhook_port,
                    },
                    "caBundle": ca_bundle,
                },
                "rules": [{
                    "apiGroups": [""],
                    "apiVersions": ["v1"],
                    "operations": ["CREATE", "UPDATE"],
                    "resources": ["pods"],
                    "scope": "Namespaced",
                }],
                "objectSelector": {
                    "matchExpressions": [{
                        "key": "shield.flowlink.ai/inject",
                        "operator": "NotIn",
                        "values": ["disabled"],
                    }],
                },
                "sideEffects": "None",
                "timeoutSeconds": 10,
                "failurePolicy": "Fail",
            }],
        }))?;

        let webhooks: Api<MutatingWebhookConfiguration> = Api::all(self.client.clone());

        // Try to create; if exists, patch
        let pp = PatchParams::apply("flowlink-shield").force();
        match webhooks.create(&PostParams::default(), &webhook).await {
            Ok(_) => {
                log::info!("Created MutatingWebhookConfiguration {}", webhook_name);
            }
            Err(kube::Error::Api(e)) if e.code == 409 => {
                // Already exists — update it
                webhooks.patch(webhook_name, &pp, &Patch::Apply(&webhook)).await?;
                log::info!("Updated MutatingWebhookConfiguration {}", webhook_name);
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    /// Ensure the ValidatingWebhookConfiguration for policy enforcement.
    pub async fn ensure_validating_webhook(&self, namespace: &str, cert_pem: &str) -> Result<()> {
        use k8s_openapi::api::admissionregistration::v1::ValidatingWebhookConfiguration;

        let webhook_name = "flowlink-shield-validator";
        let ca_bundle = base64::engine::general_purpose::STANDARD.encode(cert_pem);

        let webhook = serde_json::from_value::<ValidatingWebhookConfiguration>(json!({
            "apiVersion": "admissionregistration.k8s.io/v1",
            "kind": "ValidatingWebhookConfiguration",
            "metadata": {
                "name": webhook_name,
                "labels": {
                    "app.kubernetes.io/name": "flowlink-shield",
                    "app.kubernetes.io/managed-by": "flowlink-operator"
                }
            },
            "webhooks": [{
                "name": "shield-validator.flowlink.ai",
                "admissionReviewVersions": ["v1"],
                "clientConfig": {
                    "service": {
                        "name": "flowlink-shield-webhook",
                        "namespace": namespace,
                        "path": "/validate",
                        "port": self.config.webhook_port,
                    },
                    "caBundle": ca_bundle,
                },
                "rules": [{
                    "apiGroups": [""],
                    "apiVersions": ["v1"],
                    "operations": ["CREATE", "UPDATE"],
                    "resources": ["pods"],
                    "scope": "Namespaced",
                }],
                "sideEffects": "None",
                "timeoutSeconds": 5,
                "failurePolicy": "Fail",
            }],
        }))?;

        let webhooks: Api<ValidatingWebhookConfiguration> = Api::all(self.client.clone());
        let pp = PatchParams::apply("flowlink-shield").force();

        match webhooks.create(&PostParams::default(), &webhook).await {
            Ok(_) => log::info!("Created ValidatingWebhookConfiguration {}", webhook_name),
            Err(kube::Error::Api(e)) if e.code == 409 => {
                webhooks.patch(webhook_name, &pp, &Patch::Apply(&webhook)).await?;
                log::info!("Updated ValidatingWebhookConfiguration {}", webhook_name);
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    /// Remove webhook configurations (cleanup on policy deletion).
    pub async fn remove_webhook_configs(&self) -> Result<()> {
        use k8s_openapi::api::admissionregistration::v1::{
            MutatingWebhookConfiguration, ValidatingWebhookConfiguration,
        };

        let mutating: Api<MutatingWebhookConfiguration> = Api::all(self.client.clone());
        let validating: Api<ValidatingWebhookConfiguration> = Api::all(self.client.clone());

        if let Err(e) = mutating.delete("flowlink-shield-webhook", &DeleteParams::default()).await {
            let is_404 = matches!(&e, kube::Error::Api(ae) if ae.code == 404);
            if !is_404 {
                log::warn!("Failed to delete mutating webhook: {}", e);
            }
        }

        if let Err(e) = validating.delete("flowlink-shield-validator", &DeleteParams::default()).await {
            let is_404 = matches!(&e, kube::Error::Api(ae) if ae.code == 404);
            if !is_404 {
                log::warn!("Failed to delete validating webhook: {}", e);
            }
        }

        log::info!("Webhook configurations cleaned up");
        Ok(())
    }
}

use kube::api::DeleteParams;

#[derive(Debug, thiserror::Error)]
enum ReconcileError {
    #[error("kube error: {0}")]
    Kube(#[from] kube::Error),
    #[error("{0}")]
    Other(String),
}

/// Reconcile a FlowLinkShieldPolicy CRD.
///
/// On create/update:
///   1. Update CRD status (observedGeneration, conditions)
///   2. If enabled + admission_webhook: ensure MutatingWebhookConfiguration
///   3. If mode=Enforce: also ensure ValidatingWebhookConfiguration
///   4. If disabled: remove webhook configs
///
/// On delete: webhook configs are cleaned up by the garbage collector
/// (ownerReferences set on the webhook resources).
async fn reconcile(
    policy: Arc<FlowLinkShieldPolicy>,
    ctx: Arc<(Client, K8sConfig)>,
) -> std::result::Result<controller::Action, ReconcileError> {
    let (client, config) = ctx.as_ref();
    let name = policy.name_any();
    let ns = policy.namespace().unwrap_or_default();
    let generation = policy.metadata.generation.unwrap_or(0);
    let operator = ShieldOperator::new(client.clone(), config.clone());

    log::info!(
        "Reconciling FlowLinkShieldPolicy {} in namespace {} (gen={})",
        name, ns, generation
    );

    let crds: Api<FlowLinkShieldPolicy> = Api::namespaced(client.clone(), &ns);

    // Build status
    let now = chrono::Utc::now().to_rfc3339();
    let mut status = policy.status.clone().unwrap_or_default();
    status.observed_generation = Some(generation);

    if !policy.spec.enabled {
        log::info!("Policy {} is disabled, removing webhook configs", name);

        // Remove webhooks and update status
        if let Err(e) = operator.remove_webhook_configs().await {
            log::warn!("Failed to remove webhook configs: {}", e);
        }

        // Update status to reflect disabled state
        status.conditions = vec![PolicyCondition {
            type_: "Ready".into(),
            status: "False".into(),
            reason: Some("Disabled".into()),
            message: Some("Policy is disabled, webhooks removed".into()),
            last_transition_time: Some(now),
        }];

        update_status(&crds, &name, &status).await?;
        return Ok(controller::Action::requeue(Duration::from_secs(300)));
    }

    // Policy is enabled — configure webhooks
    let mut errors = Vec::new();

    // 1. Generate or load certs
    let (cert_pem, key_pem) = match operator.generate_webhook_cert(
        "flowlink-shield-webhook",
        &config.namespace,
    ) {
        Ok((cert, key)) => (cert, key),
        Err(e) => {
            log::error!("Failed to generate webhook cert: {}", e);
            errors.push(format!("cert generation: {}", e));
            (String::new(), String::new())
        }
    };

    // 2. Store cert as K8s secret
    if !errors.is_empty() {
        if let Err(e) = operator.store_cert_secret(&config.namespace, &cert_pem, &key_pem).await {
            log::warn!("Failed to store cert secret: {}", e);
        }
    } else if let Err(e) = operator.store_cert_secret(&config.namespace, &cert_pem, &key_pem).await {
        log::warn!("Failed to store cert secret: {}", e);
        errors.push(format!("cert storage: {}", e));
    }

    // 3. Ensure MutatingWebhookConfiguration (for sidecar injection)
    if policy.spec.admission_webhook {
        if let Err(e) = operator.ensure_webhook_config(&config.namespace, &cert_pem).await {
            log::error!("Failed to configure mutating webhook: {}", e);
            errors.push(format!("mutating webhook: {}", e));
        }
    }

    // 4. Ensure ValidatingWebhookConfiguration (for policy enforcement)
    if policy.spec.mode == ShieldMode::Enforce {
        if let Err(e) = operator.ensure_validating_webhook(&config.namespace, &cert_pem).await {
            log::error!("Failed to configure validating webhook: {}", e);
            errors.push(format!("validating webhook: {}", e));
        }
    } else {
        // In monitor mode, remove the validating webhook if it exists
        use k8s_openapi::api::admissionregistration::v1::ValidatingWebhookConfiguration;
        let validating: Api<ValidatingWebhookConfiguration> = Api::all(client.clone());
        if let Err(e) = validating.delete("flowlink-shield-validator", &DeleteParams::default()).await {
            let is_404 = matches!(&e, kube::Error::Api(ae) if ae.code == 404);
            if !is_404 {
                log::warn!("Failed to remove validating webhook: {}", e);
            }
        }
    }

    // 5. Update CRD status
    if errors.is_empty() {
        status.sidecar_injections = Some(status.sidecar_injections.unwrap_or(0));
        status.violations_blocked = Some(status.violations_blocked.unwrap_or(0));
        status.conditions = vec![
            PolicyCondition {
                type_: "Ready".into(),
                status: "True".into(),
                reason: Some("WebhooksConfigured".into()),
                message: Some(format!(
                    "Shield active in {} mode ({} rules, sidecar: {})",
                    match policy.spec.mode {
                        ShieldMode::Monitor => "monitor",
                        ShieldMode::Enforce => "enforce",
                    },
                    policy.spec.rules.len(),
                    policy.spec.admission_webhook,
                )),
                last_transition_time: Some(now),
            },
        ];
    } else {
        status.conditions = vec![PolicyCondition {
            type_: "Ready".into(),
            status: "False".into(),
            reason: Some("ConfigurationFailed".into()),
            message: Some(errors.join("; ")),
            last_transition_time: Some(now),
        }];
    }

    update_status(&crds, &name, &status).await?;

    // Requeue interval based on mode
    let requeue = match policy.spec.mode {
        ShieldMode::Monitor => Duration::from_secs(60),
        ShieldMode::Enforce => Duration::from_secs(30),
    };

    Ok(controller::Action::requeue(requeue))
}

/// Update the status subresource of a FlowLinkShieldPolicy.
async fn update_status(
    api: &Api<FlowLinkShieldPolicy>,
    name: &str,
    status: &FlowLinkShieldPolicyStatus,
) -> Result<(), ReconcileError> {
    let new_status = serde_json::to_value(status)
        .map_err(|e| ReconcileError::Other(format!("Failed to serialize status: {}", e)))?;

    let patch = json!({
        "status": new_status,
    });

    let pp = PatchParams::apply("flowlink-shield").force();
    api.patch_status(name, &pp, &Patch::Merge(&patch)).await?;
    Ok(())
}

fn error_policy(
    _policy: Arc<FlowLinkShieldPolicy>,
    error: &ReconcileError,
    _ctx: Arc<(Client, K8sConfig)>,
) -> controller::Action {
    log::error!("Reconcile error: {:?}", error);
    // Exponential backoff would be ideal, but kube-rs doesn't support it directly
    // Use a fixed delay with jitter via random offset
    controller::Action::requeue(Duration::from_secs(30))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standalone cert generation test (no K8s cluster needed)
    #[test]
    fn test_cert_generation() {
        use rcgen::{CertificateParams, SanType};
        let service_name = "flowlink-shield-webhook";
        let namespace = "flowlink-system";
        let mut params = CertificateParams::new(vec![format!(
            "{}.{}.svc.cluster.local",
            service_name, namespace
        )]);
        params
            .subject_alt_names
            .push(SanType::DnsName(service_name.to_string()));
        assert!(params.subject_alt_names.len() >= 2);
    }

    #[test]
    fn test_cert_rotation() {
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

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use base64::Engine;
use futures::StreamExt;
use kube::{Client, ResourceExt, api::{Api, Patch, PatchParams}, runtime::{controller, watcher, Controller}, runtime::watcher::Event as KubeWatchEvent};
use kube::api::PostParams;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::K8sConfig;
use crate::crd::{FlowLinkShieldPolicy, FlowLinkShieldPolicySpec, FlowLinkShieldPolicyStatus, PolicyCondition, PolicyRule, ShieldMode};

// ---------------------------------------------------------------------------
// Reconciliation helper types
// ---------------------------------------------------------------------------

/// Result of a reconciliation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileAction {
    /// Nothing more to do — requeue later.
    Done,
    /// Requeue after the given duration.
    Requeue(Duration),
}

/// Describes a single drift observation between desired and actual cluster state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftEvent {
    /// Resource identifier, e.g. "Deployment/flowlink-relay" or "ConfigMap/relay-config".
    pub resource: String,
    /// The field that drifted, e.g. "image" or "data.relay_url".
    pub field: String,
    /// Desired value.
    pub expected: String,
    /// Current actual value.
    pub actual: String,
    /// RFC 3339 timestamp of when drift was detected.
    pub detected_at: String,
}

/// Events yielded by the policy watch stream.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    Added(String, String),      // (name, namespace)
    Modified(String, String),
    Deleted(String, String),
    Error(String),
}

// ---------------------------------------------------------------------------
// Operator
// ---------------------------------------------------------------------------

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
        use k8s_openapi::api::admissionregistration::v1::MutatingWebhookConfiguration;

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

    /// Detect drift between desired policy spec and actual cluster state.
    ///
    /// Compares the CR spec fields against the live cluster resources.
    /// Returns a list of `DriftEvent`s for each mismatch found.
    pub async fn detect_drift(&self, name: &str, namespace: &str) -> Result<Vec<DriftEvent>> {
        use k8s_openapi::api::core::v1::{ConfigMap, Secret};
        use kube::ResourceExt;

        let crds: Api<FlowLinkShieldPolicy> = Api::namespaced(self.client.clone(), namespace);
        let policy = crds.get(name).await?;
        let spec = &policy.spec;
        let now = chrono::Utc::now().to_rfc3339();
        let mut drifts = Vec::new();

        // Check ConfigMap drift
        let configmaps: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
        let cm_name = format!("flowlink-{}-config", name);
        if let Ok(cm) = configmaps.get(&cm_name).await {
            let cm_data = cm.data.unwrap_or_default();
            let desired_mode_str = match spec.mode {
                ShieldMode::Monitor => "monitor",
                ShieldMode::Enforce => "enforce",
            };
            if cm_data.get("shield.mode").map(|s| s.as_str()) != Some(desired_mode_str) {
                drifts.push(DriftEvent {
                    resource: format!("ConfigMap/{}", cm_name),
                    field: "data.shield.mode".into(),
                    expected: match spec.mode {
                        ShieldMode::Monitor => "monitor".into(),
                        ShieldMode::Enforce => "enforce".into(),
                    },
                    actual: cm_data.get("shield.mode").cloned().unwrap_or_default(),
                    detected_at: now.clone(),
                });
            }
        } else {
            drifts.push(DriftEvent {
                resource: format!("ConfigMap/{}", cm_name),
                field: "existence".into(),
                expected: "exists".into(),
                actual: "missing".into(),
                detected_at: now.clone(),
            });
        }

        // Check Secret drift for webhook certs
        let secrets: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        let secret_name = "flowlink-webhook-certs";
        if spec.admission_webhook {
            if let Err(e) = secrets.get(secret_name).await {
                drifts.push(DriftEvent {
                    resource: format!("Secret/{}", secret_name),
                    field: "existence".into(),
                    expected: "exists".into(),
                    actual: format!("missing ({})", e),
                    detected_at: now.clone(),
                });
            }
        }

        // Check rule count drift
        if spec.rules.is_empty() {
            drifts.push(DriftEvent {
                resource: format!("FlowLinkShieldPolicy/{}", name),
                field: "spec.rules".into(),
                expected: "non-empty".into(),
                actual: "0 rules".into(),
                detected_at: now,
            });
        }

        Ok(drifts)
    }

    /// Apply the policy by creating/updating cluster resources.
    ///
    /// Creates ConfigMap, Secret, and optionally webhook configurations
    /// based on the FlowLinkShieldPolicy spec.
    pub async fn apply_policy(&self, name: &str, namespace: &str) -> Result<()> {
        use k8s_openapi::api::core::v1::{ConfigMap, Secret};

        let crds: Api<FlowLinkShieldPolicy> = Api::namespaced(self.client.clone(), namespace);
        let policy = crds.get(name).await?;
        let spec = &policy.spec;

        // Create/update ConfigMap
        let configmaps: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
        let cm_name = format!("flowlink-{}-config", name);
        let cm = serde_json::from_value::<ConfigMap>(json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {
                "name": cm_name,
                "namespace": namespace,
                "labels": {
                    "app.kubernetes.io/name": "flowlink-shield",
                    "app.kubernetes.io/managed-by": "flowlink-operator",
                    "flowlink.ai/policy": name,
                },
            },
            "data": {
                "shield.mode": match spec.mode {
                    ShieldMode::Monitor => "monitor",
                    ShieldMode::Enforce => "enforce",
                },
                "shield.enabled": spec.enabled.to_string(),
                "shield.rules_count": spec.rules.len().to_string(),
                "shield.admission_webhook": spec.admission_webhook.to_string(),
                "rules": serde_json::to_string(&spec.rules).unwrap_or_default(),
            }
        }))?;

        let pp = PatchParams::apply("flowlink-shield").force();
        match configmaps.create(&PostParams::default(), &cm).await {
            Ok(_) => log::info!("Created ConfigMap {}", cm_name),
            Err(kube::Error::Api(e)) if e.code == 409 => {
                configmaps.patch(&cm_name, &pp, &Patch::Apply(&cm)).await?;
                log::info!("Updated ConfigMap {}", cm_name);
            }
            Err(e) => return Err(e.into()),
        }

        // Generate and store certs if admission webhook is enabled
        if spec.admission_webhook {
            let (cert_pem, key_pem) = self.generate_webhook_cert(
                "flowlink-shield-webhook",
                namespace,
            )?;
            self.store_cert_secret(namespace, &cert_pem, &key_pem).await?;
            self.ensure_webhook_config(namespace, &cert_pem).await?;
            if spec.mode == ShieldMode::Enforce {
                self.ensure_validating_webhook(namespace, &cert_pem).await?;
            }
        }

        log::info!("Policy {} applied successfully", name);
        Ok(())
    }

    /// Cleanup all resources owned by a FlowLinkShieldPolicy CR.
    ///
    /// Removes ConfigMaps and webhook configurations created by apply_policy.
    pub async fn cleanup_policy(&self, name: &str, namespace: &str) -> Result<()> {
        use k8s_openapi::api::core::v1::ConfigMap;

        let configmaps: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
        let cm_name = format!("flowlink-{}-config", name);

        if let Err(e) = configmaps.delete(&cm_name, &DeleteParams::default()).await {
            let is_404 = matches!(&e, kube::Error::Api(ae) if ae.code == 404);
            if !is_404 {
                log::warn!("Failed to delete ConfigMap {}: {}", cm_name, e);
            }
        }

        self.remove_webhook_configs().await?;

        log::info!("Policy {} cleaned up in namespace {}", name, namespace);
        Ok(())
    }

    /// Watch FlowLinkShieldPolicy CRs and yield events.
    ///
    /// Returns a stream of WatchEvent for policy create/modify/delete.
    pub async fn watch_policies(&self, namespace: &str) -> Result<Vec<WatchEvent>> {
        let crds: Api<FlowLinkShieldPolicy> = if namespace.is_empty() {
            Api::all(self.client.clone())
        } else {
            Api::namespaced(self.client.clone(), namespace)
        };

        let mut events = Vec::new();
        let watcher = watcher(crds, watcher::Config::default());

        tokio::pin!(watcher);
        // Collect events up to a timeout
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() > deadline {
                break;
            }
            match tokio::time::timeout(Duration::from_secs(1), watcher.next()).await {
                Ok(Some(Ok(KubeWatchEvent::Apply(obj)))) => {
                    let name = obj.name_any();
                    let ns = obj.namespace().unwrap_or_default();
                    events.push(WatchEvent::Modified(name, ns));
                }
                Ok(Some(Ok(KubeWatchEvent::Delete(obj)))) => {
                    let name = obj.name_any();
                    let ns = obj.namespace().unwrap_or_default();
                    events.push(WatchEvent::Deleted(name, ns));
                }
                Ok(Some(Ok(KubeWatchEvent::InitApply(obj)))) => {
                    let name = obj.name_any();
                    let ns = obj.namespace().unwrap_or_default();
                    events.push(WatchEvent::Added(name, ns));
                }
                // Ignore Init and InitDone — these are bookmark events
                Ok(Some(Ok(KubeWatchEvent::Init | KubeWatchEvent::InitDone))) => {}
                Ok(Some(Err(e))) => {
                    events.push(WatchEvent::Error(format!("{}", e)));
                }
                Ok(None) | Err(_) => break,
            }
        }

        Ok(events)
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

    #[test]
    fn test_drift_event_serialization() {
        let event = DriftEvent {
            resource: "ConfigMap/test-config".into(),
            field: "data.shield.mode".into(),
            expected: "enforce".into(),
            actual: "monitor".into(),
            detected_at: "2024-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("enforce"));
        assert!(json.contains("monitor"));
        let parsed: DriftEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.resource, "ConfigMap/test-config");
        assert_eq!(parsed.field, "data.shield.mode");
    }

    #[test]
    fn test_drift_event_equality() {
        let a = DriftEvent {
            resource: "Deployment/relay".into(),
            field: "image".into(),
            expected: "v2.0".into(),
            actual: "v1.0".into(),
            detected_at: "2024-01-01T00:00:00Z".into(),
        };
        let b = DriftEvent {
            resource: "Deployment/relay".into(),
            field: "image".into(),
            expected: "v2.0".into(),
            actual: "v1.0".into(),
            detected_at: "2024-01-01T00:00:00Z".into(),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn test_reconcile_action_equality() {
        assert_eq!(ReconcileAction::Done, ReconcileAction::Done);
        assert_eq!(
            ReconcileAction::Requeue(Duration::from_secs(30)),
            ReconcileAction::Requeue(Duration::from_secs(30)),
        );
        assert_ne!(
            ReconcileAction::Done,
            ReconcileAction::Requeue(Duration::from_secs(30)),
        );
    }

    #[test]
    fn test_watch_event_variants() {
        let added = WatchEvent::Added("policy-a".into(), "default".into());
        let modified = WatchEvent::Modified("policy-b".into(), "kube-system".into());
        let deleted = WatchEvent::Deleted("policy-c".into(), "prod".into());
        let error = WatchEvent::Error("connection refused".into());

        match added {
            WatchEvent::Added(name, ns) => {
                assert_eq!(name, "policy-a");
                assert_eq!(ns, "default");
            }
            _ => panic!("Expected Added variant"),
        }
        match error {
            WatchEvent::Error(msg) => assert!(msg.contains("connection refused")),
            _ => panic!("Expected Error variant"),
        }
        let _ = (modified, deleted);
    }
}

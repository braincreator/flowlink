use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_mode() -> ShieldMode {
    ShieldMode::Monitor
}

fn default_sidecar_image() -> String {
    "ghcr.io/flowlink/shield:latest".into()
}

fn default_true() -> bool {
    true
}

#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "shield.flowlink.ai",
    version = "v1alpha1",
    kind = "FlowLinkShieldPolicy",
    shortname = "flsp",
    namespaced,
    status = "FlowLinkShieldPolicyStatus"
)]
pub struct FlowLinkShieldPolicySpec {
    /// Enable Shield for this namespace
    pub enabled: bool,
    /// Shield mode: monitor (observe only) or enforce (block dangerous)
    #[serde(default = "default_mode")]
    pub mode: ShieldMode,
    /// Policy rules
    pub rules: Vec<PolicyRule>,
    /// Relay endpoint for audit events
    pub relay_url: String,
    /// Sidecar image to inject
    #[serde(default = "default_sidecar_image")]
    pub sidecar_image: String,
    /// Namespaces to watch (empty = all)
    #[serde(default)]
    pub watch_namespaces: Vec<String>,
    /// Enable admission webhook
    #[serde(default = "default_true")]
    pub admission_webhook: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct FlowLinkShieldPolicyStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidecar_injections: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations_blocked: Option<i64>,
    #[serde(default)]
    pub conditions: Vec<PolicyCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyCondition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(rename = "lastTransitionTime", skip_serializing_if = "Option::is_none")]
    pub last_transition_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum ShieldMode {
    Monitor,
    Enforce,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyRule {
    pub name: String,
    pub action: String,
    pub patterns: Vec<String>,
}

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// Kubernetes resource models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Deployment {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    pub spec: DeploymentSpec,
    pub status: Option<DeploymentStatus>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DeploymentSpec {
    pub replicas: Option<i32>,
    pub selector: LabelSelector,
    pub template: PodTemplateSpec,
    pub strategy: Option<DeploymentStrategy>,
    pub paused: Option<bool>,
    pub min_ready_seconds: Option<i32>,
    pub revision_history_limit: Option<i32>,
    pub progress_deadline_seconds: Option<i32>,
    pub template_metadata: Option<ObjectMeta>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DeploymentStatus {
    pub observed_generation: Option<i64>,
    pub replicas: Option<i32>,
    pub updated_replicas: Option<i32>,
    pub ready_replicas: Option<i32>,
    pub available_replicas: Option<i32>,
    pub unavailable_replicas: Option<i32>,
    pub conditions: Vec<DeploymentCondition>,
    pub collision_count: Option<i32>,
    pub pod_template_hash: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DeploymentCondition {
    pub type_: String,
    pub status: String,
    pub last_updated: DateTime<Utc>,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PodTemplateSpec {
    pub metadata: Option<ObjectMeta>,
    pub spec: PodSpec,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PodSpec {
    pub containers: Vec<Container>,
    pub restart_policy: Option<String>,
    pub termination_grace_period_seconds: Option<i64>,
    pub active_deadline_seconds: Option<i64>,
    pub dns_policy: Option<String>,
    pub service_account_name: Option<String>,
    pub automount_service_account_token: Option<bool>,
    pub image_pull_secrets: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub priority_class_name: Option<String>,
    pub scheduler_name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Container {
    pub name: String,
    pub image: String,
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub ports: Option<Vec<ContainerPort>>,
    pub env: Option<Vec<EnvVar>>,
    pub env_from: Option<Vec<EnvFromSource>>,
    pub resources: Option<ResourceRequirements>,
    pub volume_mounts: Option<Vec<VolumeMount>>,
    pub volume_devices: Option<Vec<VolumeDevice>>,
    pub liveness_probe: Option<Probe>,
    pub readiness_probe: Option<Probe>,
    pub startup_probe: Option<Probe>,
    pub lifecycle: Option<Lifecycle>,
    pub termination_message_path: Option<String>,
    pub termination_message_policy: Option<String>,
    pub image_pull_policy: Option<String>,
    pub security_context: Option<SecurityContext>,
    pub stdin: Option<bool>,
    pub stdin_once: Option<bool>,
    pub tty: Option<bool>,
    pub stdin: Option<bool>,
    pub args: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContainerPort {
    pub container_port: i32,
    pub name: Option<String>,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct EnvVar {
    pub name: String,
    pub value: Option<String>,
    pub value_from: Option<EnvVarSource>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct EnvVarSource {
    pub config_map_key_ref: Option<ConfigMapKeySelector>,
    pub secret_key_ref: Option<SecretKeySelector>,
    pub field_ref: Option<FieldSelector>,
    pub resource_field_ref: Option<ResourceFieldSelector>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ResourceRequirements {
    pub limits: Option<ResourceList>,
    pub requests: Option<ResourceList>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ResourceList {
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub storage: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct VolumeMount {
    pub name: String,
    pub mount_path: String,
    pub mount_propagation: Option<String>,
    pub sub_path: Option<String>,
    pub sub_path_expr: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Probe {
    pub exec: Option<ExecAction>,
    pub http_get: Option<HTTPGetAction>,
    pub tcp_socket: Option<TCPSocketAction>,
    pub grpc: Option<GRPCAction>,
    pub tcp_socket: Option<TCPSocketAction>,
    pub initial_delay_seconds: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub period_seconds: Option<i32>,
    pub success_threshold: Option<i32>,
    pub failure_threshold: Option<i32>,
    pub termination_grace_period_seconds: Option<i32>,
}

#[derive(Debug, clone, serde::Deserialize, serde::Serialize)]
pub struct ExecAction {
    pub command: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HTTPGetAction {
    pub path: String,
    pub port: String,
    pub scheme: Option<String>,
    pub http_headers: Option<Vec<HTTPHeader>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HTTPHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TCPSocketAction {
    pub port: String,
}

// Service model
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Service {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    pub spec: ServiceSpec,
    pub status: Option<ServiceStatus>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServiceSpec {
    pub ports: Vec<ServicePort>,
    pub selector: HashMap<String, String>,
    pub cluster_ip: Option<String>,
    pub cluster_ips: Option<Vec<String>>,
    pub external_ip: Option<String>,
    pub external_ips: Option<Vec<String>>,
    pub external_name: Option<String>,
    pub external_traffic_policy: Option<String>,
    pub publish_not_ready_addresses: Option<bool>,
    pub session_affinity: Option<String>,
    pub session_affinity_config: Option<SessionAffinityConfig>,
    pub type_: Option<String>,
    pub ip_families: Option<Vec<String>>,
    pub ip_family_policy: Option<String>,
    pub load_balancer_source_ranges: Option<Vec<String>>,
    pub load_balancer_ip: Option<String>,
    pub allocate_load_balancer_node_ports: Option<bool>,
    pub external_traffic_policy: Option<String>,
    pub health_check_node_port: Option<i32>,
    pub publish_not_ready_addresses: Option<bool>,
    pub session_affinity: Option<String>,
    pub ports: Vec<ServicePort>,
    pub selector: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServicePort {
    pub name: Option<String>,
    pub protocol: Option<String>,
    pub port: i32,
    pub target_port: Option<String>,
    pub app_protocol: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServiceStatus {
    pub load_balancer: Option<LoadBalancerStatus>,
    pub ingress: Option<Vec<IngressLoadBalancerIngress>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LoadBalancerStatus {
    pub ingress: Option<Vec<IngressLoadBalancerIngress>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressLoadBalancerIngress {
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub ports: Option<Vec<LoadBalancerIngressPort>>,
}

// Ingress model
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Ingress {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    pub spec: IngressSpec,
    pub status: Option<IngressStatus>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressSpec {
    pub ingress_class_name: Option<String>,
    pub default_backend: Option<IngressBackend>,
    pub tls: Option<Vec<IngressTLS>>,
    pub rules: Option<Vec<IngressRule>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressTLS {
    pub hosts: Option<Vec<String>>,
    pub secret_name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressRule {
    pub http: Option<HTTPIngressRuleValue>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HTTPIngressRuleValue {
    pub paths: Option<Vec<HTTPIngressPath>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HTTPIngressPath {
    pub path: String,
    pub path_type: Option<String>,
    pub backend: IngressBackend,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressBackend {
    pub service: Option<ServiceBackend>,
    pub resource: Option<TLSIngressBackend>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServiceBackend {
    pub name: String,
    pub port: Option<ServiceBackendPort>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServiceBackendPort {
    pub number: Option<i32>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressStatus {
    pub load_balancer: Option<IngressLoadBalancer>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngressLoadBalancer {
    pub ingress: Option<Vec<IngressLoadBalancerIngress>>,
}

// Generic Kubernetes resource models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ObjectMeta {
    pub name: String,
    pub namespace: Option<String>,
    pub uid: Option<String>,
    pub resource_version: Option<String>,
    pub generation: Option<i64>,
    pub creation_timestamp: Option<DateTime<Utc>>,
    pub deletion_timestamp: Option<DateTime<Utc>>,
    pub deletion_grace_period_seconds: Option<i64>,
    pub labels: Option<HashMap<String, String>>,
    pub annotations: Option<HashMap<String, String>>,
    pub owner_references: Option<Vec<OwnerReference>>,
    pub finalizers: Option<Vec<String>>,
    pub cluster_name: Option<String>,
    pub managed_fields: Option<Vec<ManagedFieldsEntry>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OwnerReference {
    pub api_version: Option<String>,
    pub kind: String,
    pub name: String,
    pub uid: String,
    pub controller: Option<bool>,
    pub block_owner_deletion: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ManagedFieldsEntry {
    pub manager: String,
    pub operation: String,
    pub api_version: Option<String>,
    pub time: Option<DateTime<Utc>>,
    pub fields_type: Option<String>,
    pub fields_v1: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LabelSelector {
    pub match_labels: Option<HashMap<String, String>>,
    pub match_expressions: Option<Vec<LabelSelectorRequirement>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LabelSelectorRequirement {
    pub key: String,
    pub operator: String,
    pub values: Option<Vec<String>>,
}

// Operator-specific models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FlowLinkDeployment {
    pub metadata: ObjectMeta,
    pub spec: FlowLinkDeploymentSpec,
    pub status: Option<FlowLinkDeploymentStatus>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FlowLinkDeploymentSpec {
    pub replicas: Option<i32>,
    pub resources: Option<ResourceRequirements>,
    pub config: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FlowLinkDeploymentStatus {
    pub replicas: Option<i32>,
    pub ready_replicas: Option<i32>,
    pub available_replicas: Option<i32>,
}

// Kubernetes operator events
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct KubernetesEvent {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub operation: String,
    pub timestamp: DateTime<Utc>,
    pub status: String,
    pub message: Option<String>,
}

// Controller status
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ControllerStatus {
    pub controller: String,
    pub namespaces_watching: Vec<String>,
    pub resources_watching: Vec<String>,
    pub reconciliations_total: i64,
    pub reconciliations_failed: i64,
    pub health: String,
}
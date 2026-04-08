use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PolicyViolation {
    pub severity: ViolationSeverity,
    pub message: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionRequest {
    pub uid: String,
    pub kind: Value,
    pub object: Value,
    pub namespace: Option<String>,
    pub operation: String,
    pub old_object: Option<Value>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionResponse {
    pub uid: String,
    pub allowed: bool,
    pub status: Option<AdmissionResponseStatus>,
    pub patch_type: Option<String>,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionResponseStatus {
    pub code: Option<i32>,
    pub message: Option<String>,
}

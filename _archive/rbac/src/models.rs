use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// RBAC models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub resource: String,
    pub actions: Vec<String>,
    pub roles: Vec<String>,
    pub description: Option<String>,
    pub conditions: Option<Vec<PolicyCondition>>,
    pub priority: i32,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PolicyCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: String,
    pub required: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    In,
    NotIn,
    Regex,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Permission {
    pub resource: String,
    pub action: String,
    pub role: String,
    pub granted: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PermissionResult {
    pub allowed: bool,
    pub policy_id: Option<String>,
    pub required_roles: Vec<String>,
    pub user_roles: Vec<String>,
    pub granted_by_role: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RoleAssignment {
    pub id: String,
    pub user_id: String,
    pub role_id: String,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub details: HashMap<String, serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RolePermissions {
    pub role_id: String,
    pub role_name: String,
    pub permissions: Vec<Permission>,
    pub total: usize,
    pub granted: usize,
}

// Request models
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub resource: String,
    pub actions: Vec<String>,
    pub roles: Vec<String>,
    pub description: Option<String>,
    pub conditions: Option<Vec<PolicyCondition>>,
    pub priority: i32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdatePolicyRequest {
    pub name: Option<String>,
    pub actions: Option<Vec<String>>,
    pub roles: Option<Vec<String>>,
    pub description: Option<String>,
    pub conditions: Option<Vec<PolicyCondition>>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AssignRoleRequest {
    pub user_id: String,
    pub role_id: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RevokeRoleRequest {
    pub user_id: String,
    pub role_id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PermissionCheckRequest {
    pub user_id: String,
    pub resource: String,
    pub action: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BulkPermissionCheckRequest {
    pub user_id: String,
    pub checks: Vec<(String, String)>, // (resource, action)
}

// Response models
#[derive(Debug, Clone, Serialize)]
pub struct PolicyResponse {
    pub policy: Policy,
    pub can_edit: bool,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleResponse {
    pub role: Role,
    pub users_count: usize,
    pub can_edit: bool,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserResponse {
    pub user: User,
    pub roles: Vec<Role>,
    pub can_edit: bool,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditLogResponse {
    pub audit_logs: Vec<AuditLog>,
    pub total: i64,
    pub page: i32,
    pub page_size: i32,
}

// Standard roles
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_EDITOR: &str = "editor";
pub const ROLE_VIEWER: &str = "viewer";
pub const ROLE_OWNER: &str = "owner";
pub const ROLE_GUEST: &str = "guest";

// Standard resources
pub const RESOURCE_DASHBOARD: &str = "dashboard";
pub const RESOURCE_AGENTS: &str = "agents";
pub const RESOURCE_WEBHOOKS: &str = "webhooks";
pub const RESOURCE_USERS: &str = "users";
pub const RESOURCE_BILLING: &str = "billing";
pub const RESOURCE_SETTINGS: &str = "settings";
pub const RESOURCE_LOGS: &str = "logs";
pub const RESOURCE_REPORTS: &str = "reports";

// Standard actions
pub const ACTION_READ: &str = "read";
pub const ACTION_WRITE: &str = "write";
pub const ACTION_UPDATE: &str = "update";
pub const ACTION_DELETE: &str = "delete";
pub const ACTION_CREATE: &str = "create";
pub const ACTION_APPROVE: &str = "approve";
pub const ACTION_EXECUTE: &str = "execute";
pub const ACTION_MANAGE: &str = "manage";
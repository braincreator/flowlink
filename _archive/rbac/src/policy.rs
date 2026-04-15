use anyhow::Result;
use chrono::{Duration, Utc};
use std::collections::HashSet;

use super::*;
use super::models::*;

impl Policy {
    pub fn new(name: String, resource: String, actions: Vec<String>, roles: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            resource,
            actions,
            roles,
            description: None,
            conditions: None,
            priority: 0,
            is_active: true,
            expires_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self.updated_at = Utc::now();
        self
    }

    pub fn with_conditions(mut self, conditions: Vec<PolicyCondition>) -> Self {
        self.conditions = Some(conditions);
        self.updated_at = Utc::now();
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self.updated_at = Utc::now();
        self
    }

    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self.updated_at = Utc::now();
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    pub fn matches_user(&self, user_id: &str) -> bool {
        self.is_active && !self.is_expired()
    }

    pub fn get_required_roles(&self) -> Vec<String> {
        self.roles.clone()
    }
}

impl PolicyCondition {
    pub fn new(field: String, operator: ConditionOperator, value: String, required: bool) -> Self {
        Self {
            field,
            operator,
            value,
            required,
        }
    }

    pub fn evaluate(&self, context: &Context) -> bool {
        let value = context.get(&self.field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match self.operator {
            ConditionOperator::Equals => value == self.value,
            ConditionOperator::NotEquals => value != self.value,
            ConditionOperator::Contains => value.contains(&self.value),
            ConditionOperator::NotContains => !value.contains(&self.value),
            ConditionOperator::LessThan => {
                value.parse::<i64>()
                    .ok()
                    .and_then(|v| self.value.parse::<i64>().ok())
                    .map(|v| v < value)
                    .unwrap_or(false)
            }
            ConditionOperator::LessThanOrEqual => {
                value.parse::<i64>()
                    .ok()
                    .and_then(|v| self.value.parse::<i64>().ok())
                    .map(|v| v <= value)
                    .unwrap_or(false)
            }
            ConditionOperator::GreaterThan => {
                value.parse::<i64>()
                    .ok()
                    .and_then(|v| self.value.parse::<i64>().ok())
                    .map(|v| v > value)
                    .unwrap_or(false)
            }
            ConditionOperator::GreaterThanOrEqual => {
                value.parse::<i64>()
                    .ok()
                    .and_then(|v| self.value.parse::<i64>().ok())
                    .map(|v| v >= value)
                    .unwrap_or(false)
            }
            ConditionOperator::In => {
                let values: Vec<&str> = self.value.split(',').map(|s| s.trim()).collect();
                values.contains(&value)
            }
            ConditionOperator::NotIn => {
                let values: Vec<&str> = self.value.split(',').map(|s| s.trim()).collect();
                !values.contains(&value)
            }
            ConditionOperator::Regex => {
                match regex::Regex::new(&self.value) {
                    Ok(re) => re.is_match(value),
                    Err(_) => false,
                }
            }
        }
    }
}

// Policy evaluation context
pub struct Context {
    pub user_id: String,
    pub user_data: HashMap<String, serde_json::Value>,
    pub resource_data: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

impl Context {
    pub fn new(user_id: String, user_data: HashMap<String, serde_json::Value>) -> Self {
        Self {
            user_id,
            user_data,
            resource_data: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    pub fn with_resource(mut self, resource_data: HashMap<String, serde_json::Value>) -> Self {
        self.resource_data = resource_data;
        self
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.user_data.get(key)
            .or_else(|| self.resource_data.get(key))
    }
}

// Policy evaluation
pub struct PolicyEvaluator;

impl PolicyEvaluator {
    pub fn evaluate(
        policy: &Policy,
        context: &Context,
    ) -> bool {
        // Check if policy is active and not expired
        if !policy.is_active || policy.is_expired() {
            return false;
        }

        // Check if user matches policy conditions
        if let Some(conditions) = &policy.conditions {
            for condition in conditions {
                let matches = condition.evaluate(context);
                if condition.required && !matches {
                    return false;
                }
            }
        }

        // Check if user has required role
        let user_has_role = context.user_data.get("roles")
            .and_then(|v| v.as_array())
            .map(|roles| {
                roles.iter()
                    .any(|role| policy.roles.contains(&role.as_str().unwrap_or("")))
            })
            .unwrap_or(false);

        user_has_role
    }

    pub fn evaluate_bulk(
        policies: &[Policy],
        context: &Context,
    ) -> Vec<bool> {
        policies.iter()
            .map(|policy| Self::evaluate(policy, context))
            .collect()
    }
}

// Permission evaluation
pub struct PermissionEvaluator;

impl PermissionEvaluator {
    pub fn evaluate_permission(
        user_roles: &[String],
        policy: &Policy,
        action: &str,
    ) -> bool {
        // Check if action is in policy actions
        if !policy.actions.contains(&action.to_string()) {
            return false;
        }

        // Check if user has any required role
        let user_has_role = user_roles.iter()
            .any(|role| policy.roles.contains(role));

        user_has_role
    }

    pub fn evaluate_permissions(
        user_roles: &[String],
        policies: &[Policy],
        actions: &[String],
    ) -> Vec<bool> {
        let role_set: HashSet<String> = user_roles.iter().cloned().collect();

        actions.iter()
            .map(|action| {
                policies.iter()
                    .any(|policy| {
                        policy.actions.contains(action) &&
                        role_set.intersection(&policy.roles.iter().cloned().collect())
                    })
            })
            .collect()
    }
}

// Role management
pub struct RoleManager;

impl RoleManager {
    pub fn create_standard_roles() -> Vec<Role> {
        let now = Utc::now();

        vec![
            Role {
                id: uuid::Uuid::new_v4().to_string(),
                name: ROLE_ADMIN.to_string(),
                display_name: "Administrator".to_string(),
                description: Some("Full system access".to_string()),
                permissions: vec![
                    RESOURCE_DASHBOARD.to_string(),
                    RESOURCE_AGENTS.to_string(),
                    RESOURCE_WEBHOOKS.to_string(),
                    RESOURCE_USERS.to_string(),
                    RESOURCE_BILLING.to_string(),
                    RESOURCE_SETTINGS.to_string(),
                    RESOURCE_LOGS.to_string(),
                    RESOURCE_REPORTS.to_string(),
                ],
                is_system: true,
                is_active: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: uuid::Uuid::new_v4().to_string(),
                name: ROLE_EDITOR.to_string(),
                display_name: "Editor".to_string(),
                description: Some("Can edit and manage content".to_string()),
                permissions: vec![
                    RESOURCE_DASHBOARD.to_string(),
                    RESOURCE_AGENTS.to_string(),
                    RESOURCE_WEBHOOKS.to_string(),
                ],
                is_system: true,
                is_active: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: uuid::Uuid::new_v4().to_string(),
                name: ROLE_VIEWER.to_string(),
                display_name: "Viewer".to_string(),
                description: Some("Read-only access".to_string()),
                permissions: vec![
                    RESOURCE_DASHBOARD.to_string(),
                    RESOURCE_AGENTS.to_string(),
                ],
                is_system: true,
                is_active: true,
                created_at: now,
                updated_at: now,
            },
        ]
    }

    pub fn create_standard_policies() -> Vec<Policy> {
        let now = Utc::now();

        vec![
            // Dashboard policy
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Dashboard Read".to_string(),
                resource: RESOURCE_DASHBOARD.to_string(),
                actions: vec![ACTION_READ.to_string()],
                roles: vec![ROLE_VIEWER.to_string(), ROLE_EDITOR.to_string(), ROLE_ADMIN.to_string()],
                description: Some("Can view dashboard".to_string()),
                conditions: None,
                priority: 1,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Dashboard Edit".to_string(),
                resource: RESOURCE_DASHBOARD.to_string(),
                actions: vec![ACTION_UPDATE.to_string()],
                roles: vec![ROLE_EDITOR.to_string(), ROLE_ADMIN.to_string()],
                description: Some("Can edit dashboard settings".to_string()),
                conditions: None,
                priority: 2,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            // Agents policy
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Agents Read".to_string(),
                resource: RESOURCE_AGENTS.to_string(),
                actions: vec![ACTION_READ.to_string()],
                roles: vec![ROLE_VIEWER.to_string(), ROLE_EDITOR.to_string(), ROLE_ADMIN.to_string()],
                description: Some("Can view agents".to_string()),
                conditions: None,
                priority: 1,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Agents Execute".to_string(),
                resource: RESOURCE_AGENTS.to_string(),
                actions: vec![ACTION_EXECUTE.to_string()],
                roles: vec![ROLE_ADMIN.to_string()],
                description: Some("Can execute commands on agents".to_string()),
                conditions: None,
                priority: 3,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            // Webhooks policy
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Webhooks Read".to_string(),
                resource: RESOURCE_WEBHOOKS.to_string(),
                actions: vec![ACTION_READ.to_string()],
                roles: vec![ROLE_VIEWER.to_string(), ROLE_EDITOR.to_string(), ROLE_ADMIN.to_string()],
                description: Some("Can view webhooks".to_string()),
                conditions: None,
                priority: 1,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Webhooks Create".to_string(),
                resource: RESOURCE_WEBHOOKS.to_string(),
                actions: vec![ACTION_CREATE.to_string()],
                roles: vec![ROLE_EDITOR.to_string(), ROLE_ADMIN.to_string()],
                description: Some("Can create webhooks".to_string()),
                conditions: None,
                priority: 2,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Webhooks Delete".to_string(),
                resource: RESOURCE_WEBHOOKS.to_string(),
                actions: vec![ACTION_DELETE.to_string()],
                roles: vec![ROLE_ADMIN.to_string()],
                description: Some("Can delete webhooks".to_string()),
                conditions: None,
                priority: 3,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            // Billing policy
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Billing Read".to_string(),
                resource: RESOURCE_BILLING.to_string(),
                actions: vec![ACTION_READ.to_string()],
                roles: vec![ROLE_VIEWER.to_string(), ROLE_EDITOR.to_string(), ROLE_ADMIN.to_string()],
                description: Some("Can view billing info".to_string()),
                conditions: None,
                priority: 1,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            Policy {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Billing Manage".to_string(),
                resource: RESOURCE_BILLING.to_string(),
                actions: vec![ACTION_UPDATE.to_string(), ACTION_DELETE.to_string()],
                roles: vec![ROLE_ADMIN.to_string()],
                description: Some("Can manage billing".to_string()),
                conditions: None,
                priority: 4,
                is_active: true,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
        ]
    }
}
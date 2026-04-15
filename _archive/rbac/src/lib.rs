pub mod models;
pub mod policy;
pub mod permissions;
pub mod enforcement;
pub mod storage;

pub use models::*;
pub use policy::*;
pub use permissions::*;
pub use enforcement::*;
pub use storage::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

// Main RBAC orchestrator
pub struct RBACManager {
    pub policies: Arc<RwLock<Vec<Policy>>>,
    pub user_roles: Arc<RwLock<HashMap<String, Vec<String>>>>,
    pub enforcement: Arc<PolicyEnforcement>,
}

impl RBACManager {
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(Vec::new())),
            user_roles: Arc::new(RwLock::new(HashMap::new())),
            enforcement: Arc::new(PolicyEnforcement::new()),
        }
    }

    pub async fn load_policies(&self, policies: Vec<Policy>) -> Result<()> {
        let mut policies_guard = self.policies.write().await;
        policies_guard.clear();
        policies_guard.extend(policies);

        log::info!("Loaded {} RBAC policies", policies_guard.len());
        Ok(())
    }

    pub async fn load_user_roles(&self, user_id: &str, roles: Vec<String>) -> Result<()> {
        let mut user_roles_guard = self.user_roles.write().await;
        user_roles_guard.insert(user_id.to_string(), roles);

        log::info!("Loaded {} roles for user {}", roles.len(), user_id);
        Ok(())
    }

    pub async fn check_permission(
        &self,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> Result<PermissionResult> {
        let user_roles_guard = self.user_roles.read().await;
        let roles = user_roles_guard.get(user_id)
            .map(|r| r.clone())
            .unwrap_or_default();

        let policies_guard = self.policies.read().await;
        let matching_policies = policies_guard.iter()
            .filter(|p| p.resource == resource && p.actions.contains(&action.to_string()));

        for policy in matching_policies {
            // Check if user has any of the required roles
            if roles.iter().any(|role| policy.roles.contains(role)) {
                return Ok(PermissionResult {
                    allowed: true,
                    policy_id: Some(policy.id.clone()),
                    required_roles: policy.roles.clone(),
                    user_roles: roles.clone(),
                });
            }
        }

        // User doesn't have required role
        Ok(PermissionResult {
            allowed: false,
            policy_id: None,
            required_roles: vec![],
            user_roles: roles,
        })
    }

    pub async fn check_bulk_permissions(
        &self,
        user_id: &str,
        checks: Vec<(String, String)>, // (resource, action)
    ) -> Result<Vec<PermissionResult>> {
        let mut results = Vec::new();

        for (resource, action) in checks {
            results.push(self.check_permission(&user_id, &resource, &action).await?);
        }

        Ok(results)
    }

    pub async fn add_policy(&self, policy: Policy) -> Result<()> {
        let mut policies_guard = self.policies.write().await;
        policies_guard.push(policy);

        log::info!("Added RBAC policy: {} for resource {}", policy.id, policy.resource);
        Ok(())
    }

    pub async fn remove_policy(&self, policy_id: &str) -> Result<()> {
        let mut policies_guard = self.policies.write().await;
        policies_guard.retain(|p| p.id != policy_id);

        log::info!("Removed RBAC policy: {}", policy_id);
        Ok(())
    }

    pub async fn update_policy(&self, policy: Policy) -> Result<()> {
        let mut policies_guard = self.policies.write().await;

        if let Some(pos) = policies_guard.iter().position(|p| p.id == policy.id) {
            policies_guard[pos] = policy;
            log::info!("Updated RBAC policy: {}", policy.id);
        } else {
            log::warn!("Policy {} not found, creating new one", policy.id);
            policies_guard.push(policy);
        }

        Ok(())
    }

    pub async fn get_user_roles(&self, user_id: &str) -> Result<Vec<String>> {
        let user_roles_guard = self.user_roles.read().await;
        Ok(user_roles_guard.get(user_id)
            .map(|r| r.clone())
            .unwrap_or_default())
    }

    pub async fn assign_role(&self, user_id: &str, role: &str) -> Result<()> {
        let mut user_roles_guard = self.user_roles.write().await;

        let roles = user_roles_guard.entry(user_id.to_string()).or_insert_with(Vec::new);

        if !roles.contains(&role.to_string()) {
            roles.push(role.to_string());
            log::info!("Assigned role {} to user {}", role, user_id);
        }

        Ok(())
    }

    pub async fn remove_role(&self, user_id: &str, role: &str) -> Result<()> {
        let mut user_roles_guard = self.user_roles.write().await;

        if let Some(roles) = user_roles_guard.get_mut(user_id) {
            roles.retain(|r| r != role);
            log::info!("Removed role {} from user {}", role, user_id);
        }

        Ok(())
    }

    pub async fn get_user_permissions(&self, user_id: &str) -> Result<Vec<Permission>> {
        let user_roles_guard = self.user_roles.read().await;
        let roles = user_roles_guard.get(user_id)
            .map(|r| r.clone())
            .unwrap_or_default();

        let policies_guard = self.policies.read().await;
        let mut permissions = Vec::new();

        for policy in &policies_guard {
            for role in &policy.roles {
                if roles.contains(role) {
                    for action in &policy.actions {
                        permissions.push(Permission {
                            resource: policy.resource.clone(),
                            action: action.clone(),
                            role: role.clone(),
                        });
                    }
                    break;
                }
            }
        }

        Ok(permissions)
    }

    pub async fn get_role_permissions(&self, role: &str) -> Result<Vec<Permission>> {
        let policies_guard = self.policies.read().await;
        let mut permissions = Vec::new();

        for policy in &policies_guard {
            if policy.roles.contains(role) {
                for action in &policy.actions {
                    permissions.push(Permission {
                        resource: policy.resource.clone(),
                        action: action.clone(),
                        role: role.clone(),
                    });
                }
            }
        }

        Ok(permissions)
    }

    pub async fn cleanup_expired_policies(&self) -> Result<usize> {
        let mut policies_guard = self.policies.write().await;
        let initial_count = policies_guard.len();

        policies_guard.retain(|p| {
            !p.is_expired()
        });

        let removed_count = initial_count - policies_guard.len();
        log::info!("Cleaned up {} expired RBAC policies", removed_count);

        Ok(removed_count)
    }

    pub async fn get_stats(&self) -> RBACStats {
        let policies_guard = self.policies.read().await;
        let user_roles_guard = self.user_roles.read().await;

        let total_policies = policies_guard.len();
        let total_roles = policies_guard.iter()
            .flat_map(|p| p.roles.iter())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let total_users = user_roles_guard.len();
        let policies_by_resource: std::collections::HashMap<String, usize> = policies_guard.iter()
            .fold(std::collections::HashMap::new(), |mut acc, p| {
                *acc.entry(p.resource.clone()).or_insert(0) += 1;
                acc
            });

        RBACStats {
            total_policies,
            total_roles,
            total_users,
            policies_by_resource,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RBACStats {
    pub total_policies: usize,
    pub total_roles: usize,
    pub total_users: usize,
    pub policies_by_resource: std::collections::HashMap<String, usize>,
}
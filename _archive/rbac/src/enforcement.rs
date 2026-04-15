use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;
use super::policy::*;

pub struct PolicyEnforcement {
    pub rbac_manager: Arc<RBACManager>,
}

impl PolicyEnforcement {
    pub fn new() -> Self {
        Self {
            rbac_manager: Arc::new(RBACManager::new()),
        }
    }

    pub fn with_rbac_manager(mut self, rbac_manager: Arc<RBACManager>) -> Self {
        self.rbac_manager = rbac_manager;
        self
    }

    // Check if user has permission
    pub async fn check_permission(
        &self,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> Result<bool> {
        let result = self.rbac_manager.check_permission(user_id, resource, action).await?;

        Ok(result.allowed)
    }

    // Check if user has any permission
    pub async fn has_permission(
        &self,
        user_id: &str,
        resource: &str,
    ) -> Result<bool> {
        let user_roles = self.rbac_manager.get_user_roles(user_id).await?;

        let policies = self.rbac_manager.policies.read().await;
        let has_permission = policies.iter().any(|policy| {
            policy.resource == resource &&
            policy.is_active &&
            !policy.is_expired() &&
            user_roles.iter().any(|role| policy.roles.contains(role))
        });

        Ok(has_permission)
    }

    // Get user permissions
    pub async fn get_user_permissions(
        &self,
        user_id: &str,
    ) -> Result<Vec<Permission>> {
        self.rbac_manager.get_user_permissions(user_id).await
    }

    // Check if user can access resource
    pub async fn can_access_resource(
        &self,
        user_id: &str,
        resource: &str,
    ) -> Result<bool> {
        let user_roles = self.rbac_manager.get_user_roles(user_id).await?;

        let policies = self.rbac_manager.policies.read().await;
        let can_access = policies.iter().any(|policy| {
            policy.resource == resource &&
            policy.is_active &&
            !policy.is_expired() &&
            user_roles.iter().any(|role| policy.roles.contains(role))
        });

        Ok(can_access)
    }

    // Grant permission to user (requires admin)
    pub async fn grant_permission(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> Result<bool> {
        let user_has_admin_role = self.check_admin_permission(user_id).await?;

        if !user_has_admin_role {
            return Ok(false);
        }

        self.rbac_manager.assign_role(user_id, role_id).await?;

        // Log audit
        self.log_audit_log(user_id, "grant_permission", role_id).await?;

        Ok(true)
    }

    // Revoke permission from user (requires admin)
    pub async fn revoke_permission(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> Result<bool> {
        let user_has_admin_role = self.check_admin_permission(user_id).await?;

        if !user_has_admin_role {
            return Ok(false);
        }

        self.rbac_manager.remove_role(user_id, role_id).await?;

        // Log audit
        self.log_audit_log(user_id, "revoke_permission", role_id).await?;

        Ok(true)
    }

    // Check if user has admin role
    pub async fn check_admin_permission(&self, user_id: &str) -> Result<bool> {
        let user_roles = self.rbac_manager.get_user_roles(user_id).await?;
        Ok(user_roles.contains(&ROLE_ADMIN.to_string()))
    }

    // Check if user can edit resource
    pub async fn can_edit_resource(
        &self,
        user_id: &str,
        resource: &str,
    ) -> Result<bool> {
        let user_roles = self.rbac_manager.get_user_roles(user_id).await?;

        let policies = self.rbac_manager.policies.read().await;
        let can_edit = policies.iter().any(|policy| {
            policy.resource == resource &&
            policy.actions.contains(&ACTION_UPDATE.to_string()) &&
            policy.is_active &&
            !policy.is_expired() &&
            user_roles.iter().any(|role| policy.roles.contains(role))
        });

        Ok(can_edit)
    }

    // Check if user can delete resource
    pub async fn can_delete_resource(
        &self,
        user_id: &str,
        resource: &str,
    ) -> Result<bool> {
        let user_roles = self.rbac_manager.get_user_roles(user_id).await?;

        let policies = self.rbac_manager.policies.read().await;
        let can_delete = policies.iter().any(|policy| {
            policy.resource == resource &&
            policy.actions.contains(&ACTION_DELETE.to_string()) &&
            policy.is_active &&
            !policy.is_expired() &&
            user_roles.iter().any(|role| policy.roles.contains(role))
        });

        Ok(can_delete)
    }

    // Create audit log
    async fn log_audit_log(&self, user_id: &str, action: &str, resource: &str) -> Result<()> {
        // TODO: Implement actual audit log storage
        log::info!("Audit log: user={}, action={}, resource={}", user_id, action, resource);
        Ok(())
    }
}

// Middleware for Next.js API routes
pub struct PermissionMiddleware;

impl PermissionMiddleware {
    pub fn require_permission<T>(
        user_roles: &[String],
        resource: &str,
        action: &str,
    ) -> Result<T> {
        let user_has_permission = PermissionEvaluator::evaluate_permission(user_roles, &Policy {}, action);

        if !user_has_permission {
            return Err(anyhow::anyhow!(
                "Permission denied: {}:{} for role",
                resource,
                action
            ));
        }

        Ok(()) // Return empty tuple for success
    }

    pub fn require_admin<T>(user_roles: &[String]) -> Result<T> {
        if !user_roles.contains(&ROLE_ADMIN.to_string()) {
            return Err(anyhow::anyhow!("Admin permission required"));
        }

        Ok(())
    }

    pub fn require_editor<T>(user_roles: &[String]) -> Result<T> {
        if !user_roles.contains(&ROLE_EDITOR.to_string()) {
            return Err(anyhow::anyhow!("Editor permission required"));
        }

        Ok(())
    }

    pub fn require_viewer<T>(user_roles: &[String]) -> Result<T> {
        if !user_roles.contains(&ROLE_VIEWER.to_string()) {
            return Err(anyhow::anyhow!("Viewer permission required"));
        }

        Ok(())
    }
}
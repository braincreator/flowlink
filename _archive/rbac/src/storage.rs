use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct RBACStorage {
    pub pool: Arc<PgPool>,
}

impl RBACStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS rbac_policies (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                resource TEXT NOT NULL,
                actions JSONB NOT NULL,
                roles JSONB NOT NULL,
                description TEXT,
                conditions JSONB,
                priority INTEGER NOT NULL DEFAULT 0,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                expires_at TIMESTAMP,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_rbac_policies_resource ON rbac_policies(resource);
            CREATE INDEX IF NOT EXISTS idx_rbac_policies_active ON rbac_policies(is_active, expires_at);

            CREATE TABLE IF NOT EXISTS rbac_user_roles (
                id SERIAL PRIMARY KEY,
                user_id TEXT NOT NULL,
                role_id TEXT NOT NULL,
                assigned_at TIMESTAMP NOT NULL DEFAULT NOW(),
                assigned_by TEXT,
                expires_at TIMESTAMP,
                UNIQUE(user_id, role_id)
            );

            CREATE INDEX IF NOT EXISTS idx_rbac_user_roles_user_id ON rbac_user_roles(user_id);
            CREATE INDEX IF NOT EXISTS idx_rbac_user_roles_role_id ON rbac_user_roles(role_id);

            CREATE TABLE IF NOT EXISTS rbac_audit_logs (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT NOT NULL,
                details JSONB,
                ip_address TEXT,
                user_agent TEXT,
                timestamp TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_rbac_audit_logs_user_id ON rbac_audit_logs(user_id);
            CREATE INDEX IF NOT EXISTS idx_rbac_audit_logs_timestamp ON rbac_audit_logs(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_rbac_audit_logs_action ON rbac_audit_logs(action);
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("RBAC storage tables created successfully");
        Ok(())
    }

    pub async fn save_policy(&self, policy: &Policy) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO rbac_policies (
                id, name, resource, actions, roles, description,
                conditions, priority, is_active, expires_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                resource = EXCLUDED.resource,
                actions = EXCLUDED.actions,
                roles = EXCLUDED.roles,
                description = EXCLUDED.description,
                conditions = EXCLUDED.conditions,
                priority = EXCLUDED.priority,
                is_active = EXCLUDED.is_active,
                expires_at = EXCLUDED.expires_at,
                updated_at = EXCLUDED.updated_at
        "#;

        let actions_json = serde_json::to_value(&policy.actions).unwrap_or(serde_json::Value::Null);
        let roles_json = serde_json::to_value(&policy.roles).unwrap_or(serde_json::Value::Null);
        let conditions_json = policy.conditions.as_ref()
            .map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);

        sqlx::query(insert_sql)
            .bind(&policy.id)
            .bind(&policy.name)
            .bind(&policy.resource)
            .bind(&actions_json)
            .bind(&roles_json)
            .bind(&policy.description)
            .bind(&conditions_json)
            .bind(policy.priority)
            .bind(policy.is_active)
            .bind(policy.expires_at)
            .bind(policy.created_at)
            .bind(policy.updated_at)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_policy(&self, policy_id: &str) -> Result<Option<Policy>> {
        let query = r#"
            SELECT id, name, resource, actions, roles, description,
                   conditions, priority, is_active, expires_at, created_at, updated_at
            FROM rbac_policies
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(policy_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let actions_json: serde_json::Value = row.try_get("actions")?;
                let roles_json: serde_json::Value = row.try_get("roles")?;
                let conditions_json: Option<serde_json::Value> = row.try_get("conditions")?;

                let actions: Vec<String> = serde_json::from_value(actions_json)?;
                let roles: Vec<String> = serde_json::from_value(roles_json)?;

                Ok(Some(Policy {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    resource: row.try_get("resource")?,
                    actions,
                    roles,
                    description: row.try_get("description")?,
                    conditions: conditions_json.map(|c| serde_json::from_value(c).unwrap_or_default()),
                    priority: row.try_get("priority")?,
                    is_active: row.try_get("is_active")?,
                    expires_at: row.try_get("expires_at")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn list_policies(&self, resource: Option<&str>) -> Result<Vec<Policy>> {
        let query = if let Some(r) = resource {
            "SELECT id, name, resource, actions, roles, description, conditions, priority, is_active, expires_at, created_at, updated_at FROM rbac_policies WHERE resource = $1"
        } else {
            "SELECT id, name, resource, actions, roles, description, conditions, priority, is_active, expires_at, created_at, updated_at FROM rbac_policies"
        };

        let rows = if let Some(r) = resource {
            sqlx::query(query).bind(r).fetch_all(self.pool.clone()).await?
        } else {
            sqlx::query(query).fetch_all(self.pool.clone()).await?
        };

        let mut policies = Vec::new();

        for row in rows {
            let actions_json: serde_json::Value = row.try_get("actions")?;
            let roles_json: serde_json::Value = row.try_get("roles")?;
            let conditions_json: Option<serde_json::Value> = row.try_get("conditions")?;

            let actions: Vec<String> = serde_json::from_value(actions_json)?;
            let roles: Vec<String> = serde_json::from_value(roles_json)?;

            policies.push(Policy {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                resource: row.try_get("resource")?,
                actions,
                roles,
                description: row.try_get("description")?,
                conditions: conditions_json.map(|c| serde_json::from_value(c).unwrap_or_default()),
                priority: row.try_get("priority")?,
                is_active: row.try_get("is_active")?,
                expires_at: row.try_get("expires_at")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(policies)
    }

    pub async fn save_user_role(&self, user_id: &str, role_id: &str, assigned_by: Option<&str>) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO rbac_user_roles (user_id, role_id, assigned_at, assigned_by)
            VALUES ($1, $2, NOW(), $3)
            ON CONFLICT (user_id, role_id) DO UPDATE SET
                assigned_at = NOW(),
                assigned_by = EXCLUDED.assigned_by
        "#;

        sqlx::query(insert_sql)
            .bind(user_id)
            .bind(role_id)
            .bind(assigned_by)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_user_roles(&self, user_id: &str) -> Result<Vec<String>> {
        let query = r#"
            SELECT role_id FROM rbac_user_roles WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())
        "#;

        let rows = sqlx::query(query)
            .bind(user_id)
            .fetch_all(self.pool.clone())
            .await?;

        let roles: Vec<String> = rows.iter()
            .map(|row| row.try_get("role_id").unwrap_or_default())
            .collect();

        Ok(roles)
    }

    pub async fn get_role_by_id(&self, role_id: &str) -> Result<Option<Role>> {
        let query = r#"
            SELECT id, name, display_name, description, permissions, is_system, is_active, created_at, updated_at
            FROM rbac_roles
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(role_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(Some(Role {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    display_name: row.try_get("display_name")?,
                    description: row.try_get("description")?,
                    permissions: row.try_get("permissions")?,
                    is_system: row.try_get("is_system")?,
                    is_active: row.try_get("is_active")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_all_roles(&self) -> Result<Vec<Role>> {
        let query = r#"
            SELECT id, name, display_name, description, permissions, is_system, is_active, created_at, updated_at
            FROM rbac_roles
        "#;

        let rows = sqlx::query(query).fetch_all(self.pool.clone()).await?;

        let mut roles = Vec::new();

        for row in rows {
            roles.push(Role {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                display_name: row.try_get("display_name")?,
                description: row.try_get("description")?,
                permissions: row.try_get("permissions")?,
                is_system: row.try_get("is_system")?,
                is_active: row.try_get("is_active")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(roles)
    }

    pub async fn delete_user_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let query = "DELETE FROM rbac_user_roles WHERE user_id = $1 AND role_id = $2";

        sqlx::query(query)
            .bind(user_id)
            .bind(role_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn delete_policy(&self, policy_id: &str) -> Result<()> {
        let query = "DELETE FROM rbac_policies WHERE id = $1";

        sqlx::query(query)
            .bind(policy_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_expired_policies(&self) -> Result<i64> {
        let query = r#"
            DELETE FROM rbac_policies
            WHERE is_active = FALSE OR (expires_at IS NOT NULL AND expires_at < NOW())
        "#;

        let result = sqlx::query(query)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} expired RBAC policies", deleted_count);

        Ok(deleted_count)
    }

    pub async fn cleanup_expired_user_roles(&self) -> Result<i64> {
        let query = r#"
            DELETE FROM rbac_user_roles
            WHERE expires_at IS NOT NULL AND expires_at < NOW()
        "#;

        let result = sqlx::query(query)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} expired RBAC user roles", deleted_count);

        Ok(deleted_count)
    }
}

// In-memory storage for testing
pub struct InMemoryRBACStorage {
    pub policies: Arc<RwLock<HashMap<String, Policy>>>,
    pub user_roles: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl InMemoryRBACStorage {
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            user_roles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save_policy(&self, policy: &Policy) -> Result<()> {
        self.policies.write().await.insert(policy.id.clone(), policy.clone());
        Ok(())
    }

    pub async fn get_policy(&self, policy_id: &str) -> Result<Option<Policy>> {
        Ok(self.policies.read().await.get(policy_id).cloned())
    }

    pub async fn save_user_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write().await;
        user_roles.entry(user_id.to_string())
            .or_insert_with(Vec::new)
            .push(role_id.to_string());
        Ok(())
    }

    pub async fn get_user_roles(&self, user_id: &str) -> Result<Vec<String>> {
        Ok(self.user_roles.read().await.get(user_id)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn delete_user_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write().await;
        if let Some(roles) = user_roles.get_mut(user_id) {
            roles.retain(|r| r != role_id);
        }
        Ok(())
    }
}
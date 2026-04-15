use anyhow::Result;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct CIStorage {
    pub pool: Arc<PgPool>,
}

impl CIStorage {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(PgPool::none()),
        }
    }

    pub fn set_pool(&mut self, pool: Arc<PgPool>) {
        self.pool = pool;
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS ci_events (
                id SERIAL PRIMARY KEY,
                event_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                event_type TEXT NOT NULL,
                repository TEXT,
                timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
                status TEXT NOT NULL,
                metadata JSONB DEFAULT '{}'::jsonb
            );

            CREATE INDEX IF NOT EXISTS idx_ci_events_event_id ON ci_events(event_id);
            CREATE INDEX IF NOT EXISTS idx_ci_events_provider ON ci_events(provider);
            CREATE INDEX IF NOT EXISTS idx_ci_events_event_type ON ci_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_ci_events_repository ON ci_events(repository);
            CREATE INDEX IF NOT EXISTS idx_ci_events_status ON ci_events(status);
            CREATE INDEX IF NOT EXISTS idx_ci_events_timestamp ON ci_events(timestamp DESC);

            CREATE TABLE IF NOT EXISTS ci_projects (
                id SERIAL PRIMARY KEY,
                project_id TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                repository_url TEXT NOT NULL,
                branch TEXT NOT NULL,
                environment_mappings JSONB DEFAULT '{}'::jsonb,
                branch_filters TEXT[],
                auto_approve BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_ci_projects_project_id ON ci_projects(project_id);
            CREATE INDEX IF NOT EXISTS idx_ci_projects_repository_url ON ci_projects(repository_url);

            CREATE TABLE IF NOT EXISTS ci_deployments (
                id SERIAL PRIMARY KEY,
                deployment_id TEXT UNIQUE NOT NULL,
                project_id TEXT NOT NULL,
                branch TEXT NOT NULL,
                commit_sha TEXT NOT NULL,
                provider TEXT NOT NULL,
                environment TEXT NOT NULL,
                status TEXT NOT NULL,
                artifacts JSONB DEFAULT '[]'::jsonb,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_ci_deployments_deployment_id ON ci_deployments(deployment_id);
            CREATE INDEX IF NOT EXISTS idx_ci_deployments_project_id ON ci_deployments(project_id);
            CREATE INDEX IF NOT EXISTS idx_ci_deployments_status ON ci_deployments(status);
            CREATE INDEX IF NOT EXISTS idx_ci_deployments_created_at ON ci_deployments(created_at DESC);
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("CI/CD storage tables created successfully");
        Ok(())
    }

    pub async fn save_event(&self, event: &CIEvent) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO ci_events (
                event_id, provider, event_type, repository, timestamp, status, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (event_id) DO UPDATE SET
                status = EXCLUDED.status,
                metadata = EXCLUDED.metadata,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&event.id)
            .bind(&event.provider)
            .bind(&event.event_type)
            .bind(&event.repository)
            .bind(event.timestamp)
            .bind(&event.status)
            .bind(serde_json::to_value(&event.metadata)?)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_event(&self, event_id: &str) -> Result<Option<CIEvent>> {
        let query = r#"
            SELECT event_id, provider, event_type, repository, timestamp, status, metadata
            FROM ci_events
            WHERE event_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(event_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let metadata: HashMap<String, String> = serde_json::from_value(row.try_get("metadata")?)?;

                Ok(Some(CIEvent {
                    id: row.try_get("event_id")?,
                    provider: row.try_get("provider")?,
                    event_type: row.try_get("event_type")?,
                    repository: row.try_get("repository")?,
                    timestamp: row.try_get("timestamp")?,
                    status: row.try_get("status")?,
                    metadata,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_events_by_provider(&self, provider: &str, limit: i32) -> Result<Vec<CIEvent>> {
        let query = r#"
            SELECT event_id, provider, event_type, repository, timestamp, status, metadata
            FROM ci_events
            WHERE provider = $1
            ORDER BY timestamp DESC
            LIMIT $2
        "#;

        let rows = sqlx::query(query)
            .bind(provider)
            .bind(limit)
            .fetch_all(self.pool.clone())
            .await?;

        let mut events = Vec::new();

        for row in rows {
            let metadata: HashMap<String, String> = serde_json::from_value(row.try_get("metadata")?)?;

            events.push(CIEvent {
                id: row.try_get("event_id")?,
                provider: row.try_get("provider")?,
                event_type: row.try_get("event_type")?,
                repository: row.try_get("repository")?,
                timestamp: row.try_get("timestamp")?,
                status: row.try_get("status")?,
                metadata,
            });
        }

        Ok(events)
    }

    pub async fn get_events_by_repository(&self, repository: &str, limit: i32) -> Result<Vec<CIEvent>> {
        let query = r#"
            SELECT event_id, provider, event_type, repository, timestamp, status, metadata
            FROM ci_events
            WHERE repository = $1
            ORDER BY timestamp DESC
            LIMIT $2
        "#;

        let rows = sqlx::query(query)
            .bind(repository)
            .bind(limit)
            .fetch_all(self.pool.clone())
            .await?;

        let mut events = Vec::new();

        for row in rows {
            let metadata: HashMap<String, String> = serde_json::from_value(row.try_get("metadata")?)?;

            events.push(CIEvent {
                id: row.try_get("event_id")?,
                provider: row.try_get("provider")?,
                event_type: row.try_get("event_type")?,
                repository: row.try_get("repository")?,
                timestamp: row.try_get("timestamp")?,
                status: row.try_get("status")?,
                metadata,
            });
        }

        Ok(events)
    }

    pub async fn get_recent_events(&self, limit: i32) -> Result<Vec<CIEvent>> {
        let query = r#"
            SELECT event_id, provider, event_type, repository, timestamp, status, metadata
            FROM ci_events
            ORDER BY timestamp DESC
            LIMIT $1
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .fetch_all(self.pool.clone())
            .await?;

        let mut events = Vec::new();

        for row in rows {
            let metadata: HashMap<String, String> = serde_json::from_value(row.try_get("metadata")?)?;

            events.push(CIEvent {
                id: row.try_get("event_id")?,
                provider: row.try_get("provider")?,
                event_type: row.try_get("event_type")?,
                repository: row.try_get("repository")?,
                timestamp: row.try_get("timestamp")?,
                status: row.try_get("status")?,
                metadata,
            });
        }

        Ok(events)
    }

    pub async fn save_deployment(&self, deployment: &DeploymentEnvironment) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO ci_deployments (
                deployment_id, project_id, branch, commit_sha, provider, environment,
                status, artifacts, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            ON CONFLICT (deployment_id) DO UPDATE SET
                status = EXCLUDED.status,
                artifacts = EXCLUDED.artifacts,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&deployment.deployment_id)
            .bind(deployment.name.to_lowercase().replace(" ", "-"))
            .bind("main") // TODO: Get actual branch
            .bind("latest") // TODO: Get actual commit
            .bind(&deployment.name)
            .bind(&deployment.name)
            .bind(deployment.status.to_string())
            .bind(serde_json::to_value(&deployment.artifacts)?)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_deployment(&self, deployment_id: &str) -> Result<Option<DeploymentEnvironment>> {
        let query = r#"
            SELECT deployment_id, project_id, branch, commit_sha, provider, environment,
                   status, artifacts, created_at, updated_at
            FROM ci_deployments
            WHERE deployment_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(deployment_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let artifacts: Vec<BuildArtifact> = serde_json::from_value(row.try_get("artifacts")?)?;
                let status = match row.try_get::<String, _>("status")?.as_str() {
                    "pending" => CIStatus::Pending,
                    "running" => CIStatus::Running,
                    "success" => CIStatus::Success,
                    "failed" => CIStatus::Failed,
                    "canceled" => CIStatus::Canceled,
                    _ => CIStatus::Pending,
                };

                Ok(Some(DeploymentEnvironment {
                    name: row.try_get("environment")?,
                    url: format!("https://{}.flowlink.dev", row.try_get("project_id")?),
                    status,
                    deployment_id: row.try_get("deployment_id")?,
                    timestamp: row.try_get("created_at")?,
                    artifacts,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_deployments_by_status(&self, status: CIStatus) -> Result<Vec<DeploymentEnvironment>> {
        let query = r#"
            SELECT deployment_id, project_id, branch, commit_sha, provider, environment,
                   status, artifacts, created_at, updated_at
            FROM ci_deployments
            WHERE status = $1
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(status.to_string())
            .fetch_all(self.pool.clone())
            .await?;

        let mut deployments = Vec::new();

        for row in rows {
            let artifacts: Vec<BuildArtifact> = serde_json::from_value(row.try_get("artifacts")?)?;
            let status = match row.try_get::<String, _>("status")?.as_str() {
                "pending" => CIStatus::Pending,
                "running" => CIStatus::Running,
                "success" => CIStatus::Success,
                "failed" => CIStatus::Failed,
                "canceled" => CIStatus::Canceled,
                _ => CIStatus::Pending,
            };

            deployments.push(DeploymentEnvironment {
                name: row.try_get("environment")?,
                url: format!("https://{}.flowlink.dev", row.try_get("project_id")?),
                status,
                deployment_id: row.try_get("deployment_id")?,
                timestamp: row.try_get("created_at")?,
                artifacts,
            });
        }

        Ok(deployments)
    }

    pub async fn update_deployment_status(&self, deployment_id: &str, status: CIStatus) -> Result<()> {
        let update_sql = r#"
            UPDATE ci_deployments
            SET status = $1, updated_at = NOW()
            WHERE deployment_id = $2
        "#;

        sqlx::query(update_sql)
            .bind(status.to_string())
            .bind(deployment_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_old_events(&self, days: i32) -> Result<i64> {
        let query = r#"
            DELETE FROM ci_events
            WHERE created_at < NOW() - INTERVAL '1 day' * $1
        "#;

        let result = sqlx::query(query)
            .bind(days)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} old CI events", deleted_count);

        Ok(deleted_count)
    }

    pub async fn cleanup_old_deployments(&self, days: i32) -> Result<i64> {
        let query = r#"
            DELETE FROM ci_deployments
            WHERE created_at < NOW() - INTERVAL '1 day' * $1
        "#;

        let result = sqlx::query(query)
            .bind(days)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} old deployments", deleted_count);

        Ok(deleted_count)
    }
}

// In-memory storage for testing
pub struct InMemoryCIStorage {
    pub events: Arc<RwLock<Vec<CIEvent>>>,
    pub deployments: Arc<RwLock<HashMap<String, DeploymentEnvironment>>>,
}

impl InMemoryCIStorage {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            deployments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save_event(&self, event: &CIEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event.clone());
        Ok(())
    }

    pub async fn get_recent_events(&self, limit: i32) -> Result<Vec<CIEvent>> {
        let events = self.events.read().await;
        Ok(events.iter().rev().take(limit as usize).cloned().collect())
    }

    pub async fn save_deployment(&self, deployment: &DeploymentEnvironment) -> Result<()> {
        let mut deployments = self.deployments.write().await;
        deployments.insert(deployment.deployment_id.clone(), deployment.clone());
        Ok(())
    }

    pub async fn get_deployment(&self, deployment_id: &str) -> Result<Option<DeploymentEnvironment>> {
        let deployments = self.deployments.read().await;
        Ok(deployments.get(deployment_id).cloned())
    }

    pub async fn cleanup_old_events(&self, _days: i32) -> Result<i64> {
        Ok(0)
    }
}
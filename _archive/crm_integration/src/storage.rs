use anyhow::Result;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct CRMStorage {
    pub pool: Arc<PgPool>,
}

impl CRMStorage {
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
            CREATE TABLE IF NOT EXISTS crm_webhook_events (
                id SERIAL PRIMARY KEY,
                event_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload JSONB NOT NULL,
                response JSONB,
                processed_at TIMESTAMP NOT NULL DEFAULT NOW(),
                status TEXT NOT NULL DEFAULT 'processed',
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_crm_webhook_events_event_id ON crm_webhook_events(event_id);
            CREATE INDEX IF NOT EXISTS idx_crm_webhook_events_provider ON crm_webhook_events(provider);
            CREATE INDEX IF NOT EXISTS idx_crm_webhook_events_event_type ON crm_webhook_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_crm_webhook_events_created_at ON crm_webhook_events(created_at DESC);

            CREATE TABLE IF NOT EXISTS crm_sync_logs (
                id SERIAL PRIMARY KEY,
                provider TEXT NOT NULL,
                status TEXT NOT NULL,
                entities_synced INTEGER NOT NULL DEFAULT 0,
                errors TEXT[],
                warnings TEXT[],
                started_at TIMESTAMP NOT NULL DEFAULT NOW(),
                completed_at TIMESTAMP,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_crm_sync_logs_provider ON crm_sync_logs(provider);
            CREATE INDEX IF NOT EXISTS idx_crm_sync_logs_status ON crm_sync_logs(status);
            CREATE INDEX IF NOT EXISTS idx_crm_sync_logs_started_at ON crm_sync_logs(started_at DESC);

            CREATE TABLE IF NOT EXISTS crm_entity_mappings (
                id SERIAL PRIMARY KEY,
                provider TEXT NOT NULL,
                local_entity TEXT NOT NULL,
                crm_entity TEXT NOT NULL,
                field_mappings JSONB NOT NULL DEFAULT '{}'::jsonb,
                is_active BOOLEAN DEFAULT TRUE,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_crm_entity_mappings_provider ON crm_entity_mappings(provider);
            CREATE INDEX IF NOT EXISTS idx_crm_entity_mappings_local_entity ON crm_entity_mappings(local_entity);
            CREATE INDEX IF NOT EXISTS idx_crm_entity_mappings_crm_entity ON crm_entity_mappings(crm_entity);

            CREATE TABLE IF NOT EXISTS crm_flows (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                provider TEXT,
                trigger_events TEXT[] NOT NULL,
                flow_config JSONB NOT NULL,
                is_active BOOLEAN DEFAULT TRUE,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_crm_flows_name ON crm_flows(name);
            CREATE INDEX IF NOT EXISTS idx_crm_flows_provider ON crm_flows(provider);
            CREATE INDEX IF NOT EXISTS idx_crm_flows_is_active ON crm_flows(is_active);

            CREATE TABLE IF NOT EXISTS crm_entities (
                id SERIAL PRIMARY KEY,
                provider TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                provider_entity_id TEXT NOT NULL,
                local_entity_id TEXT,
                name TEXT NOT NULL,
                data JSONB DEFAULT '{}'::jsonb,
                sync_status TEXT DEFAULT 'pending',
                last_synced_at TIMESTAMP,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_crm_entities_provider ON crm_entities(provider);
            CREATE INDEX IF NOT EXISTS idx_crm_entities_entity_type ON crm_entities(entity_type);
            CREATE INDEX IF NOT EXISTS idx_crm_entities_provider_entity_id ON crm_entities(provider, provider_entity_id);
            CREATE INDEX IF NOT EXISTS idx_crm_entities_sync_status ON crm_entities(sync_status);
            CREATE INDEX IF NOT EXISTS idx_crm_entities_last_synced_at ON crm_entities(last_synced_at DESC);

            CREATE TABLE IF NOT EXISTS crm_analytics (
                id SERIAL PRIMARY KEY,
                provider TEXT NOT NULL,
                metric TEXT NOT NULL,
                value NUMERIC NOT NULL,
                date DATE NOT NULL,
                metadata JSONB DEFAULT '{}'::jsonb,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_crm_analytics_provider ON crm_analytics(provider);
            CREATE INDEX IF NOT EXISTS idx_crm_analytics_metric ON crm_analytics(metric);
            CREATE INDEX IF NOT EXISTS idx_crm_analytics_date ON crm_analytics(date);
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("CRM storage tables created successfully");
        Ok(())
    }

    pub async fn save_webhook_event(&self, event: &CRMWebhookEvent) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO crm_webhook_events (
                event_id, provider, event_type, payload, response, status
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (event_id) DO UPDATE SET
                payload = EXCLUDED.payload,
                response = EXCLUDED.response,
                status = EXCLUDED.status,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&event.id)
            .bind(&event.provider)
            .bind(&event.event_type)
            .bind(serde_json::to_value(&event.data)?)
            .bind(serde_json::to_value(event)?)
            .bind("processed")
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_webhook_event(&self, event_id: &str) -> Result<Option<CRMWebhookEvent>> {
        let query = r#"
            SELECT event_id, provider, event_type, payload, response, status, processed_at
            FROM crm_webhook_events
            WHERE event_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(event_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(Some(CRMWebhookEvent {
                    id: row.try_get("event_id")?,
                    provider: row.try_get("provider")?,
                    event_type: row.try_get("event_type")?,
                    data: row.try_get("payload")?,
                    timestamp: row.try_get("processed_at")?,
                    metadata: HashMap::new(),
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_recent_webhook_events(&self, limit: i32) -> Result<Vec<CRMWebhookEvent>> {
        let query = r#"
            SELECT event_id, provider, event_type, payload, response, status, processed_at
            FROM crm_webhook_events
            ORDER BY processed_at DESC
            LIMIT $1
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .fetch_all(self.pool.clone())
            .await?;

        let mut events = Vec::new();

        for row in rows {
            events.push(CRMWebhookEvent {
                id: row.try_get("event_id")?,
                provider: row.try_get("provider")?,
                event_type: row.try_get("event_type")?,
                data: row.try_get("payload")?,
                timestamp: row.try_get("processed_at")?,
                metadata: HashMap::new(),
            });
        }

        Ok(events)
    }

    pub async fn save_sync_log(&self, log: &CRMSyncLog) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO crm_sync_logs (
                provider, status, entities_synced, errors, warnings, started_at, completed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                entities_synced = EXCLUDED.entities_synced,
                errors = EXCLUDED.errors,
                warnings = EXCLUDED.warnings,
                completed_at = EXCLUDED.completed_at,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&log.provider)
            .bind(format!("{:?}", log.status))
            .bind(log.entities_synced)
            .bind(serde_json::to_value(&log.errors)?)
            .bind(serde_json::to_value(&log.warnings)?)
            .bind(log.started_at)
            .bind(log.completed_at)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_sync_logs(&self, provider: Option<&str>, limit: i32) -> Result<Vec<CRMSyncLog>> {
        let mut query = "SELECT provider, status, entities_synced, errors, warnings, started_at, completed_at, id, created_at FROM crm_sync_logs";
        let mut query_params = Vec::new();

        if let Some(p) = provider {
            query += " WHERE provider = $1";
            query_params.push(p);
        }

        query += " ORDER BY started_at DESC LIMIT $";
        let param_index = if provider.is_some() { 2 } else { 1 };
        query.push_str(&param_index.to_string());

        let mut sqlx_query = sqlx::query(query);

        for (i, param) in query_params.iter().enumerate() {
            sqlx_query = sqlx_query.bind(*param);
        }
        if provider.is_none() {
            sqlx_query = sqlx_query.bind(limit);
        }

        if provider.is_some() {
            sqlx_query = sqlx_query.bind(limit);
        }

        let rows = sqlx_query.fetch_all(self.pool.clone()).await?;

        let mut logs = Vec::new();

        for row in rows {
            let errors: Vec<String> = serde_json::from_value(row.try_get("errors")?)?;
            let warnings: Vec<String> = serde_json::from_value(row.try_get("warnings")?)?;
            let status = match row.try_get::<String, _>("status")?.as_str() {
                "Synced" => CRMSyncStatus::Synced,
                "Pending" => CRMSyncStatus::Pending,
                "Failed" => CRMSyncStatus::Failed,
                "Scheduled" => CRMSyncStatus::Scheduled,
                _ => CRMSyncStatus::Pending,
            };

            logs.push(CRMSyncLog {
                id: row.try_get("id")?,
                provider: row.try_get("provider")?,
                status,
                entities_synced: row.try_get("entities_synced")?,
                errors,
                warnings,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
            });
        }

        Ok(logs)
    }

    pub async fn save_entity_mapping(&self, mapping: &CRMEntityMapping) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO crm_entity_mappings (
                provider, local_entity, crm_entity, field_mappings, is_active
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (provider, local_entity) DO UPDATE SET
                crm_entity = EXCLUDED.crm_entity,
                field_mappings = EXCLUDED.field_mappings,
                is_active = EXCLUDED.is_active,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&mapping.provider)
            .bind(&mapping.local_entity)
            .bind(&mapping.crm_entity)
            .bind(serde_json::to_value(&mapping.field_mappings)?)
            .bind(mapping.is_active)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_entity_mappings(&self, provider: &str, local_entity: &str) -> Result<Option<CRMEntityMapping>> {
        let query = r#"
            SELECT provider, local_entity, crm_entity, field_mappings, is_active
            FROM crm_entity_mappings
            WHERE provider = $1 AND local_entity = $2 AND is_active = TRUE
        "#;

        let row = sqlx::query(query)
            .bind(provider)
            .bind(local_entity)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(Some(CRMEntityMapping {
                    provider: row.try_get("provider")?,
                    local_entity: row.try_get("local_entity")?,
                    crm_entity: row.try_get("crm_entity")?,
                    field_mappings: row.try_get("field_mappings")?,
                    is_active: row.try_get("is_active")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn save_flow(&self, flow: &CRMFlow) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO crm_flows (
                name, description, provider, trigger_events, flow_config, is_active
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (name) DO UPDATE SET
                description = EXCLUDED.description,
                provider = EXCLUDED.provider,
                trigger_events = EXCLUDED.trigger_events,
                flow_config = EXCLUDED.flow_config,
                is_active = EXCLUDED.is_active,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&flow.name)
            .bind(&flow.description)
            .bind(&flow.provider)
            .bind(serde_json::to_value(&flow.trigger_events)?)
            .bind(serde_json::to_value(&flow.steps)?)
            .bind(flow.active)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_flow(&self, name: &str) -> Result<Option<CRMFlow>> {
        let query = r#"
            SELECT name, description, provider, trigger_events, flow_config, is_active
            FROM crm_flows
            WHERE name = $1 AND is_active = TRUE
        "#;

        let row = sqlx::query(query)
            .bind(name)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let trigger_events: Vec<String> = serde_json::from_value(row.try_get("trigger_events")?)?;
                let flow_config: Vec<CRMFlowStep> = serde_json::from_value(row.try_get("flow_config")?)?;

                Ok(Some(CRMFlow {
                    name: row.try_get("name")?,
                    description: row.try_get("description")?,
                    provider: row.try_get("provider")?,
                    trigger_events,
                    steps: flow_config,
                    active: row.try_get("is_active")?,
                    created_at: row.try_get("created_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn save_entity(&self, entity: &CRMEntity) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO crm_entities (
                provider, entity_type, provider_entity_id, local_entity_id, name, data, sync_status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (provider, provider_entity_id) DO UPDATE SET
                local_entity_id = EXCLUDED.local_entity_id,
                name = EXCLUDED.name,
                data = EXCLUDED.data,
                sync_status = EXCLUDED.sync_status,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&entity.provider)
            .bind(&entity.entity_type)
            .bind(&entity.provider_entity_id)
            .bind(&entity.local_entity_id)
            .bind(&entity.name)
            .bind(serde_json::to_value(&entity.data)?)
            .bind(&entity.sync_status)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_entity(&self, provider: &str, entity_type: &str, provider_entity_id: &str) -> Result<Option<CRMEntity>> {
        let query = r#"
            SELECT provider, entity_type, provider_entity_id, local_entity_id, name, data, sync_status, last_synced_at
            FROM crm_entities
            WHERE provider = $1 AND entity_type = $2 AND provider_entity_id = $3
        "#;

        let row = sqlx::query(query)
            .bind(provider)
            .bind(entity_type)
            .bind(provider_entity_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(Some(CRMEntity {
                    provider: row.try_get("provider")?,
                    entity_type: row.try_get("entity_type")?,
                    provider_entity_id: row.try_get("provider_entity_id")?,
                    local_entity_id: row.try_get("local_entity_id")?,
                    name: row.try_get("name")?,
                    data: row.try_get("data")?,
                    sync_status: row.try_get("sync_status")?,
                    last_synced_at: row.try_get("last_synced_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_entities_by_provider(&self, provider: &str, entity_type: Option<&str>) -> Result<Vec<CRMEntity>> {
        let mut query = "SELECT provider, entity_type, provider_entity_id, local_entity_id, name, data, sync_status, last_synced_at FROM crm_entities WHERE provider = $1";
        let mut params = vec![provider];

        if let Some(et) = entity_type {
            query += " AND entity_type = $2";
            params.push(et);
        }

        query += " ORDER BY updated_at DESC";

        let mut sqlx_query = sqlx::query(query);
        for (i, param) in params.iter().enumerate() {
            sqlx_query = sqlx_query.bind(*param);
        }

        let rows = sqlx_query.fetch_all(self.pool.clone()).await?;

        let mut entities = Vec::new();

        for row in rows {
            entities.push(CRMEntity {
                provider: row.try_get("provider")?,
                entity_type: row.try_get("entity_type")?,
                provider_entity_id: row.try_get("provider_entity_id")?,
                local_entity_id: row.try_get("local_entity_id")?,
                name: row.try_get("name")?,
                data: row.try_get("data")?,
                sync_status: row.try_get("sync_status")?,
                last_synced_at: row.try_get("last_synced_at")?,
            });
        }

        Ok(entities)
    }

    pub async fn update_entity_sync_status(&self, provider: &str, entity_type: &str, provider_entity_id: &str, status: String) -> Result<()> {
        let update_sql = r#"
            UPDATE crm_entities
            SET sync_status = $1, last_synced_at = CASE WHEN $2 = 'synced' THEN NOW() ELSE last_synced_at END, updated_at = NOW()
            WHERE provider = $3 AND entity_type = $4 AND provider_entity_id = $5
        "#;

        sqlx::query(update_sql)
            .bind(&status)
            .bind(&status)
            .bind(provider)
            .bind(entity_type)
            .bind(provider_entity_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_old_events(&self, days: i32) -> Result<i64> {
        let query = r#"
            DELETE FROM crm_webhook_events
            WHERE processed_at < NOW() - INTERVAL '1 day' * $1
        "#;

        let result = sqlx::query(query)
            .bind(days)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} old CRM webhook events", deleted_count);

        Ok(deleted_count)
    }

    pub async fn cleanup_old_sync_logs(&self, days: i32) -> Result<i64> {
        let query = r#"
            DELETE FROM crm_sync_logs
            WHERE started_at < NOW() - INTERVAL '1 day' * $1
        "#;

        let result = sqlx::query(query)
            .bind(days)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} old CRM sync logs", deleted_count);

        Ok(deleted_count)
    }
}

// In-memory storage for testing
pub struct InMemoryCRMStorage {
    pub webhook_events: Arc<RwLock<Vec<CRMWebhookEvent>>>,
    pub sync_logs: Arc<RwLock<Vec<CRMSyncLog>>>,
    pub entity_mappings: Arc<RwLock<HashMap<String, CRMEntityMapping>>>,
    pub flows: Arc<RwLock<HashMap<String, CRMFlow>>>,
    pub entities: Arc<RwLock<Vec<CRMEntity>>>,
}

impl InMemoryCRMStorage {
    pub fn new() -> Self {
        Self {
            webhook_events: Arc::new(RwLock::new(Vec::new())),
            sync_logs: Arc::new(RwLock::new(Vec::new())),
            entity_mappings: Arc::new(RwLock::new(HashMap::new())),
            flows: Arc::new(RwLock::new(HashMap::new())),
            entities: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn save_webhook_event(&self, event: &CRMWebhookEvent) -> Result<()> {
        let mut events = self.webhook_events.write().await;
        events.push(event.clone());
        Ok(())
    }

    pub async fn get_recent_webhook_events(&self, limit: i32) -> Result<Vec<CRMWebhookEvent>> {
        let events = self.webhook_events.read().await;
        Ok(events.iter().rev().take(limit as usize).cloned().collect())
    }

    pub async fn save_sync_log(&self, log: &CRMSyncLog) -> Result<()> {
        let mut logs = self.sync_logs.write().await;
        logs.push(log.clone());
        Ok(())
    }

    pub async fn get_sync_logs(&self, _provider: Option<&str>, _limit: i32) -> Result<Vec<CRMSyncLog>> {
        let logs = self.sync_logs.read().await;
        Ok(logs.clone())
    }

    pub async fn cleanup_old_events(&self, _days: i32) -> Result<i64> {
        Ok(0)
    }

    pub async fn cleanup_old_sync_logs(&self, _days: i32) -> Result<i64> {
        Ok(0)
    }
}
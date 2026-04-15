use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;

pub struct WebhookStorage {
    pub pool: Arc<PgPool>,
}

impl WebhookStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    
    pub async fn create_tables(&self) -> Result<()> {
        let create_table_sql = r#"
            CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                service TEXT NOT NULL,
                data TEXT NOT NULL,
                timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
                headers JSONB,
                ip_address TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_webhooks_service ON webhooks(service);
            CREATE INDEX IF NOT EXISTS idx_webhooks_timestamp ON webhooks(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_webhooks_created_at ON webhooks(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_webhooks_service_timestamp ON webhooks(service, timestamp DESC);

            CREATE TABLE IF NOT EXISTS webhook_metadata (
                id SERIAL PRIMARY KEY,
                webhook_id TEXT REFERENCES webhooks(id) ON DELETE CASCADE,
                key TEXT NOT NULL,
                value JSONB NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS webhook_retry_logs (
                id SERIAL PRIMARY KEY,
                webhook_id TEXT REFERENCES webhooks(id) ON DELETE CASCADE,
                attempt_number INTEGER NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS webhook_metrics (
                service TEXT PRIMARY KEY,
                received_count INTEGER NOT NULL DEFAULT 0,
                routed_count INTEGER NOT NULL DEFAULT 0,
                failed_count INTEGER NOT NULL DEFAULT 0,
                last_received_at TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );
        "#;
        
        sqlx::query(create_table_sql)
            .execute(self.pool.clone())
            .await?;
        
        log::info!("Webhook storage tables created successfully");
        Ok(())
    }
    
    pub async fn store_webhook(&self, webhook: &Webhook) -> Result<()> {
        // Store main webhook data
        let insert_webhook_sql = r#"
            INSERT INTO webhooks (id, service, data, timestamp, headers, ip_address)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                data = EXCLUDED.data,
                timestamp = EXCLUDED.timestamp,
                headers = EXCLUDED.headers
        "#;
        
        let headers_json = serde_json::to_value(&webhook.headers).unwrap_or(Value::Null);
        
        sqlx::query(insert_webhook_sql)
            .bind(&webhook.id)
            .bind(&webhook.service)
            .bind(&webhook.data)
            .bind(webhook.timestamp)
            .bind(&headers_json)
            .bind(&webhook.ip_address)
            .execute(self.pool.clone())
            .await?;
        
        log::debug!("Stored webhook {} from service {}", webhook.id, webhook.service);
        
        Ok(())
    }
    
    pub async fn get_webhook(&self, id: &str) -> Result<Option<Webhook>> {
        let query = r#"
            SELECT id, service, data, timestamp, headers, ip_address
            FROM webhooks
            WHERE id = $1
        "#;
        
        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(self.pool.clone())
            .await?;
        
        match row {
            Some(row) => {
                let headers_json: Value = row.try_get("headers")?;
                let headers: HashMap<hyper::HeaderName, hyper::HeaderName> = serde_json::from_value(headers_json)?;
                
                Ok(Some(Webhook {
                    id: row.try_get("id")?,
                    service: row.try_get("service")?,
                    data: row.try_get("data")?,
                    timestamp: row.try_get("timestamp")?,
                    headers,
                    ip_address: row.try_get("ip_address")?,
                }))
            }
            None => Ok(None),
        }
    }
    
    pub async fn get_webhooks(&self, service: &str, limit: i64, offset: i64) -> Result<Vec<Webhook>> {
        let query = r#"
            SELECT id, service, data, timestamp, headers, ip_address
            FROM webhooks
            WHERE service = $1
            ORDER BY timestamp DESC
            LIMIT $2 OFFSET $3
        "#;
        
        let rows = sqlx::query(query)
            .bind(service)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.pool.clone())
            .await?;
        
        let mut webhooks = Vec::new();
        
        for row in rows {
            let headers_json: Value = row.try_get("headers")?;
            let headers: HashMap<hyper::HeaderName, hyper::HeaderName> = serde_json::from_value(headers_json)?;
            
            webhooks.push(Webhook {
                id: row.try_get("id")?,
                service: row.try_get("service")?,
                data: row.try_get("data")?,
                timestamp: row.try_get("timestamp")?,
                headers,
                ip_address: row.try_get("ip_address")?,
            });
        }
        
        Ok(webhooks)
    }
    
    pub async fn delete_webhook(&self, id: &str) -> Result<()> {
        let query = "DELETE FROM webhooks WHERE id = $1";
        
        sqlx::query(query)
            .bind(id)
            .execute(self.pool.clone())
            .await?;
        
        log::debug!("Deleted webhook {}", id);
        Ok(())
    }
    
    pub async fn cleanup_old_webhooks(&self, days: i32) -> Result<i64> {
        let query = r#"
            DELETE FROM webhooks
            WHERE timestamp < NOW() - INTERVAL '1 day' * $1
        "#;
        
        let result = sqlx::query(query)
            .bind(days)
            .execute(self.pool.clone())
            .await?;
        
        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} old webhooks", deleted_count);
        
        Ok(deleted_count)
    }
    
    pub async fn increment_metrics(&self, service: &str, received: bool, routed: bool, failed: bool) -> Result<()> {
        // Update metrics in a transaction
        let update_metrics = r#"
            INSERT INTO webhook_metrics (service, received_count, routed_count, failed_count, last_received_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (service) DO UPDATE SET
                received_count = webhook_metrics.received_count + EXCLUDED.received_count,
                routed_count = webhook_metrics.routed_count + EXCLUDED.routed_count,
                failed_count = webhook_metrics.failed_count + EXCLUDED.failed_count,
                last_received_at = NOW(),
                updated_at = NOW()
        "#;
        
        sqlx::query(update_metrics)
            .bind(service)
            .bind(if received { 1i64 } else { 0 })
            .bind(if routed { 1i64 } else { 0 })
            .bind(if failed { 1i64 } else { 0 })
            .execute(self.pool.clone())
            .await?;
        
        Ok(())
    }
    
    pub async fn get_metrics(&self) -> Result<Vec<MetricsRecord>> {
        let query = r#"
            SELECT service, received_count, routed_count, failed_count, last_received_at, updated_at
            FROM webhook_metrics
            ORDER BY received_count DESC
        "#;
        
        let rows = sqlx::query(query)
            .fetch_all(self.pool.clone())
            .await?;
        
        let mut metrics = Vec::new();
        
        for row in rows {
            metrics.push(MetricsRecord {
                service: row.try_get("service")?,
                received_count: row.try_get("received_count")?,
                routed_count: row.try_get("routed_count")?,
                failed_count: row.try_get("failed_count")?,
                last_received_at: row.try_get("last_received_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        
        Ok(metrics)
    }
    
    pub async fn get_service_webhook_count(&self, service: &str, hours: i32) -> Result<i64> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM webhooks
            WHERE service = $1 AND timestamp >= NOW() - INTERVAL '1 hour' * $2
        "#;
        
        let row = sqlx::query(query)
            .bind(service)
            .bind(hours)
            .fetch_one(self.pool.clone())
            .await?;
        
        Ok(row.try_get("count")?)
    }
}

pub struct MetricsRecord {
    pub service: String,
    pub received_count: i64,
    pub routed_count: i64,
    pub failed_count: i64,
    pub last_received_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

pub struct InMemoryWebhookStorage {
    pub webhooks: Arc<RwLock<HashMap<String, Webhook>>>,
    pub metrics: Arc<RwLock<HashMap<String, StorageMetrics>>>,
}

impl InMemoryWebhookStorage {
    pub fn new() -> Self {
        Self {
            webhooks: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn store_webhook(&self, webhook: &Webhook) -> Result<()> {
        self.webhooks.write().await.insert(webhook.id.clone(), webhook.clone());
        
        // Update metrics
        let mut metrics = self.metrics.write().await;
        let service_metrics = metrics.entry(webhook.service.clone()).or_insert(StorageMetrics {
            received: 0,
            routed: 0,
            failed: 0,
            last_received: Utc::now(),
        });
        service_metrics.received += 1;
        service_metrics.last_received = Utc::now();
        
        log::debug!("Stored webhook {} from service {}", webhook.id, webhook.service);
        
        Ok(())
    }
    
    pub async fn get_webhook(&self, id: &str) -> Result<Option<Webhook>> {
        Ok(self.webhooks.read().await.get(id).cloned())
    }
    
    pub async fn get_webhooks(&self, service: &str, limit: i64, offset: i64) -> Result<Vec<Webhook>> {
        let webhooks = self.webhooks.read().await;
        let filtered: Vec<_> = webhooks.values()
            .filter(|w| w.service == service)
            .cloned()
            .collect();
        
        let from = offset as usize;
        let to = (offset + limit) as usize;
        
        Ok(filtered[from..to.min(filtered.len())].to_vec())
    }
    
    pub async fn delete_webhook(&self, id: &str) -> Result<()> {
        self.webhooks.write().await.remove(id);
        Ok(())
    }
    
    pub async fn cleanup_old_webhooks(&self, days: i32) -> Result<i64> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let mut count = 0;
        
        let mut webhooks = self.webhooks.write().await;
        webhooks.retain(|_, webhook| webhook.timestamp >= cutoff);
        count = webhooks.len() as i64;
        
        log::info!("Cleaned up {} old webhooks", count);
        Ok(count)
    }
    
    pub async fn get_metrics(&self) -> Result<Vec<MetricsRecord>> {
        let metrics = self.metrics.read().await;
        
        Ok(metrics.iter()
            .map(|(service, m)| MetricsRecord {
                service: service.clone(),
                received_count: m.received,
                routed_count: m.routed,
                failed_count: m.failed,
                last_received_at: m.last_received,
                updated_at: m.last_received,
            })
            .collect())
    }
}

pub struct StorageMetrics {
    pub received: i64,
    pub routed: i64,
    pub failed: i64,
    pub last_received: chrono::DateTime<Utc>,
}
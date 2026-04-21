//! Webhook Delivery Engine — background task for sending webhooks with retry

use std::sync::Arc;
use std::time::Duration;


use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_type: String,
    pub org_id: String,
    pub agent_id: Option<String>,
    pub timestamp: String,
    pub data: serde_json::Value,
}

pub struct WebhookEngine {
    pool: Arc<PgPool>,
    client: reqwest::Client,
}

impl WebhookEngine {
    pub fn new(pool: Arc<PgPool>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        Self { pool, client }
    }

    /// Enqueue a webhook event for delivery
    pub async fn enqueue(&self, event: &WebhookEvent) -> Result<()> {
        // Find all active webhooks for this org matching this event type
        let webhooks: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, url, secret FROM webhooks WHERE org_id = $1::uuid AND is_active = true AND ($2 = ANY(events) OR 'all' = ANY(events))"
        )
        .bind(&event.org_id)
        .bind(&event.event_type)
        .fetch_all(&*self.pool)
        .await?;

        let payload = json!({
            "event": event.event_type,
            "org_id": event.org_id,
            "agent_id": event.agent_id,
            "timestamp": event.timestamp,
            "data": event.data,
        });

        for (webhook_id, _url, _secret) in webhooks {
            sqlx::query(
                "INSERT INTO webhook_deliveries (webhook_id, event_type, payload, status) VALUES ($1, $2, $3, 'pending')"
            )
            .bind(webhook_id)
            .bind(&event.event_type)
            .bind(&payload)
            .execute(&*self.pool)
            .await?;
        }

        Ok(())
    }

    /// Process pending deliveries (called periodically)
    pub async fn process_pending(&self) -> Result<usize> {
        let pending: Vec<(Uuid, Uuid, String, String, serde_json::Value)> = sqlx::query_as(
            r#"SELECT wd.id, wd.webhook_id, w.url, w.secret, wd.payload
               FROM webhook_deliveries wd
               JOIN webhooks w ON w.id = wd.webhook_id
               WHERE wd.status IN ('pending', 'retrying')
                 AND (wd.next_retry_at IS NULL OR wd.next_retry_at <= NOW())
               ORDER BY wd.created_at
               LIMIT 50"#
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut processed = 0;
        for (delivery_id, _webhook_id, url, secret, payload) in pending {
            let body = payload.to_string();

            // Compute HMAC-SHA256 signature
            let signature = {
                // HMAC-SHA256 signature
                use hmac::Mac;
                let mut mac: hmac::Hmac<sha2::Sha256> = Mac::new_from_slice(secret.as_bytes())
                    .expect("HMAC key");
                mac.update(body.as_bytes());
                let result = mac.finalize().into_bytes();
                format!("sha256={}", hex::encode(result))
            };

            match self.client.post(&url)
                .header("Content-Type", "application/json")
                .header("X-FlowLink-Signature", &signature)
                .header("X-FlowLink-Event", "event")
                .body(body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16() as i32;
                    if resp.status().is_success() {
                        sqlx::query(
                            "UPDATE webhook_deliveries SET status = 'sent', attempts = attempts + 1, last_attempt_at = NOW(), response_code = $1 WHERE id = $2"
                        )
                        .bind(status)
                        .bind(delivery_id)
                        .execute(&*self.pool)
                        .await?;
                    } else {
                        // Failed but got response, schedule retry
                        self.schedule_retry(delivery_id, status).await?;
                    }
                    processed += 1;
                }
                Err(e) => {
                    // Network error, schedule retry
                    sqlx::query(
                        "UPDATE webhook_deliveries SET status = 'retrying', attempts = attempts + 1, last_attempt_at = NOW(), error_message = $1 WHERE id = $2"
                    )
                    .bind(e.to_string())
                    .bind(delivery_id)
                    .execute(&*self.pool)
                    .await?;
                    self.schedule_retry(delivery_id, 0).await?;
                    processed += 1;
                }
            }
        }

        Ok(processed)
    }

    async fn schedule_retry(&self, delivery_id: Uuid, response_code: i32) -> Result<()> {
        // Get current attempts
        let attempts: Option<i32> = sqlx::query_scalar(
            "SELECT attempts FROM webhook_deliveries WHERE id = $1"
        )
        .bind(delivery_id)
        .fetch_optional(&*self.pool)
        .await?
        .flatten();

        let attempts = attempts.unwrap_or(0);

        if attempts >= 5 {
            // Max retries reached, mark as failed
            sqlx::query(
                "UPDATE webhook_deliveries SET status = 'failed', response_code = $1 WHERE id = $2"
            )
            .bind(response_code)
            .bind(delivery_id)
            .execute(&*self.pool)
            .await?;
        } else {
            // Exponential backoff: 30s, 2m, 10m, 30m, 1h
            let delays = [30, 120, 600, 1800, 3600];
            let delay_secs = delays.get(attempts as usize).copied().unwrap_or(3600);

            sqlx::query(
                "UPDATE webhook_deliveries SET next_retry_at = NOW() + ($1 || ' seconds')::interval, response_code = $2 WHERE id = $3"
            )
            .bind(delay_secs as i32)
            .bind(response_code)
            .bind(delivery_id)
            .execute(&*self.pool)
            .await?;
        }

        Ok(())
    }
}

/// Background task runner for webhook delivery
pub async fn webhook_delivery_worker(pool: Arc<PgPool>) {
    let engine = WebhookEngine::new(pool);
    loop {
        match engine.process_pending().await {
            Ok(n) => {
                if n > 0 {
                    log::debug!("Webhook delivery: {} processed", n);
                }
            }
            Err(e) => {
                log::warn!("Webhook delivery error: {}", e);
            }
        }
        sleep(Duration::from_secs(30)).await;
    }
}

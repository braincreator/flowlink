//! Webhook persistence — CRUD + trigger

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WebhookRow {
    pub id: Uuid,
    pub org_id: String,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_triggered_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct WebhookRepo;

impl WebhookRepo {
    pub async fn create(
        pool: &PgPool,
        org_id: &str,
        url: &str,
        secret: &str,
        events: &[String],
    ) -> Result<WebhookRow> {
        let row = sqlx::query_as::<_, WebhookRow>(
            "INSERT INTO webhooks (org_id, url, secret, events) VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(org_id)
        .bind(url)
        .bind(secret)
        .bind(events)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_by_org(pool: &PgPool, org_id: &str) -> Result<Vec<WebhookRow>> {
        let rows = sqlx::query_as::<_, WebhookRow>(
            "SELECT * FROM webhooks WHERE org_id = $1 ORDER BY created_at DESC",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete(pool: &PgPool, id: Uuid, org_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM webhooks WHERE id = $1 AND org_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get(pool: &PgPool, id: Uuid, org_id: &str) -> Result<Option<WebhookRow>> {
        let row = sqlx::query_as::<_, WebhookRow>(
            "SELECT * FROM webhooks WHERE id = $1 AND org_id = $2",
        )
        .bind(id)
        .bind(org_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Find all active webhooks for an org that subscribe to a given event type.
    pub async fn find_matching(pool: &PgPool, org_id: &str, event_type: &str) -> Result<Vec<WebhookRow>> {
        let rows = sqlx::query_as::<_, WebhookRow>(
            "SELECT * FROM webhooks WHERE org_id = $1 AND is_active = true AND $2 = ANY(events)",
        )
        .bind(org_id)
        .bind(event_type)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Update last_triggered_at for a webhook.
    pub async fn touch(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE webhooks SET last_triggered_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Fire all matching webhooks for an event (fire-and-forget).
/// Callers should spawn this in a tokio task.
pub async fn trigger_webhooks(pool: &PgPool, org_id: &str, event_type: &str, payload: Value) {
    let webhooks = match WebhookRepo::find_matching(pool, org_id, event_type).await {
        Ok(w) => w,
        Err(e) => {
            log::warn!("Failed to find matching webhooks: {}", e);
            return;
        }
    };

    for wh in webhooks {
        let id = wh.id;
        let url = wh.url.clone();
        let secret = wh.secret.clone();
        let payload = payload.clone();
        let event_type_owned = event_type.to_string();

        tokio::spawn(async move {
            // Compute HMAC-SHA256 signature
            let body = serde_json::to_string(&payload).unwrap_or_default();
            let signature = {
                use hmac::{Hmac, Mac};
                use sha2::Sha256;
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
                mac.update(body.as_bytes());
                format!("sha256={:x}", mac.finalize().into_bytes())
            };

            let client = reqwest::Client::new();
            let result = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("X-FlowLink-Signature", &signature)
                .header("X-FlowLink-Event", &event_type_owned)
                .body(body)
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    log::info!("Webhook {} delivered to {} (status {})", id, url, resp.status());
                }
                Ok(resp) => {
                    log::warn!("Webhook {} failed: {} -> status {}", id, url, resp.status());
                }
                Err(e) => {
                    log::warn!("Webhook {} delivery error: {} -> {}", id, url, e);
                }
            }
        });
    }
}

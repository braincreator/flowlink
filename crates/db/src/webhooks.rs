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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn now() -> chrono::DateTime<chrono::Utc> { Utc::now() }

    fn make_webhook() -> WebhookRow {
        WebhookRow {
            id: uuid::Uuid::new_v4(),
            org_id: "org_001".to_string(),
            url: "https://example.com/webhook".to_string(),
            secret: "whsec_abc123".to_string(),
            events: vec!["policy.changed".to_string(), "member.added".to_string()],
            is_active: true,
            created_at: now(),
            last_triggered_at: None,
        }
    }

    #[test]
    fn webhook_row_construction() {
        let w = make_webhook();
        assert_eq!(w.org_id, "org_001");
        assert_eq!(w.url, "https://example.com/webhook");
        assert_eq!(w.events.len(), 2);
        assert!(w.is_active);
    }

    #[test]
    fn webhook_row_none_last_triggered() {
        let w = make_webhook();
        assert!(w.last_triggered_at.is_none());
    }

    #[test]
    fn webhook_row_some_last_triggered() {
        let mut w = make_webhook();
        w.last_triggered_at = Some(now());
        assert!(w.last_triggered_at.is_some());
    }

    #[test]
    fn webhook_row_clone() {
        let w = make_webhook();
        let cloned = w.clone();
        assert_eq!(cloned.id, w.id);
        assert_eq!(cloned.org_id, w.org_id);
        assert_eq!(cloned.events, w.events);
    }

    #[test]
    fn webhook_row_debug() {
        let w = make_webhook();
        let debug = format!("{:?}", w);
        assert!(debug.contains("webhook"));
    }

    #[test]
    fn webhook_row_empty_events() {
        let mut w = make_webhook();
        w.events = Vec::new();
        assert!(w.events.is_empty());
    }

    #[test]
    fn webhook_row_multiple_events() {
        let mut w = make_webhook();
        w.events = vec![
            "policy.changed".to_string(),
            "member.added".to_string(),
            "member.removed".to_string(),
            "plan.upgraded".to_string(),
        ];
        assert_eq!(w.events.len(), 4);
    }

    #[test]
    fn webhook_row_is_active_true() {
        let w = make_webhook();
        assert!(w.is_active);
    }

    #[test]
    fn webhook_row_is_active_false() {
        let mut w = make_webhook();
        w.is_active = false;
        assert!(!w.is_active);
    }

    #[test]

    #[test]
    fn webhook_repo_exists() {
        let _repo = WebhookRepo;
    }
}

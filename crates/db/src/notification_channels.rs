//! User notification channel bindings — per-user channel preferences.
//!
//! Each user can bind multiple notification channels (Telegram, MAX, Slack, email).
//! The NotificationRouter resolves which channels to use for a given account_id.

use sqlx::PgPool;

/// A bound notification channel for a user account.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserChannel {
    pub id: uuid::Uuid,
    pub account_id: String,
    /// Channel type: telegram, max, slack, email, webhook
    pub channel_type: String,
    /// Channel-specific address (TG chat_id, MAX user_id, Slack webhook, email)
    pub channel_address: String,
    /// User display name on this channel (for message formatting)
    pub display_name: Option<String>,
    /// Whether this is the user's primary channel
    pub is_primary: bool,
    /// Whether the binding has been verified (e.g., user confirmed in TG)
    pub verified: bool,
    /// Per-user mute settings: which categories to skip
    /// JSON array of categories, e.g. ["system","audit"]
    pub mute_categories: Option<serde_json::Value>,
    /// Per-user minimum severity: only send at or above this level
    /// info, warning, alert, critical
    pub min_severity: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct UserChannelRepo;

impl UserChannelRepo {
    /// Bind a notification channel to an account.
    /// If a binding for same account+channel_type+channel_address exists, update it.
    pub async fn upsert(
        pool: &PgPool,
        account_id: &str,
        channel_type: &str,
        channel_address: &str,
        display_name: Option<&str>,
        is_primary: bool,
    ) -> anyhow::Result<UserChannel> {
        let row = sqlx::query_as::<_, UserChannel>(
            r#"INSERT INTO user_notification_channels (account_id, channel_type, channel_address, display_name, is_primary)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (account_id, channel_type, channel_address)
               DO UPDATE SET display_name = COALESCE(EXCLUDED.display_name, user_notification_channels.display_name),
                             is_primary = EXCLUDED.is_primary,
                             updated_at = NOW()
               RETURNING *"#,
        )
        .bind(account_id)
        .bind(channel_type)
        .bind(channel_address)
        .bind(display_name)
        .bind(is_primary)
        .fetch_one(pool)
        .await?;

        Ok(row)
    }

    /// Get all notification channels for an account.
    pub async fn list_for_account(pool: &PgPool, account_id: &str) -> anyhow::Result<Vec<UserChannel>> {
        let rows = sqlx::query_as::<_, UserChannel>(
            "SELECT * FROM user_notification_channels WHERE account_id = $1 AND verified = true ORDER BY is_primary DESC, channel_type",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get the primary channel for an account.
    pub async fn get_primary(pool: &PgPool, account_id: &str) -> anyhow::Result<Option<UserChannel>> {
        let row = sqlx::query_as::<_, UserChannel>(
            "SELECT * FROM user_notification_channels WHERE account_id = $1 AND is_primary = true AND verified = true LIMIT 1",
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Get channels of a specific type for an account.
    pub async fn get_by_type(
        pool: &PgPool,
        account_id: &str,
        channel_type: &str,
    ) -> anyhow::Result<Option<UserChannel>> {
        let row = sqlx::query_as::<_, UserChannel>(
            "SELECT * FROM user_notification_channels WHERE account_id = $1 AND channel_type = $2 AND verified = true LIMIT 1",
        )
        .bind(account_id)
        .bind(channel_type)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Mark a binding as verified.
    pub async fn verify(pool: &PgPool, id: uuid::Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE user_notification_channels SET verified = true, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Set primary channel (unsets others for same account).
    pub async fn set_primary(pool: &PgPool, id: uuid::Uuid) -> anyhow::Result<bool> {
        // Get the channel first to find account_id
        let ch = sqlx::query_as::<_, UserChannel>(
            "SELECT * FROM user_notification_channels WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        if let Some(ch) = ch {
            // Unset all primaries for this account
            sqlx::query(
                "UPDATE user_notification_channels SET is_primary = false, updated_at = NOW() WHERE account_id = $1",
            )
            .bind(&ch.account_id)
            .execute(pool)
            .await?;

            // Set this one as primary
            let result = sqlx::query(
                "UPDATE user_notification_channels SET is_primary = true, updated_at = NOW() WHERE id = $1",
            )
            .bind(id)
            .execute(pool)
            .await?;

            return Ok(result.rows_affected() > 0);
        }

        Ok(false)
    }

    /// Unbind a channel.
    pub async fn delete(pool: &PgPool, id: uuid::Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_notification_channels WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update mute categories.
    pub async fn set_mute_categories(
        pool: &PgPool,
        id: uuid::Uuid,
        categories: Vec<&str>,
    ) -> anyhow::Result<bool> {
        let json = serde_json::json!(categories);
        let result = sqlx::query(
            "UPDATE user_notification_channels SET mute_categories = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(json)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update minimum severity filter.
    pub async fn set_min_severity(
        pool: &PgPool,
        id: uuid::Uuid,
        severity: &str,
    ) -> anyhow::Result<bool> {
        let valid = ["info", "warning", "alert", "critical"];
        if !valid.contains(&severity) {
            anyhow::bail!("Invalid severity: {severity}. Must be one of: {}", valid.join(", "));
        }

        let result = sqlx::query(
            "UPDATE user_notification_channels SET min_severity = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(severity)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_channel() -> UserChannel {
        UserChannel {
            id: uuid::Uuid::new_v4(),
            account_id: "acct_123".to_string(),
            channel_type: "telegram".to_string(),
            channel_address: "123456".to_string(),
            display_name: Some("Test User".to_string()),
            is_primary: true,
            verified: true,
            mute_categories: Some(serde_json::json!(["system", "audit"])),
            min_severity: Some("warning".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn user_channel_construction() {
        let ch = make_channel();
        assert_eq!(ch.account_id, "acct_123");
        assert_eq!(ch.channel_type, "telegram");
        assert_eq!(ch.channel_address, "123456");
    }

    #[test]
    fn user_channel_clone() {
        let ch = make_channel();
        let cloned = ch.clone();
        assert_eq!(cloned.id, ch.id);
        assert_eq!(cloned.account_id, ch.account_id);
        assert_eq!(cloned.channel_type, ch.channel_type);
    }

    #[test]
    fn user_channel_debug() {
        let ch = make_channel();
        let debug = format!("{:?}", ch);
        assert!(debug.contains("telegram"));
    }

    #[test]
    fn user_channel_none_display_name() {
        let mut ch = make_channel();
        ch.display_name = None;
        assert!(ch.display_name.is_none());
    }

    #[test]
    fn user_channel_some_display_name() {
        let ch = make_channel();
        assert_eq!(ch.display_name.as_deref(), Some("Test User"));
    }

    #[test]
    fn user_channel_none_mute_categories() {
        let mut ch = make_channel();
        ch.mute_categories = None;
        assert!(ch.mute_categories.is_none());
    }

    #[test]
    fn user_channel_some_mute_categories() {
        let ch = make_channel();
        let cats = ch.mute_categories.unwrap();
        assert_eq!(cats.as_array().unwrap().len(), 2);
    }

    #[test]
    fn user_channel_none_min_severity() {
        let mut ch = make_channel();
        ch.min_severity = None;
        assert!(ch.min_severity.is_none());
    }

    #[test]
    fn user_channel_some_min_severity() {
        let ch = make_channel();
        assert_eq!(ch.min_severity.as_deref(), Some("warning"));
    }

    #[test]
    fn user_channel_is_primary_true() {
        let ch = make_channel();
        assert!(ch.is_primary);
    }

    #[test]
    fn user_channel_is_primary_false() {
        let mut ch = make_channel();
        ch.is_primary = false;
        assert!(!ch.is_primary);
    }

    #[test]
    fn user_channel_verified_true() {
        let ch = make_channel();
        assert!(ch.verified);
    }

    #[test]
    fn user_channel_verified_false() {
        let mut ch = make_channel();
        ch.verified = false;
        assert!(!ch.verified);
    }

    #[test]

    #[test]
    fn user_channel_different_channel_types() {
        for ctype in &["telegram", "max", "slack", "email", "webhook"] {
            let mut ch = make_channel();
            ch.channel_type = ctype.to_string();
            assert_eq!(ch.channel_type, *ctype);
        }
    }

    #[test]
    fn user_channel_repo_exists() {
        let _repo = UserChannelRepo;
    }
}

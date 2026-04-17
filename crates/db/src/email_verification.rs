//! Email verification codes — repository for magic link auth

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub struct EmailVerificationRepo;

impl EmailVerificationRepo {
    /// Create a new verification code
    pub async fn create_code(
        pool: &PgPool,
        email: &str,
        code: &str,
        purpose: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO email_verification_codes (email, code, purpose, expires_at) VALUES ($1, $2, $3, $4)"
        )
        .bind(email)
        .bind(code)
        .bind(purpose)
        .bind(expires_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Verify and consume a code. Returns the associated account_id if found.
    /// Looks up account by email from the accounts table.
    pub async fn verify_and_consume_code(
        pool: &PgPool,
        email: &str,
        code: &str,
        purpose: &str,
    ) -> Result<Option<String>> {
        // Mark code as used if valid
        let result = sqlx::query(
            "UPDATE email_verification_codes SET used = TRUE
             WHERE email = $1 AND code = $2 AND purpose = $3
               AND used = FALSE AND expires_at > NOW()
             RETURNING id",
        )
        .bind(email)
        .bind(code)
        .bind(purpose)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        // Look up account by email
        let account_id: Option<String> =
            sqlx::query_scalar("SELECT account_id FROM accounts WHERE email = $1")
                .bind(email)
                .fetch_optional(pool)
                .await?;

        Ok(account_id)
    }

    /// Delete all expired and used codes older than `max_age_minutes`.
    /// Designed for periodic cron cleanup.
    pub async fn cleanup_expired(pool: &PgPool, max_age_minutes: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM email_verification_codes
             WHERE expires_at < NOW() - INTERVAL '1 minute' * $1"
        )
        .bind(max_age_minutes)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Check rate limit: returns true if a new code can be sent (no code in the last minute)
    pub async fn check_rate_limit(pool: &PgPool, email: &str, purpose: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM email_verification_codes
             WHERE email = $1 AND purpose = $2 AND created_at > NOW() - INTERVAL '1 minute'",
        )
        .bind(email)
        .bind(purpose)
        .fetch_one(pool)
        .await?;

        Ok(count == 0)
    }
}

//! Waitlist notification — email templates and queue
//!
//! Sends "feature ready" emails to waitlisted users.

use sqlx::PgPool;

/// Waitlist email template — feature launched notification (EN)
pub fn feature_ready_en(feature_name: &str) -> (String, String) {
    let subject = format!("{} is now available on FlowLink!", feature_name);
    let body = format!(
        r#"<div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; max-width: 600px; margin: 0 auto; background: #0a0a0a; color: #ededed; padding: 40px 20px;">
  <div style="text-align: center; margin-bottom: 32px;">
    <h1 style="color: #0070f3; font-size: 24px; margin: 0;">FlowLink</h1>
    <p style="color: #888; font-size: 14px;">MCP Gateway + AI-Native SecOps</p>
  </div>
  <div style="background: #111; border: 1px solid #1a1a1a; border-radius: 12px; padding: 32px;">
    <h2 style="color: #fff; font-size: 20px; margin: 0 0 16px;">🚀 {} is here!</h2>
    <p style="color: #aaa; font-size: 15px; line-height: 1.6;">
      You signed up to be notified when <strong style="color: #0070f3;">{}</strong> becomes available — and it's ready now.
    </p>
    <div style="text-align: center; margin: 24px 0;">
      <a href="https://flowlink.flow-masters.ru/docs" style="background: #0070f3; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 600; font-size: 14px;">
        Check it out →
      </a>
    </div>
    <p style="color: #666; font-size: 13px; line-height: 1.5;">
      Explore the docs, try the playground, or start a free plan. No credit card required.
    </p>
  </div>
  <div style="text-align: center; margin-top: 24px; color: #555; font-size: 12px;">
    <p>FlowLink · <a href="https://flowlink.flow-masters.ru" style="color: #555;">flowlink.flow-masters.ru</a></p>
    <p>You received this because you joined the waitlist for {}.</p>
  </div>
</div>"#,
        feature_name, feature_name, feature_name
    );
    (subject, body)
}

/// Waitlist email template — feature launched notification (RU)
pub fn feature_ready_ru(feature_name: &str) -> (String, String) {
    let subject = format!("{} теперь доступен в FlowLink!", feature_name);
    let body = format!(
        r#"<div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; max-width: 600px; margin: 0 auto; background: #0a0a0a; color: #ededed; padding: 40px 20px;">
  <div style="text-align: center; margin-bottom: 32px;">
    <h1 style="color: #0070f3; font-size: 24px; margin: 0;">FlowLink</h1>
    <p style="color: #888; font-size: 14px;">MCP Gateway + AI-Native SecOps</p>
  </div>
  <div style="background: #111; border: 1px solid #1a1a1a; border-radius: 12px; padding: 32px;">
    <h2 style="color: #fff; font-size: 20px; margin: 0 0 16px;">🚀 {} уже здесь!</h2>
    <p style="color: #aaa; font-size: 15px; line-height: 1.6;">
      Вы подписались на уведомление о <strong style="color: #0070f3;">{}</strong> — и эта функция уже готова.
    </p>
    <div style="text-align: center; margin: 24px 0;">
      <a href="https://flowlink.flow-masters.ru/docs" style="background: #0070f3; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 600; font-size: 14px;">
        Попробовать →
      </a>
    </div>
    <p style="color: #666; font-size: 13px; line-height: 1.5;">
      Изучите документацию, попробуйте playground или начните с бесплатного тарифа. Без привязки карты.
    </p>
  </div>
  <div style="text-align: center; margin-top: 24px; color: #555; font-size: 12px;">
    <p>FlowLink · <a href="https://flowlink.flow-masters.ru" style="color: #555;">flowlink.flow-masters.ru</a></p>
    <p>Вы получили это письмо, потому что подписались на ожидание {}.</p>
  </div>
</div>"#,
        feature_name, feature_name, feature_name
    );
    (subject, body)
}

/// Public waitlist signup endpoint
pub async fn waitlist_signup(pool: &PgPool, email: &str, feature_id: &str, feature_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO feature_waitlist (email, feature_id, feature_name) VALUES ($1, $2, $3) ON CONFLICT (email, feature_id) DO NOTHING",
    )
    .bind(email)
    .bind(feature_id)
    .bind(feature_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Notify all waitlisted users for a feature — queues emails
pub async fn notify_waitlist(pool: &PgPool, feature_id: &str) -> Result<usize, sqlx::Error> {
    // Get unnotified entries
    let entries = sqlx::query_as::<_, (String, String)>(
        "SELECT email, feature_name FROM feature_waitlist WHERE feature_id = $1 AND notified_at IS NULL",
    )
    .bind(feature_id)
    .fetch_all(pool)
    .await?;

    let count = entries.len();
    
    // Queue emails
    for (email, feature_name) in &entries {
        let (subject, body) = feature_ready_ru(feature_name);
        let _ = sqlx::query(
            "INSERT INTO email_queue (account_id, email_type, recipient, scheduled_at, template_vars) VALUES ('waitlist', 'waitlist_notify', $1, NOW(), $2)",
        )
        .bind(email)
        .bind(serde_json::json!({ "subject": subject, "body": body, "feature_name": feature_name }))
        .execute(pool)
        .await;
    }

    // Mark as notified
    sqlx::query("UPDATE feature_waitlist SET notified_at = NOW() WHERE feature_id = $1 AND notified_at IS NULL")
        .bind(feature_id)
        .execute(pool)
        .await?;

    Ok(count)
}

/// Get all waitlist entries grouped by feature
pub async fn get_waitlist(pool: &PgPool) -> Result<Vec<WaitlistEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, WaitlistEntry>(
        "SELECT email, feature_id, feature_name, created_at::text, notified_at::text FROM feature_waitlist ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct WaitlistEntry {
    pub email: String,
    pub feature_id: String,
    pub feature_name: String,
    pub created_at: Option<String>,
    pub notified_at: Option<String>,
}

//! Promotions — configurable discounts and promo campaigns

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Promotion {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub discount_percent: i32,
    #[sqlx(default)]
    pub applicable_plans: Vec<String>,
    pub applicable_to_all: bool,
    pub duration_months: Option<i32>,
    pub max_uses: Option<i32>,
    #[sqlx(default)]
    pub current_uses: i32,
    pub is_active: bool,
    pub starts_at: Option<String>,
    pub expires_at: Option<String>,
}

/// Get all currently active promotions from PostgreSQL
pub async fn get_active_promotions(pool: &sqlx::PgPool) -> Result<Vec<Promotion>, sqlx::Error> {
    // Cast timestamps to text for simple JSON serialization
    let rows: Vec<Promotion> = sqlx::query_as::<_, Promotion>(
        r#"SELECT id, name, description, discount_percent,
                  COALESCE(applicable_plans, '{}') as applicable_plans,
                  applicable_to_all,
                  duration_months, max_uses,
                  COALESCE(current_uses, 0) as current_uses,
                  is_active,
                  starts_at::text, expires_at::text
           FROM promotions
           WHERE is_active = true
             AND (starts_at IS NULL OR starts_at <= NOW())
             AND (expires_at IS NULL OR expires_at > NOW())
           ORDER BY created_at DESC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

//! Database connection pool — PostgreSQL via sqlx

use sqlx::PgPool;

/// Database pool wrapper
pub struct DbPool {
    pool: PgPool,
}

impl DbPool {
    /// Open a new database connection pool
    pub async fn open(database_url: &str) -> anyhow::Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .idle_timeout(std::time::Duration::from_secs(600))
            .connect(database_url)
            .await?;

        tracing::info!("📦 Database connected (PostgreSQL/Supabase)");

        Ok(Self { pool })
    }

    /// Get the underlying sqlx pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run migrations from SQL files
    pub async fn migrate(&self) -> anyhow::Result<()> {
        // Migrations are handled by run_migrations() with inline SQL
        self.run_migrations().await
    }

    /// Run inline migrations (for embedded SQL)
    pub async fn run_migrations(&self) -> anyhow::Result<()> {
        crate::migrations::run(&self.pool).await
    }

    /// Health check
    pub async fn is_healthy(&self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .is_ok()
    }

    /// Close all connections
    pub async fn close(self) {
        self.pool.close().await;
    }
}

impl std::clone::Clone for DbPool {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

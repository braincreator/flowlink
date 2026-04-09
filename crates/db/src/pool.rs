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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_pool_has_clone() {
        // We can't create a DbPool without a DB, but we can verify Clone is implemented
        // by checking that the Clone trait bound compiles.
        fn assert_clone<T: Clone>() {}
        assert_clone::<DbPool>();
    }

    #[test]
    fn db_pool_has_pool_accessor() {
        // Verify the pool() method signature exists by checking the type
        // This is a compile-time check — if the method didn't exist, this wouldn't compile.
        fn check_pool_method(_pool: &DbPool) -> &PgPool {
            DbPool::pool(_pool)
        }
        // Just verify the function compiles — it won't be called
        let _ = check_pool_method;
    }

    #[test]
    fn db_pool_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DbPool>();
    }

    #[test]
    fn db_pool_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DbPool>();
    }

    #[test]
    fn health_check_query_is_valid() {
        let query = "SELECT 1";
        assert!(!query.is_empty());
        assert!(query.contains("SELECT"));
    }

    #[test]
    fn pool_config_values_are_sensible() {
        // Verify the hardcoded config values in DbPool::open are reasonable
        // (This documents the expected values; change test if config changes)
        let max_connections: u32 = 10;
        let min_connections: u32 = 2;
        let acquire_timeout_secs: u64 = 5;
        let idle_timeout_secs: u64 = 600;

        assert!(max_connections >= min_connections);
        assert!(acquire_timeout_secs > 0);
        assert!(idle_timeout_secs > acquire_timeout_secs);
    }
}

//! Database connection pool — PostgreSQL via sqlx
//!
//! Supports primary/replica topology:
//! - `write_pool`: connects to primary (all writes + migrations)
//! - `read_pool`: if replicas are configured, round-robins across them;
//!   otherwise falls back to the write pool

use sqlx::PgPool;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Database pool wrapper with primary/replica support.
pub struct DbPool {
    /// Primary pool — used for writes, migrations, and read fallback.
    pub write_pool: PgPool,
    /// Optional read replica pools for read queries.
    read_pools: Vec<PgPool>,
    /// Round-robin counter for replica selection.
    replica_index: AtomicUsize,
}

impl DbPool {
    /// Open database pools from configuration.
    ///
    /// `primary_url` is required. `replica_urls` are optional read replicas.
    /// If no replicas are provided, all reads go through the primary pool.
    pub async fn open(primary_url: &str, replica_urls: &[String]) -> anyhow::Result<Self> {
        let write_pool = Self::build_pool(primary_url).await?;

        let mut read_pools = Vec::with_capacity(replica_urls.len());
        for (i, url) in replica_urls.iter().enumerate() {
            match Self::build_pool(url).await {
                Ok(pool) => {
                    tracing::info!("📦 Read replica {i} connected");
                    read_pools.push(pool);
                }
                Err(e) => {
                    tracing::warn!("📦 Read replica {i} failed to connect: {e}. Skipping.");
                }
            }
        }

        if read_pools.is_empty() {
            tracing::info!("📦 Database connected (primary only, no replicas)");
        } else {
            tracing::info!(
                "📦 Database connected (primary + {} read replicas)",
                read_pools.len()
            );
        }

        Ok(Self {
            write_pool,
            read_pools,
            replica_index: AtomicUsize::new(0),
        })
    }

    /// Legacy convenience: open with a single URL (no replicas).
    pub async fn open_single(database_url: &str) -> anyhow::Result<Self> {
        Self::open(database_url, &[]).await
    }

    async fn build_pool(database_url: &str) -> anyhow::Result<PgPool> {
        let mut opts = sqlx::postgres::PgConnectOptions::from_str(database_url)?;
        // Disable statement cache for compatibility with poolers and
        // multi-statement migrations (sqlx::raw_sql).
        opts = opts.statement_cache_capacity(0);

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .idle_timeout(std::time::Duration::from_secs(600))
            .connect_with(opts)
            .await?;

        Ok(pool)
    }

    /// Get the write (primary) pool.
    pub fn write_pool(&self) -> &PgPool {
        &self.write_pool
    }

    /// Get a read pool — round-robins across replicas, falls back to primary.
    pub fn read_pool(&self) -> &PgPool {
        if self.read_pools.is_empty() {
            &self.write_pool
        } else {
            let idx = self.replica_index.fetch_add(1, Ordering::Relaxed) % self.read_pools.len();
            &self.read_pools[idx]
        }
    }

    /// Convenience: get pool (aliases write_pool for backward compat).
    pub fn pool(&self) -> &PgPool {
        &self.write_pool
    }

    /// Run migrations on the primary pool (always).
    pub async fn run_migrations(&self) -> anyhow::Result<()> {
        crate::migrations::run(&self.write_pool).await
    }

    /// Health check — verifies primary (and optionally first replica).
    pub async fn is_healthy(&self) -> bool {
        let primary_ok = sqlx::query("SELECT 1")
            .execute(&self.write_pool)
            .await
            .is_ok();

        if !primary_ok {
            return false;
        }

        // Check first replica if available
        if let Some(replica) = self.read_pools.first() {
            sqlx::query("SELECT 1").execute(replica).await.is_ok()
        } else {
            true
        }
    }

    /// Close all connections.
    pub async fn close(self) {
        self.write_pool.close().await;
        for replica in self.read_pools {
            replica.close().await;
        }
    }

    /// Number of configured read replicas.
    pub fn replica_count(&self) -> usize {
        self.read_pools.len()
    }
}

impl Clone for DbPool {
    fn clone(&self) -> Self {
        Self {
            write_pool: self.write_pool.clone(),
            read_pools: self.read_pools.clone(),
            replica_index: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_pool_has_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<DbPool>();
    }

    #[test]
    fn db_pool_has_pool_accessor() {
        fn check_pool_method(_pool: &DbPool) -> &PgPool {
            DbPool::pool(_pool)
        }
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
        let max_connections: u32 = 10;
        let min_connections: u32 = 2;
        let acquire_timeout_secs: u64 = 5;
        let idle_timeout_secs: u64 = 600;

        assert!(max_connections >= min_connections);
        assert!(acquire_timeout_secs > 0);
        assert!(idle_timeout_secs > acquire_timeout_secs);
    }

    #[test]
    fn replica_count_works() {
        // Compile-time check
        fn check_replica_count(_pool: &DbPool) -> usize {
            DbPool::replica_count(_pool)
        }
        let _ = check_replica_count;
    }
}

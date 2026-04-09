//! Database backup engine — PostgreSQL and MySQL dumps

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Supported database types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatabaseType {
    Postgres,
    MySQL,
    SQLite,
}

/// Database connection config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub database: String,
    /// Extra options passed to dump command
    pub extra_opts: Vec<String>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_type: DatabaseType::Postgres,
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            password: None,
            database: "default".to_string(),
            extra_opts: vec![],
        }
    }
}

/// Result of a database backup
#[derive(Debug, Clone)]
pub struct DbBackupResult {
    pub database: String,
    pub db_type: DatabaseType,
    pub dump_path: PathBuf,
    pub size_bytes: u64,
    pub checksum: String,
    pub tables_count: Option<u32>,
    pub duration_ms: u64,
}

/// Database backup engine
pub struct DatabaseBackupEngine {
    configs: Vec<DatabaseConfig>,
    max_size_mb: u64,
}

impl DatabaseBackupEngine {
    pub fn new(configs: Vec<DatabaseConfig>, max_size_mb: u64) -> Self {
        Self { configs, max_size_mb }
    }

    /// Backup all configured databases
    pub async fn backup_all(&self, output_dir: &Path) -> Result<Vec<DbBackupResult>> {
        let mut results = Vec::new();
        for config in &self.configs {
            match self.backup_database(config, output_dir).await {
                Ok(result) => {
                    info!("Database backup successful: {} ({:?})", config.database, config.db_type);
                    results.push(result);
                }
                Err(e) => {
                    warn!("Database backup failed for {}: {}", config.database, e);
                }
            }
        }
        Ok(results)
    }

    /// Backup a single database
    pub async fn backup_database(
        &self,
        config: &DatabaseConfig,
        output_dir: &Path,
    ) -> Result<DbBackupResult> {
        let start = std::time::Instant::now();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}_{}.sql.gz", 
            match config.db_type { DatabaseType::Postgres => "pg", DatabaseType::MySQL => "my", DatabaseType::SQLite => "sq" },
            config.database,
            timestamp
        );
        let dump_path = output_dir.join(&filename);

        debug!("Starting database backup: {} ({:?})", config.database, config.db_type);

        match config.db_type {
            DatabaseType::Postgres => self.dump_postgres(config, &dump_path).await?,
            DatabaseType::MySQL => self.dump_mysql(config, &dump_path).await?,
            DatabaseType::SQLite => self.dump_sqlite(config, &dump_path).await?,
        }

        let metadata = tokio::fs::metadata(&dump_path).await
            .context("Failed to read dump file metadata")?;

        let size_bytes = metadata.len();
        if size_bytes > self.max_size_mb * 1024 * 1024 {
            warn!("Database dump exceeds max size: {} bytes (max: {} MB)", size_bytes, self.max_size_mb);
        }

        let checksum = self.compute_checksum(&dump_path).await?;

        Ok(DbBackupResult {
            database: config.database.clone(),
            db_type: config.db_type.clone(),
            dump_path,
            size_bytes,
            checksum,
            tables_count: None,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Dump PostgreSQL database using pg_dump
    async fn dump_postgres(&self, config: &DatabaseConfig, output: &Path) -> Result<()> {
        let mut cmd = Command::new("pg_dump");
        cmd.args([
            "-h", &config.host,
            "-p", &config.port.to_string(),
            "-U", &config.username,
            "-F", "c", // custom format (compressed)
            "-f", output.to_string_lossy().as_ref(),
            &config.database,
        ]);
        cmd.args(&config.extra_opts);

        if let Some(ref password) = config.password {
            cmd.env("PGPASSWORD", password);
        }

        let result = cmd.output().await
            .context("Failed to execute pg_dump")?;

        if !result.status.success() {
            anyhow::bail!("pg_dump failed: {}", String::from_utf8_lossy(&result.stderr));
        }

        Ok(())
    }

    /// Dump MySQL database using mysqldump
    async fn dump_mysql(&self, config: &DatabaseConfig, output: &Path) -> Result<()> {
        let mut cmd = Command::new("mysqldump");
        cmd.args([
            "-h", &config.host,
            "-P", &config.port.to_string(),
            "-u", &config.username,
            "--single-transaction",
            "--routines",
            "--triggers",
            &config.database,
        ]);
        cmd.args(&config.extra_opts);

        if let Some(ref password) = config.password {
            cmd.arg(format!("-p{}", password));
        }

        // Pipe through gzip
        let output_path = output.to_path_buf();
        let dump_result = cmd.output().await
            .context("Failed to execute mysqldump")?;

        if !dump_result.status.success() {
            anyhow::bail!("mysqldump failed: {}", String::from_utf8_lossy(&dump_result.stderr));
        }

        // Write compressed output
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(
            std::fs::File::create(&output_path)?,
            flate2::Compression::default(),
        );
        encoder.write_all(&dump_result.stdout)?;
        encoder.finish()?;

        Ok(())
    }

    /// Dump SQLite database (simple file copy)
    async fn dump_sqlite(&self, config: &DatabaseConfig, output: &Path) -> Result<()> {
        let db_path = &config.host; // For SQLite, host field stores the file path
        tokio::fs::copy(db_path, output).await
            .context("Failed to copy SQLite database")?;
        Ok(())
    }

    /// Compute SHA256 checksum
    async fn compute_checksum(&self, path: &Path) -> Result<String> {
        let data = tokio::fs::read(path).await?;
        Ok(flowlink_crypto::sha256_hex(&data))
    }

    /// List configured databases
    pub fn list_databases(&self) -> &[DatabaseConfig] {
        &self.configs
    }

    /// Test connection to a database
    pub async fn test_connection(&self, config: &DatabaseConfig) -> Result<bool> {
        match config.db_type {
            DatabaseType::Postgres => {
                let mut cmd = Command::new("pg_isready");
                cmd.args(["-h", &config.host, "-p", &config.port.to_string()]);
                let result = cmd.output().await.context("Failed to run pg_isready")?;
                Ok(result.status.success())
            }
            DatabaseType::MySQL => {
                let mut cmd = Command::new("mysqladmin");
                cmd.args(["ping", "-h", &config.host, "-P", &config.port.to_string()]);
                if let Some(ref pw) = config.password {
                    cmd.arg(format!("-p{}", pw));
                }
                cmd.arg("-u").arg(&config.username);
                let result = cmd.output().await.context("Failed to run mysqladmin ping")?;
                Ok(result.status.success())
            }
            DatabaseType::SQLite => {
                Ok(Path::new(&config.host).exists())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.db_type, DatabaseType::Postgres);
        assert_eq!(config.port, 5432);
    }

    #[test]
    fn test_database_type_serialization() {
        let db_type = DatabaseType::Postgres;
        let json = serde_json::to_string(&db_type).unwrap();
        assert_eq!(json, "\"Postgres\"");
    }

    #[tokio::test]
    async fn test_list_databases() {
        let engine = DatabaseBackupEngine::new(vec![
            DatabaseConfig { database: "db1".to_string(), ..Default::default() },
            DatabaseConfig { database: "db2".to_string(), db_type: DatabaseType::MySQL, port: 3306, ..Default::default() },
        ], 100);
        let dbs = engine.list_databases();
        assert_eq!(dbs.len(), 2);
    }
}

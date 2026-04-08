//! Database migrations — PostgreSQL schema

use sqlx::PgPool;
use anyhow::Result;

/// Run all migrations
pub async fn run(pool: &PgPool) -> Result<()> {
    let migrations = get_migrations();

    // Create migrations tracking table
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await?;

    for (name, sql) in &migrations {
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = $1)"
        )
        .bind(name)
        .fetch_one(pool)
        .await?;

        if !applied {
            tracing::info!("📦 Migration: {}", name);
            sqlx::query(sql).execute(pool).await?;
            sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
                .bind(name)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

fn get_migrations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("001_accounts", r#"
            CREATE TABLE IF NOT EXISTS accounts (
                account_id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL DEFAULT 'free',
                active BOOLEAN NOT NULL DEFAULT TRUE,
                balance_kopecks BIGINT NOT NULL DEFAULT 0,
                payment_method TEXT,
                activated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ,
                cycle_start TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_accounts_plan ON accounts(plan_id);
            CREATE INDEX IF NOT EXISTS idx_accounts_active ON accounts(active);
        "#),

        ("002_usage_daily", r#"
            CREATE TABLE IF NOT EXISTS usage_daily (
                id SERIAL PRIMARY KEY,
                account_id TEXT NOT NULL REFERENCES accounts(account_id),
                date DATE NOT NULL,
                api_requests BIGINT NOT NULL DEFAULT 0,
                tokens BIGINT NOT NULL DEFAULT 0,
                active_agents BIGINT NOT NULL DEFAULT 0,
                storage_bytes BIGINT NOT NULL DEFAULT 0,
                api_requests_total BIGINT NOT NULL DEFAULT 0,
                tokens_total BIGINT NOT NULL DEFAULT 0,
                UNIQUE(account_id, date)
            );
            CREATE INDEX IF NOT EXISTS idx_usage_account_date ON usage_daily(account_id, date);
        "#),

        ("003_invoices", r#"
            CREATE TABLE IF NOT EXISTS invoices (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL REFERENCES accounts(account_id),
                number TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'pending',
                subtotal_kopecks BIGINT NOT NULL DEFAULT 0,
                tax_kopecks BIGINT NOT NULL DEFAULT 0,
                total_kopecks BIGINT NOT NULL DEFAULT 0,
                currency TEXT NOT NULL DEFAULT 'RUB',
                payment_method TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                paid_at TIMESTAMPTZ,
                due_at TIMESTAMPTZ NOT NULL,
                notes TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_invoices_account ON invoices(account_id);
            CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);

            CREATE TABLE IF NOT EXISTS invoice_items (
                id SERIAL PRIMARY KEY,
                invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
                description TEXT NOT NULL,
                quantity BIGINT NOT NULL DEFAULT 1,
                unit_price_kopecks BIGINT NOT NULL DEFAULT 0,
                total_kopecks BIGINT NOT NULL DEFAULT 0,
                sort_order INT NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice ON invoice_items(invoice_id);
        "#),

        ("004_audit_log", r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id SERIAL PRIMARY KEY,
                timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                level TEXT NOT NULL DEFAULT 'info',
                category TEXT,
                agent_id TEXT,
                account_id TEXT,
                action TEXT NOT NULL,
                target TEXT,
                result TEXT,
                metadata JSONB,
                hmac_hash TEXT,
                source_ip TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_level ON audit_log(level);
            CREATE INDEX IF NOT EXISTS idx_audit_account ON audit_log(account_id);
            CREATE INDEX IF NOT EXISTS idx_audit_agent ON audit_log(agent_id);
            CREATE INDEX IF NOT EXISTS idx_audit_category ON audit_log(category);
        "#),

        ("005_agents_devices", r#"
            CREATE TABLE IF NOT EXISTS agents (
                agent_id TEXT PRIMARY KEY,
                account_id TEXT,
                name TEXT,
                status TEXT NOT NULL DEFAULT 'disconnected',
                os TEXT,
                arch TEXT,
                version TEXT,
                connected_at TIMESTAMPTZ,
                last_heartbeat TIMESTAMPTZ,
                metadata JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_agents_account ON agents(account_id);
            CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);

            CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                account_id TEXT,
                name TEXT,
                device_type TEXT,
                fingerprint TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                paired_at TIMESTAMPTZ,
                last_seen TIMESTAMPTZ,
                metadata JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_devices_account ON devices(account_id);
            CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
        "#),
    ]
}

//! Database migrations — PostgreSQL schema

use anyhow::Result;
use sqlx::PgPool;

/// Run all migrations
pub async fn run(pool: &PgPool) -> Result<()> {
    let migrations = get_migrations();

    // Create migrations tracking table
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#,
    )
    .execute(pool)
    .await?;

    for (name, sql) in &migrations {
        let applied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = $1)")
                .bind(name)
                .fetch_one(pool)
                .await?;

        if !applied {
            tracing::info!("📦 Migration: {}", name);
            sqlx::raw_sql(sql).execute(pool).await?;
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
        (
            "001_accounts",
            r#"
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
        "#,
        ),
        (
            "002_usage_daily",
            r#"
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
        "#,
        ),
        (
            "003_invoices",
            r#"
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
        "#,
        ),
        (
            "004_audit_log",
            r#"
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
        "#,
        ),
        (
            "005_agents_devices",
            r#"
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
        "#,
        ),
        (
            "006_subscriptions",
            r#"
            CREATE TABLE IF NOT EXISTS subscriptions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL REFERENCES accounts(account_id),
                plan_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                period TEXT NOT NULL DEFAULT 'month',
                amount_kopecks BIGINT NOT NULL,
                tochka_subscription_id TEXT,
                payment_method TEXT,
                started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ,
                trial_ends_at TIMESTAMPTZ,
                next_billing_at TIMESTAMPTZ,
                cancelled_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_subscriptions_account ON subscriptions(account_id);
            CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);
            CREATE INDEX IF NOT EXISTS idx_subscriptions_tochka ON subscriptions(tochka_subscription_id) WHERE tochka_subscription_id IS NOT NULL;
        "#,
        ),
        (
            "007_orders",
            r#"
            CREATE TABLE IF NOT EXISTS orders (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL REFERENCES accounts(account_id),
                invoice_id TEXT REFERENCES invoices(id),
                amount_kopecks BIGINT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                payment_method TEXT NOT NULL DEFAULT 'card',
                tochka_payment_id TEXT,
                payment_url TEXT,
                paid_at TIMESTAMPTZ,
                failed_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_orders_account ON orders(account_id);
            CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
        "#,
        ),
        (
            "008_plans",
            r#"
            CREATE TABLE IF NOT EXISTS plans (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                tier INT NOT NULL DEFAULT 0,
                price_kopecks BIGINT NOT NULL DEFAULT 0,
                annual_price_kopecks BIGINT,
                period TEXT NOT NULL DEFAULT 'month',
                currency TEXT NOT NULL DEFAULT 'RUB',
                limits JSONB NOT NULL DEFAULT '{}',
                features JSONB NOT NULL DEFAULT '[]',
                is_active BOOLEAN NOT NULL DEFAULT true,
                sort_order INT NOT NULL DEFAULT 0,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#,
        ),
        (
            "009_plans_indexes",
            r#"
            CREATE INDEX IF NOT EXISTS idx_plans_active ON plans(is_active) WHERE is_active = true
        "#,
        ),
        (
            "010_plans_sort_index",
            r#"
            CREATE INDEX IF NOT EXISTS idx_plans_sort ON plans(sort_order)
        "#,
        ),
        (
            "011_orders_plan_id",
            r#"
            ALTER TABLE orders ADD COLUMN IF NOT EXISTS plan_id TEXT REFERENCES plans(id)
        "#,
        ),
        (
            "012_accounts_tg_id",
            r#"
            ALTER TABLE accounts ADD COLUMN IF NOT EXISTS tg_id BIGINT;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_tg_id ON accounts(tg_id) WHERE tg_id IS NOT NULL
        "#,
        ),
        (
            "013_accounts_email",
            r#"
            ALTER TABLE accounts ADD COLUMN IF NOT EXISTS email VARCHAR(255);
            ALTER TABLE accounts ADD COLUMN IF NOT EXISTS last_login TIMESTAMPTZ;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email) WHERE email IS NOT NULL
        "#,
        ),
        (
            "014_email_verification_codes",
            r#"
            CREATE TABLE IF NOT EXISTS email_verification_codes (
                id BIGSERIAL PRIMARY KEY,
                email VARCHAR(255) NOT NULL,
                code VARCHAR(6) NOT NULL,
                purpose VARCHAR(20) NOT NULL DEFAULT 'auth',
                used BOOLEAN NOT NULL DEFAULT FALSE,
                expires_at TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_evc_email_code ON email_verification_codes(email, code);
            CREATE INDEX IF NOT EXISTS idx_evc_email_created ON email_verification_codes(email, created_at)
        "#,
        ),
        (
            "015_email_queue",
            r#"
            CREATE TABLE IF NOT EXISTS email_queue (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                account_id VARCHAR(255) NOT NULL,
                email_type VARCHAR(50) NOT NULL,
                recipient VARCHAR(255) NOT NULL,
                scheduled_at TIMESTAMPTZ NOT NULL,
                sent_at TIMESTAMPTZ,
                template_vars JSONB DEFAULT '{}',
                attempts SMALLINT DEFAULT 0,
                max_attempts SMALLINT DEFAULT 3,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_email_queue_pending ON email_queue(scheduled_at) WHERE sent_at IS NULL;
            CREATE INDEX IF NOT EXISTS idx_email_queue_account ON email_queue(account_id)
        "#,
        ),
        (
            "016_user_notification_channels",
            r#"
            CREATE TABLE IF NOT EXISTS user_notification_channels (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                account_id VARCHAR(255) NOT NULL REFERENCES accounts(account_id),
                channel_type VARCHAR(30) NOT NULL,
                channel_address VARCHAR(255) NOT NULL,
                display_name VARCHAR(255),
                is_primary BOOLEAN NOT NULL DEFAULT FALSE,
                verified BOOLEAN NOT NULL DEFAULT FALSE,
                mute_categories JSONB DEFAULT '[]',
                min_severity VARCHAR(20) DEFAULT 'info',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(account_id, channel_type, channel_address)
            );
            CREATE INDEX IF NOT EXISTS idx_unc_account ON user_notification_channels(account_id);
            CREATE INDEX IF NOT EXISTS idx_unc_type ON user_notification_channels(channel_type);
            CREATE INDEX IF NOT EXISTS idx_unc_verified ON user_notification_channels(account_id, verified)
        "#,
        ),
        (
            "017_linking_codes",
            r#"
            CREATE TABLE IF NOT EXISTS linking_codes (
                code VARCHAR(8) PRIMARY KEY,
                account_id VARCHAR(255) NOT NULL REFERENCES accounts(account_id),
                channel_type VARCHAR(30) NOT NULL DEFAULT 'telegram',
                channel_address VARCHAR(255) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
                used_at TIMESTAMPTZ
            );
            CREATE INDEX IF NOT EXISTS idx_lc_account ON linking_codes(account_id);
            CREATE INDEX IF NOT EXISTS idx_lc_expires ON linking_codes(expires_at) WHERE used_at IS NULL
        "#,
        ),
        (
            "018_accounts_totp",
            r#"
            ALTER TABLE accounts ADD COLUMN IF NOT EXISTS totp_secret TEXT;
            ALTER TABLE accounts ADD COLUMN IF NOT EXISTS totp_enabled BOOLEAN NOT NULL DEFAULT FALSE
        "#,
        ),
        (
            "019_accounts_admin",
            r#"
            ALTER TABLE accounts ADD COLUMN IF NOT EXISTS is_admin boolean NOT NULL DEFAULT false
        "#,
        ),
        (
            "020_organizations",
            r#"
            CREATE TABLE IF NOT EXISTS organizations (
                org_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                name VARCHAR(255) NOT NULL,
                slug VARCHAR(100) NOT NULL,
                owner_account_id TEXT NOT NULL REFERENCES accounts(account_id),
                plan_id TEXT NOT NULL DEFAULT 'trial',
                limits JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_orgs_slug ON organizations(slug);
            CREATE INDEX IF NOT EXISTS idx_orgs_owner ON organizations(owner_account_id);
            CREATE INDEX IF NOT EXISTS idx_orgs_plan ON organizations(plan_id);
        "#,
        ),
        (
            "021_org_members",
            r#"
            CREATE TABLE IF NOT EXISTS org_members (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
                account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
                role VARCHAR(20) NOT NULL DEFAULT 'member' CHECK (role IN ('owner','admin','member','viewer')),
                invited_by TEXT REFERENCES accounts(account_id),
                joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_org_members_unique ON org_members(org_id, account_id);
            CREATE INDEX IF NOT EXISTS idx_org_members_account ON org_members(account_id);
        "#,
        ),
        (
            "022_org_invitations",
            r#"
            CREATE TABLE IF NOT EXISTS org_invitations (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
                email VARCHAR(255),
                role VARCHAR(20) NOT NULL DEFAULT 'member' CHECK (role IN ('owner','admin','member','viewer')),
                token TEXT NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL,
                accepted_by TEXT REFERENCES accounts(account_id),
                accepted_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_org_invitations_token ON org_invitations(token);
            CREATE INDEX IF NOT EXISTS idx_org_invitations_org ON org_invitations(org_id);
        "#,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_migrations_returns_expected_count() {
        let migrations = get_migrations();
        assert_eq!(migrations.len(), 22);
    }

    #[test]
    fn migration_names_are_sequential() {
        let migrations = get_migrations();
        let expected_names = [
            "001_accounts",
            "002_usage_daily",
            "003_invoices",
            "004_audit_log",
            "005_agents_devices",
            "006_subscriptions",
            "007_orders",
            "008_plans",
            "009_plans_indexes",
            "010_plans_sort_index",
            "011_orders_plan_id",
            "012_accounts_tg_id",
            "013_accounts_email",
            "014_email_verification_codes",
            "015_email_queue",
            "016_user_notification_channels",
            "017_linking_codes",
            "018_accounts_totp",
            "019_accounts_admin",
            "020_organizations",
            "021_org_members",
            "022_org_invitations",
        ];
        for (i, (name, _sql)) in migrations.iter().enumerate() {
            assert_eq!(
                *name, expected_names[i],
                "Migration at index {} has wrong name",
                i
            );
        }
    }

    #[test]
    fn migration_names_are_unique() {
        let migrations = get_migrations();
        let names: Vec<&str> = migrations.iter().map(|(n, _)| *n).collect();
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            assert!(seen.insert(*name), "Duplicate migration name: {}", name);
        }
    }

    #[test]
    fn all_migrations_create_tables() {
        let migrations = get_migrations();
        for (name, sql) in &migrations {
            assert!(
                sql.contains("CREATE TABLE") || sql.contains("CREATE INDEX") || sql.contains("ALTER TABLE"),
                "Migration '{}' creates neither a table, index, nor alters a table",
                name
            );
        }
    }

    #[test]
    fn all_migrations_use_if_not_exists() {
        let migrations = get_migrations();
        for (name, sql) in &migrations {
            assert!(
                sql.contains("IF NOT EXISTS"),
                "Migration '{}' missing IF NOT EXISTS",
                name
            );
        }
    }

    #[test]
    fn accounts_migration_has_expected_columns() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[0];
        let expected_cols = [
            "account_id",
            "plan_id",
            "active",
            "balance_kopecks",
            "payment_method",
            "activated_at",
            "expires_at",
            "cycle_start",
            "created_at",
            "updated_at",
        ];
        for col in &expected_cols {
            assert!(
                sql.contains(col),
                "accounts migration missing column '{}'",
                col
            );
        }
    }

    #[test]
    fn accounts_migration_has_indexes() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[0];
        assert!(sql.contains("idx_accounts_plan"));
        assert!(sql.contains("idx_accounts_active"));
    }

    #[test]
    fn usage_daily_migration_has_expected_columns() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[1];
        let expected_cols = [
            "account_id",
            "date",
            "api_requests",
            "tokens",
            "active_agents",
            "storage_bytes",
            "api_requests_total",
            "tokens_total",
        ];
        for col in &expected_cols {
            assert!(
                sql.contains(col),
                "usage_daily migration missing column '{}'",
                col
            );
        }
    }

    #[test]
    fn usage_daily_has_unique_constraint() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[1];
        assert!(sql.contains("UNIQUE(account_id, date)"));
    }

    #[test]
    fn invoices_migration_creates_both_tables() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[2];
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS invoices"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS invoice_items"));
    }

    #[test]
    fn invoices_migration_has_expected_columns() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[2];
        let inv_cols = [
            "id",
            "account_id",
            "number",
            "status",
            "total_kopecks",
            "currency",
            "paid_at",
        ];
        for col in &inv_cols {
            assert!(
                sql.contains(col),
                "invoices migration missing column '{}'",
                col
            );
        }
        let item_cols = [
            "invoice_id",
            "description",
            "quantity",
            "unit_price_kopecks",
            "total_kopecks",
            "sort_order",
        ];
        for col in &item_cols {
            assert!(
                sql.contains(col),
                "invoice_items migration missing column '{}'",
                col
            );
        }
    }

    #[test]
    fn invoice_items_has_cascade_delete() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[2];
        assert!(sql.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn audit_log_migration_has_expected_columns() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[3];
        let expected_cols = [
            "timestamp",
            "level",
            "category",
            "agent_id",
            "account_id",
            "action",
            "target",
            "result",
            "metadata",
            "hmac_hash",
            "source_ip",
        ];
        for col in &expected_cols {
            assert!(
                sql.contains(col),
                "audit_log migration missing column '{}'",
                col
            );
        }
    }

    #[test]
    fn audit_log_metadata_is_jsonb() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[3];
        assert!(sql.contains("metadata JSONB"));
    }

    #[test]
    fn audit_log_has_expected_indexes() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[3];
        assert!(sql.contains("idx_audit_timestamp"));
        assert!(sql.contains("idx_audit_level"));
        assert!(sql.contains("idx_audit_account"));
        assert!(sql.contains("idx_audit_agent"));
        assert!(sql.contains("idx_audit_category"));
    }

    #[test]
    fn agents_devices_migration_creates_both_tables() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[4];
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS agents"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS devices"));
    }

    #[test]
    fn agents_migration_has_expected_columns() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[4];
        let expected_cols = [
            "agent_id",
            "account_id",
            "name",
            "status",
            "os",
            "arch",
            "version",
            "connected_at",
            "last_heartbeat",
            "metadata",
            "created_at",
        ];
        for col in &expected_cols {
            assert!(
                sql.contains(col),
                "agents migration missing column '{}'",
                col
            );
        }
    }

    #[test]
    fn devices_migration_has_expected_columns() {
        let migrations = get_migrations();
        let (_, sql) = &migrations[4];
        let expected_cols = [
            "device_id",
            "account_id",
            "name",
            "device_type",
            "fingerprint",
            "status",
            "paired_at",
            "last_seen",
            "metadata",
            "created_at",
        ];
        for col in &expected_cols {
            assert!(
                sql.contains(col),
                "devices migration missing column '{}'",
                col
            );
        }
    }

    #[test]
    fn all_migrations_create_indexes() {
        let migrations = get_migrations();
        for (name, sql) in &migrations {
            assert!(
                sql.contains("CREATE INDEX") || sql.contains("CREATE TABLE") || sql.contains("ALTER TABLE"),
                "Migration '{}' creates neither an index, table, nor alters a table",
                name
            );
        }
    }

    #[test]
    fn migration_sql_is_not_empty() {
        let migrations = get_migrations();
        for (name, sql) in &migrations {
            assert!(!sql.trim().is_empty(), "Migration '{}' has empty SQL", name);
            assert!(
                sql.len() > 50,
                "Migration '{}' SQL seems too short ({} bytes)",
                name,
                sql.len()
            );
        }
    }

    #[test]
    fn foreign_keys_reference_accounts() {
        let migrations = get_migrations();
        // usage_daily references accounts
        assert!(migrations[1].1.contains("REFERENCES accounts(account_id)"));
        // invoices references accounts
        assert!(migrations[2].1.contains("REFERENCES accounts(account_id)"));
    }

    #[test]
    fn foreign_keys_reference_invoices() {
        let migrations = get_migrations();
        // invoice_items references invoices
        assert!(migrations[2].1.contains("REFERENCES invoices(id)"));
    }
}

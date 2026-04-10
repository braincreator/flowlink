#!/usr/bin/env bash
set -euo pipefail

# FlowLink Database Migration Script
# Purpose: Normalize all plans to use identical features and update pricing
# Prerequisites: 
# 1. Rust billing crate is updated with new Plan definitions
# 2. Migration SQL is ready
# 3. Database backup exists

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DB_POOL_DIR="$SCRIPT_DIR/../db"
MIGRATION_SQL="$SCRIPT_DIR/pricing-migration.sql"

echo "🗄️ FlowLink Database Migration"
echo "=================================="

# Check if migration file exists
if [ ! -f "$MIGRATION_SQL" ]; then
    echo "❌ Migration file not found: $MIGRATION_SQL"
    exit 1
fi

# Load environment
if [ -f "$SCRIPT_DIR/.env" ]; then
    echo "📥 Loading environment..."
    source "$SCRIPT_DIR/.env"
fi

# Database connection (should be set in .env or environment)
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-postgres}"
DB_NAME="${DB_NAME:-flowlink}"

# Create backup before migration
BACKUP_DIR="/tmp/flowlink-backup-$(date +%Y%m%d_%H%M%S)"
echo "💾 Creating backup at: $BACKUP_DIR"
mkdir -p "$BACKUP_DIR"
pg_dump -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" > "$BACKUP_DIR/flowlink-backup.sql"
echo "✅ Backup completed: $BACKUP_DIR/flowlink-backup.sql"

# Check if migration script can be applied
echo "🔍 Checking current plan data..."
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "
SELECT id, name, features, price_kopecks, annual_price_kopecks FROM plans ORDER BY id;
" | head -10

# Ask for confirmation
read -p "👉 Apply migration? This will update all plan features and pricing (y/N): " confirm
if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "❌ Migration cancelled"
    exit 0
fi

# Apply migration
echo "⚡ Applying migration..."
if psql -h "$DB_HOST" -p "$DB_PORT" -U "$USER" -d "$DB_NAME" -f "$MIGRATION_SQL"; then
    echo "✅ Migration applied successfully"
else
    echo "❌ Migration failed"
    exit 1
fi

# Verify results
echo "📊 Verification..."
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "
SELECT 
    id,
    name,
    price_kopecks,
    annual_price_kopecks,
    features,
    CASE 
        WHEN price_kopecks = 0 THEN 'FREE'
        WHEN price_kopecks = 199000 THEN 'Starter (1990₽)'
        WHEN price_kopecks = 599000 THEN 'Pro (5990₽)'
        ELSE 'Unknown'
    END as plan_type
FROM plans 
ORDER BY sort_order;
"

echo "🎉 Migration completed!"
echo "🔄 Restart billing service to load updated plans"
echo "📋 Next: Test checkout flow with new pricing"
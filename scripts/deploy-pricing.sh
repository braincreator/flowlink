#!/usr/bin/env bash
set -euo pipefail

# Task 16: FlowLink Pricing Migration → Production
# Deploys updated pricing with new structure (resource gating)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VPS_HOST="93.93.207.44"
VPS_USER="root"
VPS_PATH="/var/www/flowlink"
DB_HOST="localhost"
DB_PORT="5432"
DB_USER="postgres"
DB_NAME="flowlink"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Task 16: Pricing Migration → Production ===${NC}"

# Pre-check: VPS connectivity
echo -e "${YELLOW}📡 Testing VPS connection...${NC}"
export SSHPASS='o#EkBHi*wZru8+'
SSH_OPTS="-o StrictHostKeyChecking=no -o PubkeyAuthentication=no"

if ! sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "echo '✅ VPS accessible'"; then
    echo -e "${RED}❌ Cannot connect to VPS${NC}"
    exit 1
fi

# 1. Build everything
echo -e "${YELLOW}[1/6] Building FlowLink workspace...${NC}"
cd "$PROJECT_DIR"

# Build Rust components
echo -e "${YELLOW}  Building relay binary...${NC}"
cargo build --release --bin flowlink

echo -e "${YELLOW}  Building billing crate...${NC}"
cargo build --release --package flowlink-billing

# Build website
echo -e "${YELLOW}  Building website...${NC}"
cd "$PROJECT_DIR/website"
npm ci --silent
npm run build

echo -e "${GREEN}  ✓ Build complete${NC}"

# 2. Upload migration scripts
echo -e "${YELLOW}[2/6] Uploading migration scripts...${NC}"
sshpass -e scp $SSH_OPTS \
    "$PROJECT_DIR/scripts/db/pricing-migration.sql" \
    "$PROJECT_DIR/scripts/db/migrate-pricing.sh" \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/scripts/

# Make migration script executable
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} \
    "chmod +x ${VPS_PATH}/scripts/migrate-pricing.sh"

echo -e "${GREEN}  ✓ Migration scripts uploaded${NC}"

# 3. Upload deployment
echo -e "${YELLOW}[3/6] Uploading binaries to VPS...${NC}"
sshpass -e scp $SSH_OPTS \
    "$PROJECT_DIR/target/release/flowlink" \
    "$PROJECT_DIR/Dockerfile" \
    "$PROJECT_DIR/Dockerfile.agent" \
    "$PROJECT_DIR/docker-compose.yml" \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/

sshpass -e scp -r $SSH_OPTS \
    "$PROJECT_DIR/website/out/"* \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/website/

echo -e "${GREEN}  ✓ Files uploaded${NC}"

# 4. Check current database state
echo -e "${YELLOW}[4/6] Checking current database state...${NC}"
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
    cd ${VPS_PATH}
    echo \"Current plans:\"
    psql -h localhost -U postgres -d flowlink -c \"SELECT id, name, price_kopecks, features FROM plans ORDER BY id;\"
"

echo -e "${YELLOW}[5/6] Running database migration...${NC}"
echo "This will:"
echo "- Update all plans to have identical features"
echo "- Update prices: Trial=0, Starter=1990₽, Pro=5990₽"
echo "- Create backup"
echo ""
read -p "👉 Continue with database migration? (y/N): " confirm
if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo -e "${RED}❌ Migration cancelled${NC}"
    exit 0
fi

# Execute migration on VPS
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
    cd ${VPS_PATH}
    echo \"📁 Creating backup...\"
    mkdir -p /tmp/flowlink-backup-\$(date +%Y%m%d_%H%M%S)
    pg_dump -h localhost -U postgres -d flowlink > /tmp/flowlink-backup-\$(date +%Y%m%d_%H%M%S)/backup.sql
    
    echo \"🗄️ Running migration...\"
    ./scripts/migrate-pricing.sh
    
    echo \"📊 Migration results:\"
    psql -h localhost -U postgres -d flowlink -c \"
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
        ORDER BY sort_order;\"
"

echo -e "${GREEN}  ✓ Database migration complete${NC}"

# 6. Restart services
echo -e "${YELLOW}[6/6] Restarting services...${NC}"
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
    cd ${VPS_PATH}
    
    # Stop existing services
    echo \"🛑 Stopping existing services...\"
    pkill -f 'flowlink' 2>/dev/null || true
    sleep 2
    
    # Start relay with new billing
    echo \"🚀 Starting new relay...\"
    nohup ./bin/flowlink relay --config config/relay.json > /var/log/flowlink-relay.log 2>&1 &
    RELAY_PID=\$!
    echo \"Relay PID: \$RELAY_PID\"
    
    # Wait for service to start
    echo \"⏳ Waiting for relay to start...\"
    sleep 3
    
    # Check health
    if curl -s http://localhost:3000/api/health | grep -q 'ok'; then
        echo \"✅ Relay healthy\"
    else
        echo \"❌ Relay health check failed\"
    fi
"

# 7. Verify deployment
echo -e "${YELLOW}🔍 Verifying deployment...${NC}"

# Check API
echo -e "${YELLOW}  Testing /api/plans endpoint...${NC}"
PLANS_RESPONSE=$(curl -s "https://flowlink.flow-masters.ru/api/plans" | jq . || echo "API error")
if echo "$PLANS_RESPONSE" | grep -q '"price_kopecks"'; then
    echo -e "${GREEN}  ✓ /api/plans working${NC}"
    echo "Sample plan data:"
    echo "$PLANS_RESPONSE" | jq '.[] | {id, name, price_kopecks, features: .features[:3]}'
else
    echo -e "${RED}  ❌ /api/plans failed${NC}"
fi

# Check website
echo -e "${YELLOW}  Testing website pricing page...${NC}"
if curl -s "https://flowlink.flow-masters.ru/pricing" | grep -q 'Стarter.*1 990₽'; then
    echo -e "${GREEN}  ✓ Website pricing updated${NC}"
else
    echo -e "${RED}  ❌ Website pricing not updated${NC}"
fi

echo -e "${BLUE}=== Task 16 Complete ===${NC}"
echo -e "${GREEN}🎉 New pricing deployed!${NC}"
echo ""
echo "📊 Summary:"
echo "- Trial: FREE (1 host, 7 days)"
echo "- Starter: 1,990₽/month (5 hosts, 5 users)"  
echo "- Pro: 5,990₽/month (50 hosts, 25 users)"
echo "- All features available on every plan!"
echo ""
echo "🔗 Next steps:"
echo "1. Test checkout flow with new pricing"
echo "2. Update documentation (if needed)"
echo "3. Monitor billing metrics"
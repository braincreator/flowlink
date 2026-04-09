#!/usr/bin/env bash
set -euo pipefail

# FlowLink VPS Deploy Script
# Deploys relay + agent + website to 93.93.207.44

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VPS_HOST="93.93.207.44"
VPS_USER="root"
VPS_PATH="/var/www/flowlink"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== FlowLink Deploy ===${NC}"

# 1. Build release binary
echo -e "${YELLOW}[1/5] Building release binary...${NC}"
cd "$PROJECT_DIR"
cargo build --release --bin flowlink
echo -e "${GREEN}  ✓ Build complete${NC}"

# 2. Build website
echo -e "${YELLOW}[2/5] Building website...${NC}"
cd "$PROJECT_DIR/website"
npm ci --silent
npm run build
echo -e "${GREEN}  ✓ Website build complete${NC}"

# 3. Upload to VPS
echo -e "${YELLOW}[3/5] Uploading to VPS...${NC}"
export SSHPASS='o#EkBHi*wZru8+'
SSH_OPTS="-o StrictHostKeyChecking=no -o PubkeyAuthentication=no"

sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "mkdir -p ${VPS_PATH}/{bin,config,website}"

sshpass -e scp $SSH_OPTS \
    "$PROJECT_DIR/target/release/flowlink" \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/bin/flowlink

sshpass -e scp $SSH_OPTS \
    "$PROJECT_DIR/Dockerfile" \
    "$PROJECT_DIR/Dockerfile.agent" \
    "$PROJECT_DIR/docker-compose.yml" \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/

sshpass -e scp -r $SSH_OPTS \
    "$PROJECT_DIR/website/out/"* \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/website/

echo -e "${GREEN}  ✓ Upload complete${NC}"

# 4. Configure nginx
echo -e "${YELLOW}[4/5] Configuring nginx...${NC}"
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "nginx -t && systemctl reload nginx"
echo -e "${GREEN}  ✓ Nginx configured${NC}"

# 5. Restart services
echo -e "${YELLOW}[5/5] Restarting services...${NC}"
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
    cd ${VPS_PATH}
    chmod +x bin/flowlink
    # Stop existing if running
    pkill -f 'flowlink relay' 2>/dev/null || true
    sleep 1
    # Start relay in background
    nohup bin/flowlink relay --config config/relay.json > /var/log/flowlink-relay.log 2>&1 &
    echo \"Relay PID: \$!\"
"
echo -e "${GREEN}  ✓ Services restarted${NC}"

echo -e "${GREEN}=== Deploy Complete ===${NC}"
echo -e "Website: https://flowlink.flow-masters.ru"
echo -e "Health:  https://flowlink.flow-masters.ru/api/health"

#!/usr/bin/env bash
set -euo pipefail

# FlowLink Website Deploy (Server Mode)
# Deploys Next.js website with API routes to flowlink.flow-masters.ru
# Requires: pm2, Node.js 18+ on VPS

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WEBSITE_DIR="$PROJECT_DIR/website"
VPS_HOST="93.93.207.44"
VPS_USER="root"
VPS_PATH="/var/www/flowlink/website"
NGINX_CONF="$SCRIPT_DIR/nginx-flowlink.conf"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== FlowLink Website Deploy (Server Mode) ===${NC}"

# 1. Build website
echo -e "${YELLOW}[1/5] Building website...${NC}"
cd "$WEBSITE_DIR"
npm ci --silent 2>/dev/null || npm install --silent
npm run build
echo -e "${GREEN}  ✓ Build complete${NC}"

# 2. Upload to VPS
echo -e "${YELLOW}[2/5] Uploading to VPS...${NC}"
export SSHPASS='o#EkBHi*wZru8+'
SSH_OPTS="-o StrictHostKeyChecking=no -o PubkeyAuthentication=no"

sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "mkdir -p ${VPS_PATH}"

# Upload build output, config, and deps
sshpass -e scp -r $SSH_OPTS \
    "$WEBSITE_DIR/.next/" \
    "$WEBSITE_DIR/package.json" \
    "$WEBSITE_DIR/package-lock.json" \
    "$WEBSITE_DIR/public/" \
    "$WEBSITE_DIR/node_modules/" \
    "$WEBSITE_DIR/next.config.ts" \
    "$WEBSITE_DIR/tsconfig.json" \
    "$WEBSITE_DIR/.env.production" \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/

echo -e "${GREEN}  ✓ Upload complete${NC}"

# 3. Install deps on VPS and restart
echo -e "${YELLOW}[3/5] Restarting website on VPS...${NC}"
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
    cd ${VPS_PATH}
    npm install --production 2>/dev/null

    # Set environment
    export RELAY_URL=http://127.0.0.1:8080
    export NODE_ENV=production
    export PORT=3000

    # Restart with pm2
    pm2 delete flowlink-website 2>/dev/null || true
    pm2 start npm --name flowlink-website -- start
    pm2 save
"
echo -e "${GREEN}  ✓ Website restarted on :3000${NC}"

# 4. Update nginx config
echo -e "${YELLOW}[4/5] Updating nginx config...${NC}"
if [ -f "$NGINX_CONF" ]; then
    sshpass -e scp $SSH_OPTS "$NGINX_CONF" \
        ${VPS_USER}@${VPS_HOST}:/etc/nginx/sites-available/flowlink.conf
    sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
        ln -sf /etc/nginx/sites-available/flowlink.conf /etc/nginx/sites-enabled/flowlink.conf
        nginx -t 2>&1 && nginx -s reload
    "
    echo -e "${GREEN}  ✓ Nginx reloaded${NC}"
else
    echo -e "${YELLOW}  ⚠ nginx config not found, skipping${NC}"
fi

# 5. Verify
echo -e "${YELLOW}[5/5] Verifying...${NC}"
sleep 3

# Check website
HTTP_CODE=$(curl -s -o /dev/null -w '%{http_code}' https://flowlink.flow-masters.ru 2>/dev/null || echo "000")
if [ "$HTTP_CODE" = "200" ]; then
    echo -e "${GREEN}  ✓ Website live (HTTP $HTTP_CODE)${NC}"
else
    echo -e "${RED}  ✗ Website error (HTTP $HTTP_CODE)${NC}"
fi

# Check API
API_CODE=$(curl -s -o /dev/null -w '%{http_code}' https://flowlink.flow-masters.ru/api/plans 2>/dev/null || echo "000")
if [ "$API_CODE" = "200" ]; then
    echo -e "${GREEN}  ✓ API proxy working (HTTP $API_CODE)${NC}"
else
    echo -e "${YELLOW}  ⚠ API proxy (HTTP $API_CODE) — relay may not be running${NC}"
fi

echo -e "${GREEN}=== Deploy Complete ===${NC}"
echo -e "URL: https://flowlink.flow-masters.ru"
echo -e "API: https://flowlink.flow-masters.ru/api/plans"

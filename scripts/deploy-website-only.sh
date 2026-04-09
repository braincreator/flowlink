#!/usr/bin/env bash
set -euo pipefail

# FlowLink Website-Only Deploy
# Deploys Next.js website to flowlink.flow-masters.ru

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WEBSITE_DIR="$PROJECT_DIR/website"
VPS_HOST="93.93.207.44"
VPS_USER="root"
VPS_PATH="/var/www/flowlink"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== FlowLink Website Deploy ===${NC}"

# 1. Build website
echo -e "${YELLOW}[1/4] Building website...${NC}"
cd "$WEBSITE_DIR"
npm ci --silent 2>/dev/null || npm install --silent
npm run build
echo -e "${GREEN}  ✓ Build complete${NC}"

# 2. Upload to VPS
echo -e "${YELLOW}[2/4] Uploading to VPS...${NC}"
export SSHPASS='o#EkBHi*wZru8+'
SSH_OPTS="-o StrictHostKeyChecking=no -o PubkeyAuthentication=no"

sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "mkdir -p ${VPS_PATH}/website"

# Upload the entire .next directory and needed files
sshpass -e scp -r $SSH_OPTS \
    "$WEBSITE_DIR/.next/" \
    "$WEBSITE_DIR/package.json" \
    "$WEBSITE_DIR/package-lock.json" \
    "$WEBSITE_DIR/public/" \
    "$WEBSITE_DIR/node_modules/" \
    ${VPS_USER}@${VPS_HOST}:${VPS_PATH}/website/

echo -e "${GREEN}  ✓ Upload complete${NC}"

# 3. Install deps on VPS and restart
echo -e "${YELLOW}[3/4] Installing deps on VPS...${NC}"
sshpass -e ssh $SSH_OPTS ${VPS_USER}@${VPS_HOST} "
    cd ${VPS_PATH}/website
    npm install --production 2>/dev/null
    pm2 delete flowlink-website 2>/dev/null || true
    pm2 start npm --name flowlink-website -- start
    pm2 save
"
echo -e "${GREEN}  ✓ Website restarted${NC}"

# 4. Verify
echo -e "${YELLOW}[4/4] Verifying...${NC}"
sleep 2
HTTP_CODE=$(curl -s -o /dev/null -w '%{http_code}' https://flowlink.flow-masters.ru 2>/dev/null || echo "000")
if [ "$HTTP_CODE" = "200" ]; then
    echo -e "${GREEN}  ✓ Website live (HTTP $HTTP_CODE)${NC}"
else
    echo -e "${YELLOW}  ⚠ Got HTTP $HTTP_CODE (may need nginx config)${NC}"
fi

echo -e "${GREEN}=== Deploy Complete ===${NC}"
echo -e "URL: https://flowlink.flow-masters.ru"

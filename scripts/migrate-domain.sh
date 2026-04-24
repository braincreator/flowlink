#!/usr/bin/env bash
# FlowLink — Domain Migration Script
# Usage: ./migrate-domain.sh <new-domain> [--dry-run]
# Example: ./migrate-domain.sh flowlink.newdomain.com --dry-run
#
# Updates ALL hardcoded domain references across both repos:
#   - Relay config (relay.json on VPS)
#   - Rust source (env var fallbacks)
#   - Next.js website (SEO, install scripts, API URLs)
#   - Nginx config
#   - PM2 ecosystem
#
# --dry-run  Show what would change without making changes
set -euo pipefail

OLD_DOMAIN="flowlink.flow-masters.ru"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FLOWLINK_DIR="${SCRIPT_DIR}"
WEBSITE_DIR="${FLOWLINK_DIR}/../flowlink-website"
VPS="root@93.93.207.44"

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <new-domain> [--dry-run]"
  echo "  Example: $0 flowlink.newdomain.com"
  echo "  Example: $0 flowlink.newdomain.com --dry-run"
  exit 1
fi

NEW_DOMAIN="$1"
DRY_RUN=false
[[ "${2:-}" == "--dry-run" ]] && DRY_RUN=true

echo "================================================"
echo "  FlowLink Domain Migration"
echo "  ${OLD_DOMAIN} → ${NEW_DOMAIN}"
echo "================================================"
echo ""

if $DRY_RUN; then
  echo "🔍 DRY RUN — no changes will be made"
  echo ""
fi

# ─── 1. Rust source: default fallback in lib.rs ───
echo "📦 [1/8] Rust relay — server_base_url() fallback"
RUST_FILE="${FLOWLINK_DIR}/crates/relay/src/lib.rs"
if $DRY_RUN; then
  grep -n "$OLD_DOMAIN" "$RUST_FILE" | head -3 || echo "  (already updated or no matches)"
else
  sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$RUST_FILE"
  echo "  ✅ Updated"
fi

# ─── 2. Rust source: email default fallback ───
echo "📦 [2/8] Rust relay — email.rs SERVER_URL fallback"
EMAIL_FILE="${FLOWLINK_DIR}/crates/relay/src/email.rs"
if $DRY_RUN; then
  grep -n "$OLD_DOMAIN" "$EMAIL_FILE" | head -3 || echo "  (already updated or no matches)"
else
  sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$EMAIL_FILE"
  echo "  ✅ Updated"
fi

# ─── 3. Rust source: RelayConfig default ───
echo "📦 [3/8] Rust relay — config.rs public_url() default"
CONFIG_FILE="${FLOWLINK_DIR}/crates/core/src/config.rs"
if $DRY_RUN; then
  grep -n "$OLD_DOMAIN" "$CONFIG_FILE" | head -3 || echo "  (already updated or no matches)"
else
  sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$CONFIG_FILE"
  echo "  ✅ Updated"
fi

# ─── 4. Rust source: SAML test data ───
echo "📦 [4/8] Rust relay — saml.rs test data"
SAML_FILE="${FLOWLINK_DIR}/crates/relay/src/saml.rs"
if $DRY_RUN; then
  grep -c "$OLD_DOMAIN" "$SAML_FILE" | xargs -I{} echo "  {} occurrences"
else
  sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$SAML_FILE"
  echo "  ✅ Updated"
fi

# ─── 5. Next.js website: constants.ts ───
echo "🌐 [5/8] Website — lib/constants.ts"
CONST_FILE="${WEBSITE_DIR}/lib/constants.ts"
if $DRY_RUN; then
  grep -n "$OLD_DOMAIN" "$CONST_FILE" || echo "  (already updated)"
else
  sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$CONST_FILE"
  echo "  ✅ Updated"
fi

# ─── 6. Next.js website: all remaining hardcoded URLs ───
# These are in docs (code examples) — update them too
echo "🌐 [6/8] Website — doc code examples & remaining refs"
DOC_COUNT=0
if $DRY_RUN; then
  DOC_COUNT=$(grep -rn "$OLD_DOMAIN" "${WEBSITE_DIR}/app/" --include="*.tsx" --include="*.ts" 2>/dev/null | grep -v node_modules | grep -v ".next" | wc -l | tr -d ' ')
  echo "  ${DOC_COUNT} remaining occurrences in docs/examples"
else
  # Update all .tsx/.ts files in app/
  find "${WEBSITE_DIR}/app" -name "*.tsx" -o -name "*.ts" | while read -r f; do
    if grep -q "$OLD_DOMAIN" "$f" 2>/dev/null; then
      sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$f"
    fi
  done
  # Update lib/ files (i18n, etc.)
  find "${WEBSITE_DIR}/lib" -name "*.tsx" -o -name "*.ts" | while read -r f; do
    if grep -q "$OLD_DOMAIN" "$f" 2>/dev/null; then
      sed -i '' "s|https://${OLD_DOMAIN}|https://${NEW_DOMAIN}|g" "$f"
    fi
  done
  echo "  ✅ Updated all occurrences"
fi

# ─── 7. VPS: relay.json server_url ───
echo "🖥️  [7/8] VPS — relay.json server_url"
if $DRY_RUN; then
  echo "  Would set server_url = https://${NEW_DOMAIN} in relay.json"
else
  ssh "$VPS" "cd /opt/flowlink && python3 -c \"
import json
with open('relay.json', 'r') as f:
    c = json.load(f)
c['server_url'] = 'https://${NEW_DOMAIN}'
# Also update dashboard_url if it matches old domain
if c.get('dashboard_url', '').find('${OLD_DOMAIN}') >= 0:
    c['dashboard_url'] = 'https://${NEW_DOMAIN}'
with open('relay.json', 'w') as f:
    json.dump(c, f, indent=4)
print('Done')
\"" 2>&1
  echo "  ✅ Updated relay.json"
fi

# ─── 8. VPS: nginx server_name ───
echo "🖥️  [8/8] VPS — nginx config"
if $DRY_RUN; then
  echo "  Would update server_name in nginx config"
  echo "  ⚠️  SSL certificate must be obtained separately (certbot)"
else
  echo "  ⚠️  Skipping nginx auto-update (requires SSL cert setup)"
  echo "     Manual steps:"
  echo "       1. certbot --nginx -d ${NEW_DOMAIN}"
  echo "       2. Update server_name in /etc/nginx/sites-enabled/flowlink"
  echo "       3. nginx -t && systemctl reload nginx"
fi

echo ""
echo "================================================"
echo "  Post-migration checklist"
echo "================================================"
echo ""
echo "After deploying:"
echo "  1. cd ${FLOWLINK_DIR} && cargo zigbuild --release --target x86_64-unknown-linux-gnu -p flowlink"
echo "  2. scp target/x86_64-unknown-linux-gnu/release/flowlink ${VPS}:/opt/flowlink/bin/flowlink"
echo "  3. ssh ${VPS} 'systemctl restart flowlink-relay'"
echo ""
echo "  4. cd ${WEBSITE_DIR} && NEXT_PUBLIC_SERVER_URL=https://${NEW_DOMAIN} npm run build"
echo "  5. cd ${WEBSITE_DIR} && ./deploy.sh"
echo ""
echo "  6. On VPS: set SERVER_URL=https://${NEW_DOMAIN} in systemd env"
echo "     (or in /etc/systemd/system/flowlink-relay.service Environment=)"
echo ""
echo "  7. Get SSL: certbot --nginx -d ${NEW_DOMAIN}"
echo "  8. Update DNS A record to point ${NEW_DOMAIN} → 93.93.207.44"
echo "  9. Update Cloudflare (if proxied)"
echo "  10. Test: curl -I https://${NEW_DOMAIN}/health"
echo ""

if $DRY_RUN; then
  echo "🔍 DRY RUN complete — no changes were made."
  echo "   Run without --dry-run to apply."
else
  echo "✅ Domain references updated locally."
  echo "   Build & deploy both repos, then update nginx/SSL/DNS."
fi

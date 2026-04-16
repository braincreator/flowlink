#!/bin/bash
# FlowLink Release: bump patch, build, deploy, tag, GitHub release
# Usage: ./scripts/release.sh
# Bumps 0.1.0 → 0.1.1 (patch only, minor stays)
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# Get current version
CURRENT=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
MAJOR=$(echo "$CURRENT" | cut -d. -f1)
MINOR=$(echo "$CURRENT" | cut -d. -f2)
PATCH=$(echo "$CURRENT" | cut -d. -f3)

# Bump patch
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"
TAG="v${NEW_VERSION}"

echo "╔══════════════════════════════════╗"
echo "║   FlowLink Release               ║"
echo "║   $CURRENT → $NEW_VERSION          ║"
echo "╚══════════════════════════════════╝"

# Confirm
read -rp "Release $NEW_VERSION? [y/N] " confirm
if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
    echo "Cancelled"
    exit 0
fi

# 1. Bump version in Cargo.toml
echo ""
echo "📝 Bumping version to $NEW_VERSION..."
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" Cargo.toml

git add Cargo.toml
git commit -m "chore: bump version to $NEW_VERSION"
git push origin main

# 2. Build
echo ""
echo "🔨 Cross-compiling with zig..."
cargo zigbuild --release --bin flowlink --target x86_64-unknown-linux-gnu 2>&1 | grep -E "Compiling|Finished|error" | tail -5

BIN="target/x86_64-unknown-linux-gnu/release/flowlink"
if [ ! -f "$BIN" ]; then
    echo "❌ Build failed!"
    exit 1
fi
echo "   $(du -h "$BIN" | cut -f1)"

# 3. Deploy
echo ""
echo "📦 Deploying to VPS..."
VPS="root@93.93.207.44"
scp -q "$BIN" "$VPS:/opt/flowlink/bin/flowlink.new"

ssh -o ConnectTimeout=10 "$VPS" '
    kill -9 $(pgrep -f "flowlink relay") 2>/dev/null || true
    sleep 2
    mv /opt/flowlink/bin/flowlink.new /opt/flowlink/bin/flowlink 2>/dev/null
    chmod +x /opt/flowlink/bin/flowlink
    systemctl restart flowlink-relay
    sleep 3
    systemctl is-active --quiet flowlink-relay && echo "✅ Deployed!" || echo "❌ Service failed"
'

# 4. Tag + GitHub Release
echo ""
echo "🏷️  Creating release $TAG..."

git tag "$TAG"
git push origin "$TAG"

ARCHIVE="/tmp/flowlink-${NEW_VERSION}-linux-amd64.tar.gz"
cp "$BIN" "/tmp/flowlink"
tar czf "$ARCHIVE" -C /tmp flowlink
rm -f /tmp/flowlink

gh release create "$TAG" "$ARCHIVE" \
    --title "FlowLink $TAG" \
    --notes "Release $TAG — $(git rev-parse --short HEAD)" \
    --target main

rm -f "$ARCHIVE"

echo ""
echo "✅ Released!"
echo "🔗 https://github.com/braincreator/flowlink/releases/tag/$TAG"

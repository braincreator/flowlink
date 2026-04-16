#!/bin/bash
# FlowLink Release: bump patch, build (linux + macos), deploy, tag, GitHub release
# Usage: ./scripts/release.sh
# Bumps 0.1.0 → 0.1.1 (patch only)
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
VPS="root@93.93.207.44"

echo "╔══════════════════════════════════╗"
echo "║   FlowLink Release               ║"
echo "║   $CURRENT → $NEW_VERSION          ║"
echo "╚══════════════════════════════════╝"

read -rp "Release $NEW_VERSION? [y/N] " confirm
if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
    echo "Cancelled"
    exit 0
fi

# 1. Bump version
echo ""
echo "📝 Bumping version to $NEW_VERSION..."
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" Cargo.toml
git add Cargo.toml
git commit -m "chore: bump version to $NEW_VERSION"
git push origin main

# 2. Build Linux x86_64 (zig cross-compile)
echo ""
echo "🔨 Building Linux x86_64..."
cargo zigbuild --release --bin flowlink --target x86_64-unknown-linux-gnu 2>&1 | grep -E "Compiling|Finished|error" | tail -5
LINUX_BIN="target/x86_64-unknown-linux-gnu/release/flowlink"
if [ ! -f "$LINUX_BIN" ]; then echo "❌ Linux build failed!"; exit 1; fi
echo "   $(du -h "$LINUX_BIN" | cut -f1)"

# 3. Build macOS ARM64 (native)
echo ""
echo "🔨 Building macOS ARM64..."
cargo build --release --bin flowlink --target aarch64-apple-darwin 2>&1 | grep -E "Compiling|Finished|error" | tail -5
MAC_BIN="target/aarch64-apple-darwin/release/flowlink"
if [ ! -f "$MAC_BIN" ]; then echo "❌ macOS build failed!"; exit 1; fi
echo "   $(du -h "$MAC_BIN" | cut -f1)"

# 4. Deploy Linux binary to VPS
echo ""
echo "📦 Deploying to VPS..."
scp -q "$LINUX_BIN" "$VPS:/opt/flowlink/bin/flowlink.new"
ssh -o ConnectTimeout=10 "$VPS" '
    kill -9 $(pgrep -f "flowlink relay") 2>/dev/null || true
    sleep 2
    mv /opt/flowlink/bin/flowlink.new /opt/flowlink/bin/flowlink 2>/dev/null
    chmod +x /opt/flowlink/bin/flowlink
    systemctl restart flowlink-relay
    sleep 3
    systemctl is-active --quiet flowlink-relay && echo "✅ Deployed!" || echo "❌ Service failed"
'

# 5. Tag + GitHub Release with both binaries
echo ""
echo "🏷️  Creating release $TAG..."

git tag "$TAG"
git push origin "$TAG"

LINUX_ARCHIVE="/tmp/flowlink-${NEW_VERSION}-linux-amd64.tar.gz"
MAC_ARCHIVE="/tmp/flowlink-${NEW_VERSION}-macos-arm64.tar.gz"

cp "$LINUX_BIN" "/tmp/flowlink" && tar czf "$LINUX_ARCHIVE" -C /tmp flowlink && rm -f /tmp/flowlink
cp "$MAC_BIN" "/tmp/flowlink" && tar czf "$MAC_ARCHIVE" -C /tmp flowlink && rm -f /tmp/flowlink

gh release create "$TAG" "$LINUX_ARCHIVE" "$MAC_ARCHIVE" \
    --title "FlowLink $TAG" \
    --notes "Release $TAG

- **Linux** x86_64 — \`flowlink-${NEW_VERSION}-linux-amd64.tar.gz\`
- **macOS** ARM64 — \`flowlink-${NEW_VERSION}-macos-arm64.tar.gz\`

Built from $(git rev-parse --short HEAD)" \
    --target main

rm -f "$LINUX_ARCHIVE" "$MAC_ARCHIVE"

echo ""
echo "✅ Released!"
echo "🔗 https://github.com/braincreator/flowlink/releases/tag/$TAG"

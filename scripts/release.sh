#!/bin/bash
# FlowLink Release: bump patch, build (linux + macos), deploy, tag, GitHub release
# Usage:
#   ./scripts/release.sh              # interactive (asks confirmation)
#   ./scripts/release.sh --yes        # non-interactive (auto-confirm)
#   ./scripts/release.sh --deploy-only # deploy cached binary, skip build
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

CONFIRM="yes"
MODE="full"
while [ $# -gt 0 ]; do
    case "$1" in
        --yes)         CONFIRM="no" ;;
        --deploy-only) MODE="deploy" ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
    shift
done

# ─── Detect current version from latest git tag ───
LAST_TAG=$(git tag -l --sort=-creatordate | head -1)
if [ -z "$LAST_TAG" ]; then
    echo "❌ No git tags found"
    exit 1
fi

# Strip 'v' prefix and bump patch
CURRENT="${LAST_TAG#v}"
MAJOR=$(echo "$CURRENT" | cut -d. -f1)
MINOR=$(echo "$CURRENT" | cut -d. -f2)
PATCH=$(echo "$CURRENT" | cut -d. -f3)
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"
TAG="v${NEW_VERSION}"

VPS="root@93.93.207.44"
PUBLIC_REPO="braincreator/flowlink-ai-firewall"
LINUX_BIN="target/x86_64-unknown-linux-gnu/release/flowlink"
MAC_BIN="target/aarch64-apple-darwin/release/flowlink"

echo "╔════════════════════════════════════════════╗"
echo "║   FlowLink Release                         ║"
echo "║   ${LAST_TAG} → ${TAG}                       ║"
echo "║   Mode: ${MODE}                             ║"
echo "║   Public: ${PUBLIC_REPO}     ║"
echo "╚════════════════════════════════════════════╝"

# ─── Confirm ───
if [ "$CONFIRM" = "yes" ]; then
    echo ""
    read -rp "Release ${TAG}? [y/N] " confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        echo "Cancelled"
        exit 0
    fi
fi

# ─── 1. Bump version in all crate Cargo.toml files ───
echo ""
echo "📝 [1/5] Bumping version to ${NEW_VERSION}..."
for crate_file in crates/*/Cargo.toml; do
    if grep -q '^version = "' "$crate_file"; then
        sed -i '' "s/^version = \"[0-9]*\.[0-9]*\.[0-9]*\"/version = \"${NEW_VERSION}\"/" "$crate_file"
        echo "   ✅ $(basename $(dirname "$crate_file"))"
    fi
done
git add crates/*/Cargo.toml
git commit -m "chore: bump version to ${NEW_VERSION}"
git push origin main
echo "   Committed & pushed"

# ─── 2. Build Linux x86_64 ───
if [ "$MODE" != "deploy" ]; then
    echo ""
    echo "🔨 [2/5] Cross-compiling Linux x86_64 (zig)..."
    cargo zigbuild --release --bin flowlink --target x86_64-unknown-linux-gnu 2>&1 | tail -10
    if [ ! -f "$LINUX_BIN" ]; then
        echo "❌ Linux build failed!"
        exit 1
    fi
    echo "   ✅ $(du -h "$LINUX_BIN" | cut -f1)"
fi

# ─── 3. Build macOS ARM64 ───
if [ "$MODE" != "deploy" ]; then
    echo ""
    echo "🔨 [3/5] Building macOS ARM64..."
    cargo build --release --bin flowlink --target aarch64-apple-darwin 2>&1 | tail -10
    if [ ! -f "$MAC_BIN" ]; then
        echo "❌ macOS build failed!"
        exit 1
    fi
    echo "   ✅ $(du -h "$MAC_BIN" | cut -f1)"
fi

# ─── 4. Deploy Linux binary to VPS ───
echo ""
echo "🚀 [4/5] Deploying to VPS..."

scp -q "$LINUX_BIN" "$VPS:/opt/flowlink/bin/flowlink.new"

ssh -o ConnectTimeout=10 "$VPS" bash -s << 'REMOTE'
    SERVICE="flowlink-relay"
    BIN="/opt/flowlink/bin/flowlink"

    echo "   Stopping ${SERVICE}..."
    systemctl stop "$SERVICE" 2>/dev/null || true
    sleep 2
    kill -9 $(pgrep -f "flowlink relay") 2>/dev/null || true
    sleep 1

    echo "   Replacing binary..."
    cp "$BIN" "${BIN}.bak" 2>/dev/null || true
    mv "${BIN}.new" "$BIN"
    chmod +x "$BIN"

    echo "   Starting ${SERVICE}..."
    systemctl start "$SERVICE"
    sleep 3

    if systemctl is-active --quiet "$SERVICE"; then
        VERSION=$($BIN --version 2>/dev/null || echo "unknown")
        echo "   ✅ Deployed! (${VERSION})"
    else
        echo "   ❌ Service failed! Rolling back..."
        systemctl stop "$SERVICE" 2>/dev/null || true
        mv "${BIN}.bak" "$BIN" 2>/dev/null
        systemctl start "$SERVICE" 2>/dev/null
        journalctl -u "$SERVICE" --no-pager -n 10
        exit 1
    fi
REMOTE

# ─── 5. Tag + GitHub Release (private) ───
echo ""
echo "🏷️  [5/6] Creating GitHub release ${TAG}..."

git tag "$TAG"
git push origin "$TAG"

LINUX_ARCHIVE="/tmp/flowlink-${NEW_VERSION}-linux-amd64.tar.gz"
MAC_ARCHIVE="/tmp/flowlink-${NEW_VERSION}-macos-arm64.tar.gz"

cp "$LINUX_BIN" "/tmp/flowlink" && tar czf "$LINUX_ARCHIVE" -C /tmp flowlink && rm -f /tmp/flowlink
cp "$MAC_BIN" "/tmp/flowlink" && tar czf "$MAC_ARCHIVE" -C /tmp flowlink && rm -f /tmp/flowlink

COMMITS=$(git log "${LAST_TAG}..HEAD" --oneline | wc -l | tr -d ' ')

gh release create "$TAG" "$LINUX_ARCHIVE" "$MAC_ARCHIVE" \
    --title "FlowLink ${TAG}" \
    --notes "Release ${TAG} — ${COMMITS} commits since ${LAST_TAG}

- **Linux** x86_64 — \`flowlink-${NEW_VERSION}-linux-amd64.tar.gz\`
- **macOS** ARM64 — \`flowlink-${NEW_VERSION}-macos-arm64.tar.gz\`

Built from $(git rev-parse --short HEAD)" \
    --target main 2>&1 || echo "   ⚠️  Release may already exist"

# ─── 6. Mirror release to public repo ───
echo ""
echo "🌐 [6/6] Mirroring to ${PUBLIC_REPO}..."

git tag "$TAG" 2>/dev/null || true
gh release create "$TAG" "$LINUX_ARCHIVE" "$MAC_ARCHIVE" \
    --repo "$PUBLIC_REPO" \
    --title "FlowLink ${TAG}" \
    --notes "Release ${TAG} — ${COMMITS} commits since ${LAST_TAG}

- **Linux** x86_64 — \`flowlink-${NEW_VERSION}-linux-amd64.tar.gz\`
- **macOS** ARM64 — \`flowlink-${NEW_VERSION}-macos-arm64.tar.gz\`

Built from $(git rev-parse --short HEAD)" \
    --target main 2>&1 || echo "   ⚠️  Public release may already exist"

rm -f "$LINUX_ARCHIVE" "$MAC_ARCHIVE"

echo ""
echo "══════════════════════════════════════"
echo "  ✅ Released ${TAG}!"
echo "  🔗 https://github.com/braincreator/flowlink/releases/tag/${TAG}"
echo "  🌐 https://github.com/${PUBLIC_REPO}/releases/tag/${TAG}"
echo "══════════════════════════════════════"

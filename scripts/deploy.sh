#!/bin/bash
# FlowLink CI: zig cross-compile on Mac → deploy to VPS → GitHub release
# Requires: zig, cargo-zigbuild, gh
# Usage:
#   ./scripts/deploy.sh                    # full: build + deploy + release
#   ./scripts/deploy.sh --skip-release     # build + deploy only
#   ./scripts/deploy.sh --deploy-only      # deploy cached binary
#   ./scripts/deploy.sh --tag v1.0.0       # custom tag
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

MODE="full"
TAG=""
while [ $# -gt 0 ]; do
    case "$1" in
        --skip-release) MODE="build-deploy" ;;
        --deploy-only)  MODE="deploy" ;;
        --tag)          TAG="$2"; shift ;;
    esac
    shift
done

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="${TAG:-v$VERSION}"
VPS="root@93.93.207.44"
BIN="target/x86_64-unknown-linux-gnu/release/flowlink"
SERVICE="flowlink-relay"

echo "╔══════════════════════════════════╗"
echo "║   FlowLink Deploy               ║"
echo "║   Version: $VERSION               ║"
echo "║   Tag: $TAG              ║"
echo "║   Mode: $MODE                    ║"
echo "╚══════════════════════════════════╝"

# ─── BUILD ───
if [ "$MODE" = "build-deploy" ] || [ "$MODE" = "full" ]; then
    echo ""
    echo "🔨 [1/3] Cross-compiling with zig (~1.5 min)..."
    cargo zigbuild --release --bin flowlink --target x86_64-unknown-linux-gnu 2>&1 | tail -5
    echo "   ✅ $(du -h "$BIN" | cut -f1) $(file "$BIN" | cut -d: -f2 | xargs)"
fi

# ─── DEPLOY ───
if [ "$MODE" = "deploy" ] || [ "$MODE" = "build-deploy" ] || [ "$MODE" = "full" ]; then
    if [ "$MODE" = "deploy" ] && [ ! -f "$BIN" ]; then
        echo "❌ Binary not found: $BIN"
        exit 1
    fi

    echo ""
    echo "🚀 $([ "$MODE" = "deploy" ] && echo "[1/2]" || echo "[2/3]") Deploying to VPS..."

    scp -q "$BIN" "$VPS:/opt/flowlink/bin/flowlink.new"

    ssh "$VPS" bash -s << 'REMOTE'
        SERVICE="flowlink-relay"
        BIN="/opt/flowlink/bin/flowlink"
        
        echo "   Stopping..."
        systemctl stop "$SERVICE" || true
        sleep 2
        kill -9 $(pgrep -f "flowlink relay") 2>/dev/null || true
        sleep 1
        
        echo "   Replacing..."
        cp "$BIN" "${BIN}.bak" 2>/dev/null || true
        mv "${BIN}.new" "$BIN"
        chmod +x "$BIN"
        
        echo "   Starting..."
        systemctl start "$SERVICE"
        sleep 3
        
        if systemctl is-active --quiet "$SERVICE"; then
            echo "   ✅ Deployed! ($($BIN --version 2>/dev/null))"
        else
            echo "   ❌ Failed! Rolling back..."
            systemctl stop "$SERVICE" 2>/dev/null || true
            mv "${BIN}.bak" "$BIN" 2>/dev/null
            systemctl start "$SERVICE" 2>/dev/null
            journalctl -u "$SERVICE" --no-pager -n 5
            exit 1
        fi
REMOTE
fi

# ─── GITHUB RELEASE ───
if [ "$MODE" = "full" ]; then
    echo ""
    echo "🏷️  [3/3] Creating release $TAG..."

    ARCHIVE="/tmp/flowlink-${TAG#v}-linux-amd64.tar.gz"
    cp "$BIN" "/tmp/flowlink"
    tar czf "$ARCHIVE" -C /tmp flowlink
    rm -f /tmp/flowlink

    if ! git rev-parse "$TAG" >/dev/null 2>&1; then
        git tag "$TAG"
        git push origin "$TAG" 2>/dev/null || true
    fi

    gh release create "$TAG" "$ARCHIVE" \
        --title "FlowLink $TAG" \
        --notes "Release $TAG — $(git rev-parse --short HEAD)" \
        --target main 2>&1 || echo "   ⚠️  Release already exists"

    echo "   🔗 https://github.com/braincreator/flowlink/releases/tag/$TAG"
    rm -f "$ARCHIVE"
fi

echo ""
echo "✅ Done!"

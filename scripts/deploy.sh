#!/bin/bash
# FlowLink CI: build on VPS → deploy → GitHub release
# Usage:
#   ./scripts/deploy.sh                  # full pipeline
#   ./scripts/deploy.sh --skip-release   # build + deploy, no release
#   ./scripts/deploy.sh --skip-build     # deploy cached binary + release
#   ./scripts/deploy.sh --tag v1.0.0     # custom tag
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

SKIP_BUILD=false
SKIP_RELEASE=false
TAG=""
while [ $# -gt 0 ]; do
    case "$1" in
        --skip-build)   SKIP_BUILD=true ;;
        --skip-release) SKIP_RELEASE=true ;;
        --tag)          TAG="$2"; shift ;;
    esac
    shift
done

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="${TAG:-v$VERSION}"
VPS="root@93.93.207.44"
REMOTE_DIR="/root/fl-build"
SERVICE="flowlink-relay"

echo "╔══════════════════════════════════╗"
echo "║   FlowLink Deploy               ║"
echo "║   Version: $VERSION               ║"
echo "║   Tag: $TAG              ║"
echo "╚══════════════════════════════════╝"

# 1. Build on VPS
if [ "$SKIP_BUILD" = false ]; then
    echo ""
    echo "📦 [1/3] Uploading source to VPS..."
    tar czf /tmp/fl-src.tar.gz \
        --exclude="target" --exclude=".git" --exclude="video" \
        --exclude="website" --exclude="docs" --exclude="scripts" \
        --exclude='*.md' \
        Cargo.toml Cargo.lock crates/
    scp -q /tmp/fl-src.tar.gz "$VPS:/tmp/"
    rm -f /tmp/fl-src.tar.gz

    echo "🔨 [2/3] Building on VPS..."
    ssh "$VPS" bash -s << REMOTE
        set -euo pipefail
        cd $REMOTE_DIR
        tar xzf /tmp/fl-src.tar.gz
        . /root/.cargo/env
        cargo build --release --bin flowlink
        echo "✅ Build done: \$(ls -lh target/release/flowlink | awk '{print \$5}')"
REMOTE
fi

# 2. Deploy
if [ "$SKIP_BUILD" = true ]; then
    echo ""
    echo "🚀 [1/2] Deploying cached binary..."
else
    echo ""
    echo "🚀 [3/3] Deploying..."
fi

ssh "$VPS" bash -s << REMOTE
    set -euo pipefail
    BIN="/opt/flowlink/bin/flowlink"
    
    echo "   Stopping $SERVICE..."
    systemctl stop $SERVICE || true
    sleep 2
    kill -9 \$(pgrep -f "flowlink relay") 2>/dev/null || true
    sleep 1
    
    echo "   Replacing binary..."
    cp "$BIN" "\${BIN}.bak" 2>/dev/null || true
    cp $REMOTE_DIR/target/release/flowlink "$BIN"
    chmod +x "$BIN"
    
    echo "   Starting $SERVICE..."
    systemctl start $SERVICE
    sleep 3
    
    if systemctl is-active --quiet $SERVICE; then
        VERSION=\$(\$BIN --version 2>/dev/null || echo "?")
        echo "   ✅ Deployed! (\$VERSION)"
    else
        echo "   ❌ Failed! Rolling back..."
        systemctl stop $SERVICE 2>/dev/null || true
        mv "\${BIN}.bak" "$BIN" 2>/dev/null
        systemctl start $SERVICE 2>/dev/null
        journalctl -u $SERVICE --no-pager -n 10
        exit 1
    fi
REMOTE

# 3. GitHub Release
if [ "$SKIP_RELEASE" = false ]; then
    echo ""
    echo "🏷️  Creating release $TAG..."

    ARCHIVE="/tmp/flowlink-${TAG#v}-linux-amd64.tar.gz"
    scp -q "$VPS:$REMOTE_DIR/target/release/flowlink" "/tmp/flowlink"
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

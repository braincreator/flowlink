#!/bin/bash
# FlowLink Local CI: Docker build → deploy to VPS → GitHub release
# Requires: Colima (x86_64), gh CLI
# Usage:
#   ./scripts/ci-deploy.sh                    # full: build + deploy + release
#   ./scripts/ci-deploy.sh --build-only       # just build
#   ./scripts/ci-deploy.sh --deploy-only      # just deploy (use cached binary)
#   ./scripts/ci-deploy.sh --tag v1.0.0       # custom tag
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# Parse args
MODE="full"
TAG=""
while [ $# -gt 0 ]; do
    case "$1" in
        --build-only)  MODE="build" ;;
        --deploy-only) MODE="deploy" ;;
        --tag)         TAG="$2"; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
    shift
done

# Config
IMAGE="flowlink-builder"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="${TAG:-v$VERSION}"
LOCAL_BIN="/tmp/flowlink-$TAG-linux-amd64"
ARCHIVE="/tmp/flowlink-${TAG#v}-linux-amd64.tar.gz"
VPS="root@93.93.207.44"
SERVICE="flowlink-relay"

echo "╔══════════════════════════════════╗"
echo "║   FlowLink Local CI             ║"
echo "║   Version: $VERSION               ║"
echo "║   Tag: $TAG              ║"
echo "║   Mode: $MODE                    ║"
echo "╚══════════════════════════════════╝"
echo ""

# Ensure Colima is running
if ! colima status &>/dev/null; then
    echo "⚠️  Colima not running, starting..."
    colima start --arch x86_64 --cpu 2 --memory 4 2>&1 | tail -3
fi

# ─── BUILD ───
if [ "$MODE" = "build" ] || [ "$MODE" = "full" ]; then
    echo "📦 [1/3] Building in Docker (linux/amd64)..."
    
    docker build -t "$IMAGE" -f Dockerfile . 2>&1 | tail -10
    if [ $? -ne 0 ]; then
        echo "❌ Build failed!"
        exit 1
    fi
    
    echo ""
    echo "📥 [2/3] Extracting binary..."
    docker create --name fl-extract "$IMAGE" 2>/dev/null || docker rm -f fl-extract >/dev/null 2>&1
    docker create --name fl-extract "$IMAGE"
    docker cp fl-extract:/usr/local/bin/flowlink "$LOCAL_BIN"
    docker rm fl-extract >/dev/null 2>&1
    chmod +x "$LOCAL_BIN"
    
    echo "   Binary: $(du -h "$LOCAL_BIN" | cut -f1)"
fi

# ─── DEPLOY ───
if [ "$MODE" = "deploy" ] || [ "$MODE" = "full" ]; then
    if [ ! -f "$LOCAL_BIN" ]; then
        echo "❌ Binary not found: $LOCAL_BIN (run --build-only first)"
        exit 1
    fi
    
    echo ""
    echo "🚀 [3/3] Deploying to VPS ($VPS)..."
    
    scp -q "$LOCAL_BIN" "$VPS:/opt/flowlink/bin/flowlink.new"
    
    ssh "$VPS" bash << 'REMOTE'
        set -euo pipefail
        SERVICE="flowlink-relay"
        BIN="/opt/flowlink/bin/flowlink"
        NEW="/opt/flowlink/bin/flowlink.new"
        
        echo "   Stopping service..."
        systemctl stop "$SERVICE" || true
        sleep 2
        kill -9 $(pgrep -f "flowlink relay") 2>/dev/null || true
        sleep 1
        
        echo "   Replacing binary..."
        cp "$BIN" "${BIN}.bak" 2>/dev/null || true
        mv "$NEW" "$BIN"
        chmod +x "$BIN"
        
        echo "   Starting service..."
        systemctl start "$SERVICE"
        sleep 3
        
        if systemctl is-active --quiet "$SERVICE"; then
            VERSION=$(/opt/flowlink/bin/flowlink --version 2>/dev/null || echo "?")
            echo "   ✅ Deployed! ($VERSION)"
        else
            echo "   ❌ Failed! Rolling back..."
            systemctl stop "$SERVICE" 2>/dev/null || true
            mv "${BIN}.bak" "$BIN" 2>/dev/null
            systemctl start "$SERVICE" 2>/dev/null
            journalctl -u "$SERVICE" --no-pager -n 10
            exit 1
        fi
REMOTE
fi

# ─── GITHUB RELEASE ───
if [ "$MODE" = "full" ]; then
    echo ""
    echo "🏷️  Creating GitHub release $TAG..."
    
    # Archive
    cp "$LOCAL_BIN" "/tmp/flowlink"
    tar czf "$ARCHIVE" -C /tmp flowlink
    rm -f /tmp/flowlink
    
    # Tag
    if ! git rev-parse "$TAG" >/dev/null 2>&1; then
        git tag "$TAG"
        git push origin "$TAG" 2>/dev/null || true
    fi
    
    # Release
    gh release create "$TAG" "$ARCHIVE" \
        --title "FlowLink $TAG" \
        --notes "Release $TAG — built from $(git rev-parse --short HEAD) on $(date -u +%Y-%m-%d)" \
        --target main 2>&1 || echo "   ⚠️  Release already exists or failed"
    
    echo "   🔗 https://github.com/braincreator/flowlink/releases/tag/$TAG"
    
    # Cleanup
    rm -f "$LOCAL_BIN" "$ARCHIVE"
fi

echo ""
echo "✅ Done!"

#!/bin/bash
# FlowLink deploy script — runs on VPS
# Usage: ./deploy.sh [skip-build]
set -e

FL_DIR="/root/fl-build"
BIN="/opt/flowlink/bin/flowlink"
SERVICE="flowlink-relay"

echo "=== FlowLink Deploy ==="

# 1. Build if needed
if [ "${1:-}" != "skip-build" ]; then
    echo "[1/4] Building..."
    cd "$FL_DIR"
    . /root/.cargo/env
    cargo build --release --bin flowlink 2>&1 | tail -3
    echo "Build done."
else
    echo "[1/4] Skipping build (using existing binary)"
fi

# 2. Stop service
echo "[2/4] Stopping service..."
systemctl stop "$SERVICE" || true
sleep 2
kill -9 $(pgrep -f "flowlink relay") 2>/dev/null || true
sleep 1

# 3. Replace binary
echo "[3/4] Replacing binary..."
cp "$FL_DIR/target/release/flowlink" "$BIN"
chmod +x "$BIN"

# 4. Start service
echo "[4/4] Starting service..."
systemctl start "$SERVICE"
sleep 3

if systemctl is-active --quiet "$SERVICE"; then
    echo "✅ Deploy successful!"
    /opt/flowlink/bin/flowlink --version
else
    echo "❌ Service failed!"
    journalctl -u "$SERVICE" --no-pager -n 10
    exit 1
fi

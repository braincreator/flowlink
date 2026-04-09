#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════
# FlowLink Agent — One-line Install Script
# Usage: curl -sSL https://get.flowlink.sh | sh -s -- YOUR_TOKEN
# ═══════════════════════════════════════════════

VERSION="${FLOWLINK_VERSION:-latest}"
INSTALL_DIR="${FLOWLINK_INSTALL_DIR:-/usr/local/bin}"
CONFIG_DIR="${FLOWLINK_CONFIG_DIR:-/etc/flowlink}"
BACKUP_DIR="${FLOWLINK_BACKUP_DIR:-/var/lib/flowlink/backups}"
TOKEN="${1:-}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}►${NC} $*"; }
ok()    { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}!${NC} $*"; }
fail()  { echo -e "${RED}✗${NC} $*" >&2; exit 1; }

# ─── Preflight Checks ───────────────────────────

info "FlowLink Agent Installer v${VERSION}"

if [ "$(id -u)" -ne 0 ]; then
    warn "Not running as root — will install to ~/.local/bin instead"
    INSTALL_DIR="${HOME}/.local/bin"
    CONFIG_DIR="${HOME}/.flowlink"
    BACKUP_DIR="${HOME}/.flowlink/backups"
fi

command -v curl >/dev/null 2>&1 || fail "curl is required. Install: apt install curl / yum install curl"
command -v tar  >/dev/null 2>&1 || fail "tar is required"

# ─── Detect OS / Arch ──────────────────────────

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "darwin" ;;
        *)          fail "Unsupported OS: $os" ;;
    esac
}

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)   echo "amd64" ;;
        aarch64|arm64)  echo "arm64" ;;
        *)              fail "Unsupported architecture: $arch" ;;
    esac
}

OS="$(detect_os)"
ARCH="$(detect_arch)"
BINARY_NAME="flowlink-agent"
ARCHIVE_NAME="flowlink-agent-${OS}-${ARCH}.tar.gz"

ok "Detected: ${OS}/${ARCH}"

# ─── Download Binary ───────────────────────────

DOWNLOAD_URL="https://github.com/nicepkg/flowlink/releases/${VERSION}/download/${ARCHIVE_NAME}"

info "Downloading ${BINARY_NAME}..."
curl -fsSL "$DOWNLOAD_URL" -o "/tmp/${ARCHIVE_NAME}" || {
    # Fallback: check if binary exists locally
    if [ -f "./target/release/${BINARY_NAME}" ]; then
        warn "Download failed, using local binary from ./target/release/"
        cp "./target/release/${BINARY_NAME}" "/tmp/${BINARY_NAME}"
    else
        fail "Download failed. Check: ${DOWNLOAD_URL}"
    fi
}

ok "Downloaded to /tmp/${ARCHIVE_NAME}"

# ─── Install ───────────────────────────────────

info "Installing to ${INSTALL_DIR}/..."
mkdir -p "$INSTALL_DIR"

if [ -f "/tmp/${ARCHIVE_NAME}" ]; then
    # Archive — extract
    tar xzf "/tmp/${ARCHIVE_NAME}" -C /tmp/ 2>/dev/null || true
    cp "/tmp/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    rm -f "/tmp/${ARCHIVE_NAME}" "/tmp/${BINARY_NAME}"
else
    fail "Binary not found"
fi

ok "Installed: ${INSTALL_DIR}/${BINARY_NAME}"

# ─── Configure ─────────────────────────────────

mkdir -p "$CONFIG_DIR" "$BACKUP_DIR"

CONFIG_FILE="${CONFIG_DIR}/config.json"

if [ ! -f "$CONFIG_FILE" ]; then
    info "Creating config..."
    cat > "$CONFIG_FILE" <<EOF
{
  "agent_id": "$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || echo 'local-$(date +%s)')",
  "relay_url": "wss://control.flowlink.app/ws",
  "token": "${TOKEN}",
  "backup": {
    "enabled": true,
    "backup_dir": "${BACKUP_DIR}",
    "max_snapshots": 50,
    "retention_days": 30,
    "compression": "gzip"
  },
  "sandbox": {
    "allowed_dirs": ["/"],
    "blocked_patterns": [],
    "max_file_size": 104857600,
    "max_exec_timeout": 300,
    "allow_sudo": false
  }
}
EOF
    chmod 600 "$CONFIG_FILE"
    ok "Config: ${CONFIG_FILE}"
else
    warn "Config already exists: ${CONFIG_FILE} (not overwritten)"
    # Update token if provided
    if [ -n "$TOKEN" ]; then
        if command -v jq >/dev/null 2>&1; then
            jq --arg tok "$TOKEN" '.token = $tok' "$CONFIG_FILE" > "${CONFIG_FILE}.tmp" && mv "${CONFIG_FILE}.tmp" "$CONFIG_FILE"
            ok "Token updated in config"
        fi
    fi
fi

# ─── Systemd Service (Linux) ───────────────────

setup_systemd() {
    local service_file="/etc/systemd/system/flowlink-agent.service"

    info "Setting up systemd service..."
    cat > "$service_file" <<EOF
[Unit]
Description=FlowLink Agent — AI Security Shield
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/${BINARY_NAME} --config ${CONFIG_FILE}
Restart=on-failure
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable flowlink-agent
    ok "Service installed: flowlink-agent"
}

setup_launchd() {
    local plist_file="${HOME}/Library/LaunchAgents/com.flowlink.agent.plist"

    info "Setting up launchd service..."
    cat > "$plist_file" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.flowlink.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>${INSTALL_DIR}/${BINARY_NAME}</string>
        <string>--config</string>
        <string>${CONFIG_FILE}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/flowlink-agent.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/flowlink-agent.err</string>
</dict>
</plist>
EOF

    launchctl load "$plist_file" 2>/dev/null || warn "launchctl load failed (try manually)"
    ok "Service installed: com.flowlink.agent"
}

case "$OS" in
    linux)
        if command -v systemctl >/dev/null 2>&1; then
            setup_systemd
        else
            warn "systemd not found — service not installed. Run manually: ${INSTALL_DIR}/${BINARY_NAME} --config ${CONFIG_FILE}"
        fi
        ;;
    darwin)
        if [ "$(id -u)" -ne 0 ]; then
            setup_launchd
        else
            warn "On macOS, run install without sudo for launchd support"
        fi
        ;;
esac

# ─── Done ──────────────────────────────────────

echo ""
echo -e "${GREEN}═══════════════════════════════════════${NC}"
echo -e "${GREEN}  FlowLink Agent installed successfully!${NC}"
echo -e "${GREEN}═══════════════════════════════════════${NC}"
echo ""
echo "  Binary:  ${INSTALL_DIR}/${BINARY_NAME}"
echo "  Config:  ${CONFIG_FILE}"
echo "  Backup:  ${BACKUP_DIR}"
echo ""

if [ -n "$TOKEN" ]; then
    echo -e "  ${BLUE}Token configured${NC} — agent will connect to control plane"
else
    warn "No token provided — set it in ${CONFIG_FILE} or re-run with token"
fi

echo ""
echo "  Commands:"
echo "    flowlink-agent --config ${CONFIG_FILE}   # Start agent"
if [ "$OS" = "linux" ] && command -v systemctl >/dev/null 2>&1; then
    echo "    systemctl start flowlink-agent          # Start via systemd"
    echo "    systemctl status flowlink-agent         # Check status"
    echo "    journalctl -u flowlink-agent -f          # View logs"
fi
echo ""

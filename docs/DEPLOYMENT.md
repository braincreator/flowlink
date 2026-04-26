# Deployment Guide

## Installation on VPS (Ubuntu/Debian)

### Prerequisites

- Ubuntu 22.04+ or Debian 12+
- Public IP address or domain name
- Open ports: 80, 443, 8443, 8080

### Option 1: Install Script (Recommended)

```bash
curl -sSL https://flowlink.flow-masters.ru/install.sh | bash
```

The script will:
1. Download the latest `flowlink-relay` binary
2. Create config at `/etc/flowlink/relay.yaml`
3. Create systemd service
4. Start the relay

### Option 2: Manual Installation

```bash
# 1. Download binary
wget https://github.com/braincreator/flowlink/releases/latest/download/flowlink-relay-linux-amd64 -O /usr/local/bin/flowlink-relay
chmod +x /usr/local/bin/flowlink-relay

# 2. Create directories
mkdir -p /etc/flowlink /var/lib/flowlink/{clients,agents,audit,billing,tls-cache}

# 3. Create config
cat > /etc/flowlink/relay.yaml << 'EOF'
wss_addr: ":8443"
api_addr: ":8080"
tls_mode: "letsencrypt"
tls_domain: "relay.yourdomain.com"
tls_cache: "/var/lib/flowlink/tls-cache"
api_token: "GENERATE_A_STRONG_TOKEN_HERE"
data_dir: "/var/lib/flowlink"
rate_limit_rpm: 60
EOF

# 4. Create systemd service
cat > /etc/systemd/system/flowlink-relay.service << 'EOF'
[Unit]
Description=FlowLink Relay Server
After=network.target

[Service]
Type=simple
User=flowlink
Group=flowlink
ExecStart=/usr/local/bin/flowlink-relay -config /etc/flowlink/relay.yaml
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

# 5. Create user and set permissions
useradd -r -s /bin/false flowlink
chown -R flowlink:flowlink /etc/flowlink /var/lib/flowlink

# 6. Start
systemctl daemon-reload
systemctl enable flowlink-relay
systemctl start flowlink-relay
```

---

## Nginx Reverse Proxy

Recommended for production. Handles TLS termination, compression, and security headers.

```nginx
# /etc/nginx/sites-available/flowlink
server {
    listen 80;
    server_name relay.yourdomain.com;

    location / {
        return 301 https://$server_name$request_uri;
    }

    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
    }
}

server {
    listen 443 ssl http2;
    server_name relay.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/relay.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/relay.yourdomain.com/privkey.pem;

    # Security headers
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header X-XSS-Protection "1; mode=block";

    # WebSocket (agent connections)
    location /ws {
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 86400;
        proxy_send_timeout 86400;
    }

    # HTTP API + MCP
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # SSE support
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400;
    }
}
```

```bash
ln -s /etc/nginx/sites-available/flowlink /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx
```

### With Nginx, update relay config

```yaml
# /etc/flowlink/relay.yaml
tls_mode: "off"              # Nginx handles TLS
api_addr: "127.0.0.1:8080"   # Listen on localhost only
wss_addr: "127.0.0.1:8443"   # Listen on localhost only
```

---

## Let's Encrypt Certificate

### Option A: Standalone (relay handles TLS)

The relay has built-in Let's Encrypt support via `tls_mode: "letsencrypt"`. Ensure port 80 is open and the domain resolves to your server.

### Option B: Via Nginx + Certbot

```bash
# Install certbot
apt install -y certbot python3-certbot-nginx

# Get certificate
certbot --nginx -d relay.yourdomain.com

# Auto-renewal (already configured by certbot)
certbot renew --dry-run
```

---

## Systemd Service Management

```bash
# Start / Stop / Restart
systemctl start flowlink-relay
systemctl stop flowlink-relay
systemctl restart flowlink-relay

# View logs
journalctl -u flowlink-relay -f

# Service status
systemctl status flowlink-relay
```

---

## Updating

### Via install script

```bash
curl -sSL https://flowlink.flow-masters.ru/install.sh | bash
```

The script detects existing installation and updates in-place.

### Manual update

```bash
# 1. Stop service
systemctl stop flowlink-relay

# 2. Download new binary
wget https://github.com/braincreator/flowlink/releases/latest/download/flowlink-relay-linux-amd64 -O /usr/local/bin/flowlink-relay
chmod +x /usr/local/bin/flowlink-relay

# 3. Start service
systemctl start flowlink-relay

# 4. Verify
journalctl -u flowlink-relay --since "1 min ago"
```

---

## Backup

### Relay data

```bash
# Create backup
tar czf flowlink-backup-$(date +%Y%m%d).tar.gz /etc/flowlink /var/lib/flowlink

# Restore
systemctl stop flowlink-relay
tar xzf flowlink-backup-20260327.tar.gz -C /
systemctl start flowlink-relay
```

### Automated backup (cron)

```bash
# Daily backup at 3 AM
echo "0 3 * * * root tar czf /var/backups/flowlink-$(date +\%Y\%m\%d).tar.gz /etc/flowlink /var/lib/flowlink && find /var/backups -name 'flowlink-*' -mtime +30 -delete" > /etc/cron.d/flowlink-backup
```

---

## Troubleshooting

### Agent won't connect

```bash
# Check relay is running
systemctl status flowlink-relay

# Check firewall
ufw allow 8443/tcp
ufw allow 80/tcp
ufw allow 443/tcp

# Check DNS resolution
dig relay.yourdomain.com

# Test WSS connection
wscat -c wss://relay.yourdomain.com/ws
```

### TLS errors

```bash
# Check certificate
openssl s_client -connect relay.yourdomain.com:443 -servername relay.yourdomain.com </dev/null 2>/dev/null | openssl x509 -noout -dates

# Check certbot renewal
certbot certificates

# Renew manually
certbot renew
```

### Relay not responding

```bash
# Check logs
journalctl -u flowlink-relay -f --no-pager -n 100

# Check port binding
ss -tlnp | grep -E '8080|8443'

# Check disk space
df -h /var/lib/flowlink

# Check memory
free -h
```

### Permission errors

```bash
# Fix ownership
chown -R flowlink:flowlink /etc/flowlink /var/lib/flowlink
chmod 600 /etc/flowlink/relay.yaml
```

### Rate limiting

```bash
# Check rate limit in config
grep rate_limit /etc/flowlink/relay.yaml

# Temporarily increase for debugging
# rate_limit_rpm: 0  # disables rate limiting
```

---

## Firewall Configuration

### UFW (Ubuntu)

```bash
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp      # SSH
ufw allow 80/tcp      # HTTP (Let's Encrypt)
ufw allow 443/tcp     # HTTPS
ufw allow 8443/tcp    # WSS (direct, without Nginx)
ufw enable
```

### With Nginx reverse proxy, only 80 and 443 are needed externally.

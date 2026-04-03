---
name: SSL Manager
version: 0.1.0
description: Manage SSL certificates, check expiration, auto-renew with Let's Encrypt.
icon: 🔒
commands:
  - name: ssl_list
    description: List all SSL certificates on server
    run: |
      echo "=== Let's Encrypt Certificates ===" && \
      certbot certificates 2>/dev/null || echo "Certbot not found" && \
      echo "" && \
      echo "=== Nginx SSL Certificates ===" && \
      grep -r "ssl_certificate" /etc/nginx/sites-enabled/ 2>/dev/null | awk -F: '{print $2}' | sort -u
    timeout: 15
  - name: ssl_check
    description: Check certificate expiration date
    run: |
      DOMAIN={domain} && \
      echo | openssl s_client -servername $DOMAIN -connect $DOMAIN:443 2>/dev/null | \
      openssl x509 -noout -dates -subject 2>/dev/null || \
      echo "Certificate check failed for $DOMAIN"
    timeout: 20
    args:
      - name: domain
        required: true
        description: Domain name to check
  - name: ssl_renew
    description: Renew all Let's Encrypt certificates
    run: certbot renew --quiet
    timeout: 120
  - name: ssl_renew_domain
    description: Renew certificate for specific domain
    run: certbot renew --cert-name {domain} --force-renewal
    timeout: 120
    args:
      - name: domain
        required: true
        description: Domain name
  - name: ssl_info
    description: Get detailed certificate information
    run: |
      DOMAIN={domain} && \
      echo | openssl s_client -servername $DOMAIN -connect $DOMAIN:443 2>/dev/null | \
      openssl x509 -noout -text 2>/dev/null | \
      grep -E "Issuer:|Subject:|Not Before|Not After|DNS:|Signature Algorithm"
    timeout: 20
    args:
      - name: domain
        required: true
        description: Domain name
  - name: ssl_install
    description: Install new Let's Encrypt certificate
    run: certbot certonly --nginx -d {domain} --non-interactive --agree-tos --email {email}
    timeout: 120
    args:
      - name: domain
        required: true
        description: Domain name
      - name: email
        required: true
        description: Email for notifications
  - name: ssl_test
    description: Test SSL configuration (SSL Labs style)
    run: |
      DOMAIN={domain} && \
      echo "Testing $DOMAIN..." && \
      echo | openssl s_client -servername $DOMAIN -connect $DOMAIN:443 -tlsextdebug 2>/dev/null | \
      grep -E "Protocol|Cipher|Verify return code"
    timeout: 15
    args:
      - name: domain
        required: true
        description: Domain name
auto_checks:
  - name: ssl_expiration
    description: Check all certificates for expiration
    interval: 86400
    run: certbot certificates 2>/dev/null | grep -E "Expiry Date|Domains"
    notifications:
      - days: 14
        message: "⚠️ Certificate expires in 14 days"
      - days: 7
        message: "🔴 Certificate expires in 7 days"
      - days: 3
        message: "🚨 CRITICAL: Certificate expires in 3 days"
---

# SSL Manager

Automated SSL certificate management with Let's Encrypt integration.

## Features

### Certificate Management
- **List certificates** — View all installed SSL certificates
- **Check expiration** — Get remaining days before expiry
- **Renew certificates** — Auto-renew with certbot
- **Install new** — Add certificates for new domains

### Monitoring
- **Daily checks** — Automatic expiration monitoring
- **Telegram alerts** — Notifications at 14/7/3 days before expiry
- **Test SSL** — Verify SSL configuration

## Usage Examples

```bash
# List all certificates
ssl_list

# Check specific domain
ssl_check domain=flow-masters.ru

# Get detailed info
ssl_info domain=flow-masters.ru

# Renew all certificates
ssl_renew

# Install new certificate
ssl_install domain=api.example.com email=admin@example.com

# Test SSL configuration
ssl_test domain=flow-masters.ru
```

## Auto-Check Schedule

The skill runs daily checks on all certificates:

| Days Left | Alert Level | Action |
|-----------|-------------|--------|
| 14 days | ⚠️ Warning | Telegram notification |
| 7 days | 🔴 High | Telegram + auto-renew attempt |
| 3 days | 🚨 Critical | Telegram + force renew |

## Certificate Types

### Let's Encrypt (Recommended)
- Free, automated
- 90-day validity
- Auto-renewal support
- Wildcard support (DNS challenge)

### Self-Signed
- Manual installation
- No auto-renewal
- Useful for internal services

## Renewal Process

```bash
# Dry run (test without making changes)
certbot renew --dry-run

# Force renew specific domain
ssl_renew_domain domain=flow-masters.ru

# Reload nginx after renewal
nginx -s reload
```

## Troubleshooting

### Common Issues

1. **Port 80 blocked** — Let's Encrypt requires port 80 for HTTP-01 challenge
2. **Rate limits** — 5 certificates per domain per week
3. **DNS propagation** — Wait for DNS to propagate before requesting

### Check renewal status
```bash
certbot certificates
```

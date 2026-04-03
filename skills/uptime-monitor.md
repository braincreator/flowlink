---
name: Uptime Monitor
version: 0.1.0
description: Monitor HTTP endpoints, TCP ports, and DNS resolution with Telegram alerts.
icon: 📡
commands:
  - name: uptime_check
    description: Check single endpoint status
    run: |
      URL={url} && \
      START=$(date +%s%3N) && \
      STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time {timeout} $URL 2>/dev/null || echo "000") && \
      END=$(date +%s%3N) && \
      LATENCY=$((END - START)) && \
      echo "URL: $URL" && \
      echo "Status: $STATUS" && \
      echo "Latency: ${LATENCY}ms" && \
      if [ "$STATUS" = "200" ] || [ "$STATUS" = "301" ] || [ "$STATUS" = "302" ]; then \
        echo "✅ UP"; \
      else \
        echo "❌ DOWN"; \
      fi
    timeout: 30
    args:
      - name: url
        required: true
        description: URL to check
      - name: timeout
        required: false
        description: Timeout in seconds
        default: "10"
  - name: uptime_check_tcp
    description: Check TCP port status
    run: |
      HOST={host} && \
      PORT={port} && \
      START=$(date +%s%3N) && \
      if timeout {timeout} bash -c "echo >/dev/tcp/$HOST/$PORT" 2>/dev/null; then \
        END=$(date +%s%3N) && \
        LATENCY=$((END - START)) && \
        echo "Host: $HOST:$PORT" && \
        echo "Latency: ${LATENCY}ms" && \
        echo "✅ PORT OPEN"; \
      else \
        echo "Host: $HOST:$PORT" && \
        echo "❌ PORT CLOSED"; \
      fi
    timeout: 15
    args:
      - name: host
        required: true
        description: Hostname or IP
      - name: port
        required: true
        description: Port number
      - name: timeout
        required: false
        default: "5"
  - name: uptime_check_dns
    description: Check DNS resolution
    run: |
      DOMAIN={domain} && \
      START=$(date +%s%3N) && \
      RESULT=$(dig +short $DOMAIN @8.8.8.8 2>/dev/null | head -1) && \
      END=$(date +%s%3N) && \
      LATENCY=$((END - START)) && \
      echo "Domain: $DOMAIN" && \
      echo "Resolved: $RESULT" && \
      echo "Latency: ${LATENCY}ms" && \
      if [ -n "$RESULT" ]; then \
        echo "✅ DNS OK"; \
      else \
        echo "❌ DNS FAILED"; \
      fi
    timeout: 10
    args:
      - name: domain
        required: true
        description: Domain to resolve
  - name: uptime_status
    description: Show status of all monitored endpoints
    run: |
      echo "=== Monitored Endpoints ===" && \
      cat /etc/flowlink/endpoints.json 2>/dev/null | jq -r '.[] | "\(.name): \(.url)"' 2>/dev/null || \
      echo "No endpoints configured" && \
      echo "" && \
      echo "=== Recent Checks ===" && \
      tail -20 /var/log/flowlink/uptime.log 2>/dev/null || echo "No check history"
    timeout: 10
  - name: uptime_history
    description: Show uptime history for period
    run: |
      DAYS={days} && \
      echo "=== Uptime History (Last $DAYS days) ===" && \
      grep "$(date -d "$DAYS days ago" +%Y-%m-%d)" /var/log/flowlink/uptime.log 2>/dev/null | \
      awk '{print $1, $2, $4, $6}' | sort | uniq -c | tail -50 || \
      echo "No history available"
    timeout: 15
    args:
      - name: days
        required: false
        description: Number of days
        default: "7"
  - name: uptime_add
    description: Add endpoint to monitoring
    run: |
      mkdir -p /etc/flowlink && \
      cat /etc/flowlink/endpoints.json 2>/dev/null || echo '[]' > /etc/flowlink/endpoints.json && \
      jq '. += [{"name": "{name}", "url": "{url}", "interval": {interval}, "alerts": {alerts}}]' \
      /etc/flowlink/endpoints.json > /tmp/endpoints.json && \
      mv /tmp/endpoints.json /etc/flowlink/endpoints.json && \
      echo "Added {name} to monitoring"
    timeout: 5
    args:
      - name: name
        required: true
        description: Endpoint name
      - name: url
        required: true
        description: URL to monitor
      - name: interval
        required: false
        description: Check interval in seconds
        default: "60"
      - name: alerts
        required: false
        description: Enable Telegram alerts
        default: "true"
  - name: uptime_remove
    description: Remove endpoint from monitoring
    run: |
      jq 'del(.[] | select(.name == "{name}"))' /etc/flowlink/endpoints.json > /tmp/endpoints.json && \
      mv /tmp/endpoints.json /etc/flowlink/endpoints.json && \
      echo "Removed {name} from monitoring"
    timeout: 5
    args:
      - name: name
        required: true
        description: Endpoint name
protocols:
  - name: http
    description: HTTP/HTTPS endpoints (status code + latency)
    check: curl
  - name: tcp
    description: TCP port connectivity
    check: bash /dev/tcp
  - name: dns
    description: DNS resolution check
    check: dig
alerting:
  channel: telegram
  rules:
    - condition: downtime_consecutive
      threshold: 3
      message: "🔴 {name} is DOWN for 3 consecutive checks"
    - condition: recovery
      message: "✅ {name} is back UP"
check_intervals:
  - name: fast
    seconds: 30
    description: Critical services
  - name: normal
    seconds: 60
    description: Standard endpoints
  - name: slow
    seconds: 300
    description: Low-priority services
---

# Uptime Monitor

Multi-protocol uptime monitoring with Telegram alerts.

## Supported Protocols

| Protocol | Check Type | Metrics |
|----------|------------|---------|
| HTTP/HTTPS | Status code, latency | Response time, up/down |
| TCP | Port connectivity | Open/closed, latency |
| DNS | Resolution | IP resolved, latency |

## Usage Examples

### HTTP Monitoring

```bash
# Check single URL
uptime_check url=https://flow-masters.ru

# Add to monitoring
uptime_add name=flowmasters url=https://flow-masters.ru interval=60 alerts=true
```

### TCP Monitoring

```bash
# Check SSH port
uptime_check_tcp host=93.93.207.44 port=22

# Check database port
uptime_check_tcp host=localhost port=5432
```

### DNS Monitoring

```bash
# Check domain resolution
uptime_check_dns domain=flow-masters.ru
```

### Management

```bash
# Show all monitored endpoints
uptime_status

# View history
uptime_history days=7

# Remove endpoint
uptime_remove name=flowmasters
```

## Check Intervals

| Interval | Seconds | Use Case |
|----------|---------|----------|
| Fast | 30 | Critical services |
| Normal | 60 | Standard endpoints |
| Slow | 300 | Low-priority services |

## Alerting

### Telegram Alerts

Alerts are sent to Telegram when:
1. **Down alert** — 3 consecutive failed checks
2. **Recovery alert** — Service comes back up

### Alert Format

```
🔴 flowmasters is DOWN for 3 consecutive checks
URL: https://flow-masters.ru
Last Status: 502
Latency: 1023ms
Time: 2024-01-15 14:30:00 MSK
```

## Configuration File

Endpoints are stored in `/etc/flowlink/endpoints.json`:

```json
[
  {
    "name": "flowmasters",
    "url": "https://flow-masters.ru",
    "interval": 60,
    "alerts": true
  },
  {
    "name": "api",
    "url": "https://api.flow-masters.ru/health",
    "interval": 30,
    "alerts": true
  }
]
```

## Status Codes

| Code Range | Status | Alert |
|------------|--------|-------|
| 200-299 | ✅ UP | None |
| 300-399 | ✅ UP (redirect) | None |
| 400-499 | ⚠️ WARNING | Check needed |
| 500-599 | ❌ DOWN | Immediate alert |
| 000 | ❌ DOWN (timeout) | Immediate alert |

## Logs

Check history stored in `/var/log/flowlink/uptime.log`:
```
2024-01-15 14:30:00 flowmasters https://flow-masters.ru 200 45ms UP
2024-01-15 14:31:00 flowmasters https://flow-masters.ru 502 1023ms DOWN
```

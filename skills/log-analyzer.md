---
name: Log Analyzer
version: 0.1.0
description: AI-powered log analysis for system logs, Docker logs, and nginx logs.
icon: 📋
commands:
  - name: log_tail
    description: Get last N lines from a log file
    run: tail -n {lines} {logfile}
    timeout: 10
    args:
      - name: logfile
        required: true
        description: Path to log file
      - name: lines
        required: false
        description: Number of lines (default 100)
        default: "100"
  - name: log_search
    description: Search log file for pattern
    run: grep -E "{pattern}" {logfile} | tail -n {lines}
    timeout: 15
    args:
      - name: logfile
        required: true
        description: Path to log file
      - name: pattern
        required: true
        description: Regex pattern to search
      - name: lines
        required: false
        description: Max results (default 50)
        default: "50"
  - name: log_stats
    description: Count error types and frequency
    run: |
      echo "=== Error Counts ===" && \
      grep -c "ERROR" {logfile} 2>/dev/null || echo "0" && \
      echo "=== Warning Counts ===" && \
      grep -c "WARN" {logfile} 2>/dev/null || echo "0" && \
      echo "=== Top 10 Error Patterns ===" && \
      grep "ERROR" {logfile} 2>/dev/null | sed 's/.*ERROR: //' | sort | uniq -c | sort -rn | head -10
    timeout: 20
    args:
      - name: logfile
        required: true
        description: Path to log file
  - name: log_anomalies
    description: Detect unusual patterns (spikes, rare events)
    run: |
      echo "=== Failed Login Attempts ===" && \
      grep "Failed password" {logfile} 2>/dev/null | awk '{print $(NF-3)}' | sort | uniq -c | sort -rn | head -5 && \
      echo "=== Error Spike Detection ===" && \
      grep -E "$(date +%b)\s+$(date +%d)" {logfile} 2>/dev/null | grep -c "ERROR" && \
      echo "=== Unusual HTTP Status Codes ===" && \
      grep -E " (4[0-9]{2}|5[0-9]{2}) " {logfile} 2>/dev/null | awk '{print $9}' | sort | uniq -c | sort -rn | head -5
    timeout: 30
    args:
      - name: logfile
        required: true
        description: Path to log file
  - name: log_nginx_access
    description: Analyze nginx access log
    run: tail -n {lines} /var/log/nginx/access.log
    timeout: 10
    args:
      - name: lines
        required: false
        default: "100"
  - name: log_nginx_error
    description: Analyze nginx error log
    run: tail -n {lines} /var/log/nginx/error.log
    timeout: 10
    args:
      - name: lines
        required: false
        default: "100"
  - name: log_auth
    description: Analyze authentication log
    run: tail -n {lines} /var/log/auth.log
    timeout: 10
    args:
      - name: lines
        required: false
        default: "100"
  - name: log_syslog
    description: Analyze system log
    run: tail -n {lines} /var/log/syslog
    timeout: 10
    args:
      - name: lines
        required: false
        default: "100"
  - name: log_docker
    description: Get Docker container logs
    run: docker logs --tail {lines} {container} 2>&1
    timeout: 15
    args:
      - name: container
        required: true
        description: Container name
      - name: lines
        required: false
        default: "100"
log_sources:
  - path: /var/log/syslog
    type: system
    description: System messages and events
  - path: /var/log/auth.log
    type: auth
    description: Authentication events
  - path: /var/log/nginx/access.log
    type: nginx_access
    description: HTTP access log
  - path: /var/log/nginx/error.log
    type: nginx_error
    description: Nginx errors
---

# Log Analyzer

AI-powered log analysis skill for detecting issues, anomalies, and security threats.

## Supported Log Types

| Log Type | Path | Description |
|----------|------|-------------|
| System | /var/log/syslog | Kernel, services, system events |
| Auth | /var/log/auth.log | SSH, sudo, login attempts |
| Nginx Access | /var/log/nginx/access.log | HTTP requests |
| Nginx Error | /var/log/nginx/error.log | Web server errors |
| Docker | docker logs | Container output |

## Analysis Features

### 1. Tail Logs
Quick view of recent log entries:
```bash
log_tail logfile=/var/log/syslog lines=200
```

### 2. Pattern Search
Search with regex patterns:
```bash
log_search logfile=/var/log/auth.log pattern="Failed password.*from"
```

### 3. Statistics
Count errors, warnings, and identify top error patterns:
```bash
log_stats logfile=/var/log/syslog
```

Output:
- Error count
- Warning count
- Top 10 recurring error messages

### 4. Anomaly Detection
Detect security threats and unusual activity:
```bash
log_anomalies logfile=/var/log/auth.log
```

Detects:
- **Failed login spikes** — Brute force attempts
- **Error spikes** — Sudden increase in errors
- **Unusual HTTP codes** — 4xx/5xx patterns

## Usage Examples

```bash
# Check recent SSH login attempts
log_auth lines=50

# Search for errors in syslog
log_search logfile=/var/log/syslog pattern="error|fail|critical"

# Analyze nginx access patterns
log_nginx_access lines=500

# Check Docker container logs
log_docker container=api-server lines=200
```

## Output Format

Logs are formatted as tables with:
- **Timestamp** — When the event occurred
- **Level** — INFO, WARN, ERROR, CRITICAL
- **Message** — Event description

## Security Monitoring

The skill automatically detects:
- Brute force SSH attempts (5+ failed logins from same IP)
- Unusual sudo usage
- Port scanning patterns
- HTTP 4xx/5xx spikes

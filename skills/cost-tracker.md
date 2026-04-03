---
name: Cost Tracker
version: 0.1.0
description: Track server costs, resource usage, and optimization recommendations.
icon: 💰
commands:
  - name: cost_summary
    description: Monthly cost summary
    run: |
      echo "=== Monthly Cost Summary ===" && \
      echo "" && \
      echo "📊 Servers: $(cat /etc/flowlink/servers.json 2>/dev/null | jq 'length' 2>/dev/null || echo '0')" && \
      echo "" && \
      echo "💰 Fixed Costs:" && \
      cat /etc/flowlink/costs.json 2>/dev/null | jq -r '.fixed[] | "  \(.name): \(.cost) \(.currency)"' 2>/dev/null || echo "  Not configured" && \
      echo "" && \
      echo "📈 Variable Costs:" && \
      cat /etc/flowlink/costs.json 2>/dev/null | jq -r '.variable[] | "  \(.name): \(.cost) \(.currency)"' 2>/dev/null || echo "  Not configured" && \
      echo "" && \
      TOTAL=$(cat /etc/flowlink/costs.json 2>/dev/null | jq '[.fixed[].cost, .variable[].cost] | add' 2>/dev/null || echo '0') && \
      echo "💵 Total: $TOTAL RUB/month"
    timeout: 10
  - name: cost_forecast
    description: Predict costs for current month
    run: |
      DAY=$(date +%d) && \
      DAYS_IN_MONTH=$(date -d "$(date +%Y-%m-01) +1 month -1 day" +%d 2>/dev/null || echo "30") && \
      USAGE_PERCENT=$((DAY * 100 / DAYS_IN_MONTH)) && \
      echo "=== Cost Forecast ===" && \
      echo "" && \
      echo "📅 Day $DAY of $DAYS_IN_MONTH ($USAGE_PERCENT% through month)" && \
      echo "" && \
      cat /etc/flowlink/costs.json 2>/dev/null | jq -r '.fixed[] | .cost' 2>/dev/null | \
      awk -v pct=$USAGE_PERCENT '{sum+=$1} END {printf "Predicted month total: %.0f RUB\n", sum}' && \
      echo "" && \
      echo "⚠️ Over-provisioned resources may increase actual costs"
    timeout: 10
  - name: cost_servers
    description: Cost breakdown by server
    run: |
      echo "=== Cost by Server ===" && \
      cat /etc/flowlink/servers.json 2>/dev/null | \
      jq -r '.[] | "🖥️ \(.name)\n   Fixed: \(.cost_fixed) RUB\n   Resources: \(.cpu) CPU, \(.ram)GB RAM, \(.disk)GB disk\n   Utilization: \(.utilization)%\n   Status: \(.status)\n"' 2>/dev/null || \
      echo "No servers configured. Add servers to /etc/flowlink/servers.json"
    timeout: 10
  - name: cost_optimize
    description: Get optimization recommendations
    run: |
      echo "=== Optimization Recommendations ===" && \
      echo "" && \
      echo "🔍 Analyzing resource usage..." && \
      echo "" && \
      echo "💡 Recommendations:" && \
      echo "" && \
      echo "1. 📉 Under-utilized servers:" && \
      cat /etc/flowlink/servers.json 2>/dev/null | \
      jq -r '.[] | select(.utilization < 30) | "   - \(.name): \(.utilization)% CPU (consider downsizing)"' 2>/dev/null || \
      echo "   None found" && \
      echo "" && \
      echo "2. 🔥 Over-utilized servers:" && \
      cat /etc/flowlink/servers.json 2>/dev/null | \
      jq -r '.[] | select(.utilization > 80) | "   - \(.name): \(.utilization)% CPU (consider upgrading)"' 2>/dev/null || \
      echo "   None found" && \
      echo "" && \
      echo "3. 💾 Storage optimization:" && \
      echo "   - Check for unused Docker images: docker image prune -a" && \
      echo "   - Remove old logs: journalctl --vacuum-time=7d" && \
      echo "   - Clean package cache: apt clean" && \
      echo "" && \
      echo "4. 📊 Potential savings:" && \
      cat /etc/flowlink/servers.json 2>/dev/null | \
      jq -r '[.[] | select(.utilization < 30) | .cost_fixed] | add * 0.3' 2>/dev/null | \
      awk '{printf "   Downsizing could save ~%.0f RUB/month\n", $1}' || \
      echo "   Run cost analysis first"
    timeout: 15
  - name: cost_resource_usage
    description: Get current resource usage
    run: |
      echo "=== Current Resource Usage ===" && \
      echo "" && \
      echo "🖥️ CPU:" && \
      top -bn1 | grep "Cpu(s)" | awk '{print "   Usage: " 100-$8 "%"}' && \
      echo "" && \
      echo "💾 Memory:" && \
      free -h | awk '/Mem:/ {print "   Used: " $3 " / " $2 " (" $3/$2*100 "%)"}' && \
      echo "" && \
      echo "💿 Disk:" && \
      df -h / | awk 'NR==2 {print "   Used: " $3 " / " $2 " (" $5 ")"}' && \
      echo "" && \
      echo "🐳 Docker:" && \
      docker ps --format "{{.Names}}" | wc -l | xargs echo "   Containers running:" && \
      docker images | wc -l | xargs echo "   Images:"
    timeout: 10
  - name: cost_add_server
    description: Add server to tracking
    run: |
      mkdir -p /etc/flowlink && \
      cat /etc/flowlink/servers.json 2>/dev/null || echo '[]' > /etc/flowlink/servers.json && \
      jq '. += [{"name": "{name}", "host": "{host}", "cost_fixed": {cost}, "cpu": {cpu}, "ram": {ram}, "disk": {disk}, "utilization": 0, "status": "active"}]' \
      /etc/flowlink/servers.json > /tmp/servers.json && \
      mv /tmp/servers.json /etc/flowlink/servers.json && \
      echo "Added server {name}"
    timeout: 5
    args:
      - name: name
        required: true
        description: Server name
      - name: host
        required: true
        description: Server hostname or IP
      - name: cost
        required: true
        description: Monthly fixed cost in RUB
      - name: cpu
        required: false
        description: CPU cores
        default: "2"
      - name: ram
        required: false
        description: RAM in GB
        default: "4"
      - name: disk
        required: false
        description: Disk in GB
        default: "50"
  - name: cost_set_fixed
    description: Set fixed monthly costs
    run: |
      mkdir -p /etc/flowlink && \
      echo '{"fixed": [{"name": "VPS Primary", "cost": {vps_cost}, "currency": "RUB"}], "variable": []}' > /etc/flowlink/costs.json && \
      echo "Fixed costs updated"
    timeout: 5
    args:
      - name: vps_cost
        required: true
        description: Primary VPS monthly cost
reporting:
  schedule: weekly
  day: monday
  time: "09:00"
  channel: telegram
  format: |
    💰 Weekly Cost Report
    
    📊 This Month: {monthly_cost} RUB
    📈 Forecast: {forecast} RUB
    💡 Potential Savings: {savings} RUB
    
    Top Recommendations:
    {recommendations}
---

# Cost Tracker

Server cost tracking and optimization recommendations.

## Features

### Cost Tracking
- **Monthly summary** — Total fixed and variable costs
- **Per-server breakdown** — Cost by individual server
- **Forecasting** — Predict end-of-month costs

### Resource Monitoring
- **CPU usage** — Current utilization
- **Memory usage** — RAM consumption
- **Disk usage** — Storage consumption
- **Docker containers** — Running services count

### Optimization
- **Under-utilization detection** — Servers using < 30% resources
- **Over-utilization alerts** — Servers using > 80% resources
- **Savings calculator** — Potential monthly savings

## Usage Examples

### View Costs

```bash
# Monthly summary
cost_summary

# Cost by server
cost_servers

# Monthly forecast
cost_forecast
```

### Resource Usage

```bash
# Current usage
cost_resource_usage
```

### Optimization

```bash
# Get recommendations
cost_optimize
```

### Configuration

```bash
# Add server
cost_add_server name=flowmasters host=93.93.207.44 cost=500 cpu=2 ram=4 disk=50

# Set fixed costs
cost_set_fixed vps_cost=500
```

## Configuration Files

### Servers (`/etc/flowlink/servers.json`)

```json
[
  {
    "name": "flowmasters",
    "host": "93.93.207.44",
    "cost_fixed": 500,
    "cpu": 2,
    "ram": 4,
    "disk": 50,
    "utilization": 45,
    "status": "active"
  }
]
```

### Costs (`/etc/flowlink/costs.json`)

```json
{
  "fixed": [
    {"name": "VPS Primary", "cost": 500, "currency": "RUB"}
  ],
  "variable": [
    {"name": "Traffic overage", "cost": 0, "currency": "RUB"}
  ]
}
```

## Weekly Report

Every Monday at 09:00 MSK, a report is sent to Telegram:

```
💰 Weekly Cost Report

📊 This Month: 500 RUB
📈 Forecast: 500 RUB
💡 Potential Savings: 150 RUB

Top Recommendations:
• flowmasters: 25% CPU (downsize to save 150 RUB)
• Clean up 2GB unused Docker images
```

## Optimization Rules

| Utilization | Status | Action |
|-------------|--------|--------|
| < 30% | 📉 Under-utilized | Downsize |
| 30-80% | ✅ Optimal | No action |
| > 80% | 🔥 Over-utilized | Upgrade |

## Cost Categories

1. **Fixed Costs** — Monthly VPS fees, reserved instances
2. **Variable Costs** — Traffic overage, additional storage
3. **Hidden Costs** — Backup storage, CDN, monitoring

## Integration with Uptime Monitor

The cost tracker integrates with `uptime-monitor` skill:
- Uses uptime data for availability metrics
- Correlates downtime with cost impact
- Calculates cost-per-uptime percentage

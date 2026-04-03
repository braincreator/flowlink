---
name: Docker Manager
version: 0.1.0
description: Deploy, restart, scale containers. Health checks and auto-healing.
icon: 🐳
commands:
  - name: docker_ps
    description: List running containers
    run: docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
    timeout: 10
  - name: docker_logs
    description: Get container logs
    run: docker logs --tail 100 {container}
    timeout: 15
    args:
      - name: container
        required: true
        description: Container name
  - name: docker_restart
    description: Restart a container
    run: docker restart {container}
    timeout: 30
    args:
      - name: container
        required: true
        description: Container name
  - name: docker_stop
    description: Stop a container
    run: docker stop {container}
    timeout: 20
    args:
      - name: container
        required: true
        description: Container name
  - name: docker_start
    description: Start a container
    run: docker start {container}
    timeout: 20
    args:
      - name: container
        required: true
        description: Container name
  - name: docker_stats
    description: Show resource usage (CPU, memory, network, disk)
    run: docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.BlockIO}}"
    timeout: 15
  - name: docker_images
    description: List all Docker images
    run: docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"
    timeout: 10
  - name: docker_prune
    description: Remove unused images, containers, and networks
    run: docker system prune -f
    timeout: 60
  - name: docker_exec
    description: Execute command in container
    run: docker exec {container} {command}
    timeout: 30
    args:
      - name: container
        required: true
        description: Container name
      - name: command
        required: true
        description: Command to execute
health_checks:
  - name: container_running
    description: Check if container is running
    interval: 60
    run: docker inspect -f '{{.State.Running}}' {container}
    on_fail: notify
auto_healing:
  - name: restart_on_failure
    description: Restart container if health check fails 3 times
    trigger: container_running
    failures_threshold: 3
    action: docker restart {container}
---

# Docker Manager

Comprehensive Docker container management skill for FlowLink agents.

## Features

### Container Management
- **List containers** — View all running containers with status and port mappings
- **Start/Stop/Restart** — Control container lifecycle
- **Logs** — Retrieve last N lines of container output
- **Execute commands** — Run commands inside containers

### Resource Monitoring
- **CPU/Memory usage** — Real-time resource consumption
- **Network I/O** — Bandwidth usage per container
- **Disk I/O** — Block device usage

### Cleanup
- **Prune** — Remove unused images, containers, networks
- **Image listing** — View all images with sizes

## Usage Examples

```bash
# List all running containers
docker_ps

# Get logs from nginx container
docker_logs container=nginx

# Restart a container
docker_restart container=api-server

# Check resource usage
docker_stats

# Clean up unused resources
docker_prune
```

## Health Checks

The skill monitors container health every 60 seconds. If a container stops responding, it triggers a notification to the Telegram bot.

### Auto-Healing

When enabled, containers that fail health checks 3 consecutive times are automatically restarted. This ensures high availability without manual intervention.

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| container | Yes | Container name or ID |
| command | For exec | Command to run inside container |

## Timeouts

- Quick commands (ps, images): 10s
- Standard operations (logs, stats): 15s
- Lifecycle changes (restart, stop, start): 20-30s
- Cleanup (prune): 60s

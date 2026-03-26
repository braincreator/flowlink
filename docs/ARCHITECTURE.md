# FlowLink — Архитектура

## Обзор

FlowLink решает проблему удалённого управления машинами клиентов через AI.
Агент устанавливается одной командой, подключается к реле, и OpenClaw может
выполнять команды, читать/писать файлы, собирать системную информацию.

---

## Язык: Go

**Почему Go:**
- Статический бинарник, **ноль зависимостей** на клиенте
- Кросс-компиляция (GOOS/GOARCH) — один код для macOS, Linux, Windows
- Маленький размер (~5MB)
- Отличная стандартная библиотека (crypto, net, os/exec)
- gorilla/websocket для WSS
- Эффективные горутины для конкурентности
- Быстрая компиляция

**Почему не Rust:**
- Длиннее разработка (borrow checker, lifetime)
- Избыточно для сетевого демона
- Rust идеален для CPU-bound, Go — для I/O-bound (наш случай)

**Почему не Python:**
- Требует интерпретатор на клиенте
- Нельзя упаковать в один бинарник без боли (PyInstaller)
- Медленнее для I/O

---

## Транспорт

### Почему WSS (WebSocket Secure), а не SSH:

| Критерий | WSS | SSH | HTTP Polling |
|----------|-----|-----|-------------|
| Пробивает NAT | ✅ (outbound) | ❌ (inbound) | ✅ |
| Бидирекциональный | ✅ | ✅ | ❌ |
| Реалтайм | ✅ | ✅ | ❌ (latency) |
| Простота клиента | ✅ | ❌ (сервер) | ✅ |
| TLS | ✅ | ✅ | ✅ |
| Proxy-friendly | ✅ | 🟡 | ✅ |

**WSS outbound** = клиент подключается ИЗ машины к реле. NAT не проблема.

---

## Протокол

### Формат сообщений

Все сообщения — JSON через WSS:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "type": "exec_request",
  "agent_id": "abc123def456",
  "session_id": "",
  "payload": { ... },
  "timestamp": 1712345678,
  "error": ""
}
```

### Жизненный цикл

```
1. Агент → Реле:    connect     {agent_id, token, hostname, os, arch}
2. Реле → Агент:    connected   {agent_id, relay_id, heartbeat_interval}
3. Агент → Реле:    heartbeat   (каждые 30 сек)
4. Реле → Агент:    heartbeat_ack
5. OpenClaw → Реле: HTTP POST /api/v1/agents/exec {agent_id, command}
6. Реле → Агент:    exec_request {command, timeout, dir, env}
7. Агент → Реле:    needs_approval {command, risk} (если опасная команда)
8. Клиент (TTY):    "Выполнить? [y/N]" → Y
9. Агент → Реле:    exec_approve
10. Агент → Реле:   exec_output {stdout chunk}
11. Агент → Реле:   exec_done {exit_code, duration}
12. Реле → OpenClaw: HTTP response
```

---

## Безопасность

### Модель угроз

| Угроза | Защита |
|--------|--------|
| Перехват трафика | TLS (WSS) |
| Подмена агента | Pairwise токены |
| Подмена реле | Certificate pinning (TODO) |
| Опасные команды | Sandbox + Approval |
| Чтение файлов | Directory whitelist |
| DoS (длинная команда) | Timeout + max output size |
| Повторная атака | Nonce в сообщениях (TODO) |

### Слой 1: Аутентификация

- Каждый агент генерирует уникальный `agent_id` + `token` при `--init`
- Токен проверяется реле при подключении
- HTTP API защищён Bearer токеном (`FLOWLINK_API_TOKEN`)

### Слой 2: Sandbox

- **Blocked patterns:** `rm -rf /*`, `mkfs*`, fork bomb, `dd if=*`
- **AllowSudo:** по умолчанию false
- **MaxFileSize:** 100MB для файловых операций
- **MaxExecTimeout:** 5 минут
- **AllowedDirs:** ограничение доступа к директориям

### Слой 3: Approval

Три режима:
- `auto` — всё выполняется без спроса
- `ask` (default) — опасные команды требуют Y/N в терминале клиента
- `deny` — ничего не выполняется без разрешения

Оценка риска:
- **high:** `rm -rf`, `sudo`, `shutdown`, `curl|sh`, `chmod 777`
- **medium:** `rm`, `chmod`, `systemctl`, `iptables`
- **low:** всё остальное

---

## Конфигурация

### Агент (`~/.flowlink/config.json`)

```json
{
  "agent_id": "abc123...",
  "token": "def456...",
  "relay_url": "wss://relay.flowmasters.ru/ws",
  "label": "MacBook Саня",
  "heartbeat_sec": 30,
  "approval": {
    "mode": "ask",
    "dangerous_patterns": ["rm *", "sudo*", ...],
    "auto_approve_patterns": ["ls*", "cat*", "pwd", ...]
  },
  "sandbox": {
    "allowed_dirs": [],
    "blocked_patterns": ["rm -rf /*", ...],
    "max_file_size": 104857600,
    "max_exec_timeout": 300,
    "allow_sudo": false
  }
}
```

### Реле (`relay.json`)

```json
{
  "wss_addr": ":8443",
  "api_addr": ":8080",
  "api_token": "your-secret-token",
  "heartbeat_timeout_sec": 90,
  "max_agents": 100,
  "allowed_tokens": {
    "client-token-1": "agent-id-1",
    "client-token-2": ""
  }
}
```

---

## Деплой

### Реле на VPS (Timeweb, 477₽/мес)

```bash
# 1. Собрать
make build-relay

# 2. Скопировать
scp bin/flowlink-relay root@vps:~/

# 3. Запустить с systemd
ssh root@vps << 'EOF'
cat > /etc/systemd/system/flowlink-relay.service << UNIT
[Unit]
Description=FlowLink Relay
After=network.target

[Service]
Type=simple
ExecStart=/root/flowlink-relay -api-token YOUR_TOKEN
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable flowlink-relay
systemctl start flowlink-relay
EOF

# 4. SSL через Caddy (автоматический)
# Caddyfile:
# relay.flowmasters.ru {
#     reverse_proxy /ws localhost:8443
#     reverse_proxy /api localhost:8080
# }
```

### Агент у клиента

```bash
# Одна команда
curl -sSL https://install.flowmasters.ru | bash

# Или сборка из source
go install github.com/braincreator/flowlink/cmd/agent@latest
flowlink --init --relay wss://relay.flowmasters.ru/ws
flowlink agent start
```

---

## OpenClaw Integration

### HTTP API (через exec curl)

```bash
# Список агентов
API_TOKEN="your-token" RELAY="http://relay.flowmasters.ru:8080"

# GET /api/v1/agents
curl -sH "Authorization: Bearer $API_TOKEN" $RELAY/api/v1/agents | jq

# POST /api/v1/agents/exec
curl -sX POST -H "Authorization: Bearer $API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"agent_id":"abc","command":"docker ps"}' \
  $RELAY/api/v1/agents/exec | jq

# POST /api/v1/agents/sysinfo
curl -sX POST -H "Authorization: Bearer $API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"agent_id":"abc"}' \
  $RELAY/api/v1/agents/sysinfo | jq
```

### OpenClaw Skill (планируется)

```
flowlink list                    → список агентов
flowlink exec <agent> <command>  → выполнить команду
flowlink read <agent> <path>     → прочитать файл
flowlink write <agent> <path>    → записать файл
flowlink info <agent>            → системная информация
```

---

## Roadmap

### MVP (v0.1) — сейчас
- [x] Протокол (JSON/WSS)
- [x] Агент (connect, exec, files, sysinfo)
- [x] Реле (WSS + HTTP API)
- [x] Sandbox + Approval
- [x] Install script
- [x] Кросс-компиляция

### v0.2
- [ ] Event streaming (SSE для real-time вывода команд)
- [ ] Файловый трансфер (upload/download больших файлов)
- [ ] TLS certificate pinning
- [ ] Systemd/LaunchAgent автозапуск

### v0.3
- [ ] OpenClaw skill (flowlink)
- [ ] Web UI для мониторинга агентов
- [ ] Multi-relay (балансировка)
- [ ] Audit log

### v1.0
- [ ] End-to-end шифрование (E2EE)
- [ ] Групповые команды (на несколько агентов)
- [ ] Remote shell (интерактивный терминал)
- [ ] Мобильный companion app

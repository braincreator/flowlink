# FlowLink

> Легковесный удалённый агент для AI-управления машиной клиента

**Одна команда на клиенте — полный контроль через AI.**

```
curl -sSL https://install.flowmasters.ru | bash
```

---

## Что это

FlowLink — это бинарник (~5MB), который устанавливается на машину клиента и подключается к вашему реле-серверу. После этого вы (через OpenClaw) можете:

- 🖥️ **Выполнять команды** — shell exec на машине клиента
- 📁 **Работать с файлами** — читать, писать, листать директории
- 📊 **Собирать системную информацию** — CPU, RAM, диск, uptime
- ✅ **Approval-модель** — клиент подтверждает опасные операции

## Архитектура

```
                    ┌─────────────────┐
                    │  Relay Server   │
                    │  (VPS 477₽/мес) │
                    │  :8443 WSS      │
                    │  :8080 HTTP API │
                    └────────┬────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
 ┌────────┴───────┐ ┌──────┴───────┐ ┌────────┴───────┐
 │  Client A      │ │  Client B    │ │  Client C      │
 │  flowlink      │ │  flowlink    │ │  flowlink      │
 │  macOS         │ │  Windows     │ │  Linux VPS     │
 │  (5MB binary)  │ │  (5MB exe)   │ │  (5MB binary)  │
 └────────────────┘ └──────────────┘ └────────────────┘
          ↑                  ↑                  ↑
          └──────────────────┼──────────────────┘
                             │
                    ┌────────┴────────┐
                    │  OpenClaw       │
                    │  (оператор)     │
                    │  HTTP API calls │
                    └─────────────────┘
```

### Компоненты

| Компонент | Язык | Размер | Назначение |
|-----------|------|--------|------------|
| `flowlink` | Go | ~5MB | Агент на машине клиента |
| `flowlink-relay` | Go | ~8MB | Реле-сервер на VPS |
| OpenClaw skill | — | — | Управление агентами из OpenClaw |

### Протокол

JSON-сообщения через WSS (outbound от агента → пробивает NAT):

```json
{
  "id": "uuid",
  "type": "exec_request",
  "agent_id": "abc123",
  "payload": { "command": "ls -la", "timeout_sec": 30 },
  "timestamp": 1712345678
}
```

Типы сообщений: `connect`, `heartbeat`, `exec_request/response`, `file_read/write/list`, `sys_info`, `needs_approval`

## Безопасность

- 🔑 **Pairwise токены** — уникальный токен для каждого агента
- 🔒 **TLS (WSS)** — шифрование всего трафика
- 🛡️ **Sandbox** — блокировка опасных команд (rm -rf, sudo, fork bomb)
- ✅ **Approval** — клиент подтверждает операции в терминале
- 📁 **File whitelist** — ограничение доступа к директориям
- ⏱️ **Timeout** — ограничение времени выполнения команд

## Установка

### Для оператора (вы)

1. Соберите бинарники:
```bash
make build
```

2. Деплой реле на VPS:
```bash
scp bin/flowlink-relay server:~/
ssh server "FLOWLINK_API_TOKEN=your-secret-token ./flowlink-relay"
```

3. Используйте HTTP API из OpenClaw:
```bash
# Список агентов
curl -H "Authorization: Bearer your-secret-token" \
  http://relay:8080/api/v1/agents

# Выполнить команду
curl -X POST -H "Authorization: Bearer your-secret-token" \
  -d '{"agent_id":"abc","command":"ls -la"}' \
  http://relay:8080/api/v1/agents/exec
```

### Для клиента

```bash
# Установка одной командой
curl -sSL https://install.flowmasters.ru | bash

# Или вручную:
flowlink --init --relay wss://relay.flowmasters.ru/ws --label "MacBook Саня"
flowlink agent start
```

## Разработка

```bash
# Зависимости
make deps

# Сборка
make build

# Запуск реле (dev)
make run-relay

# Запуск агента (dev, подключится к реле)
FLOWLINK_RELAY=ws://localhost:8443/ws make run-agent

# Тесты
make test

# Линт
make lint

# Кросс-компиляция
make build-release
```

## API Endpoints

| Endpoint | Метод | Описание |
|----------|-------|----------|
| `/api/v1/agents` | GET | Список подключённых агентов |
| `/api/v1/agents/exec` | POST | Выполнить команду на агенте |
| `/api/v1/agents/files/read` | POST | Прочитать файл |
| `/api/v1/agents/files/write` | POST | Записать файл |
| `/api/v1/agents/files/list` | POST | Список файлов |
| `/api/v1/agents/sysinfo` | POST | Системная информация |

## Лицензия

Private — Flow Masters © 2026

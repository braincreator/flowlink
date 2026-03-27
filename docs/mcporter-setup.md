# Подключение flowlink к OpenClaw через mcporter

## Установка

```bash
# Добавить MCP сервер
mcporter config add flowlink https://relay.example.com/mcp \
  --header "Authorization: Bearer YOUR_TOKEN"

# Или через конфиг файл (~/.config/mcporter/config.yaml):
servers:
  flowlink:
    url: https://relay.example.com/mcp
    headers:
      Authorization: "Bearer YOUR_TOKEN"
    transport: streamable-http
```

## Проверка

```bash
# Список инструментов
mcporter list flowlink

# Вызвать инструмент
mcporter call flowlink.flowlink_agents
mcporter call flowlink.flowlink_exec '{"agent": "my-server", "command": "ls -la"}'
mcporter call flowlink.flowlink_sysinfo '{"agent": "my-server"}'
```

## Доступные инструменты

| Инструмент | Описание | Обязательные параметры |
|---|---|---|
| `flowlink_agents` | Список подключённых агентов | — |
| `flowlink_exec` | Выполнить shell-команду | `agent`, `command` |
| `flowlink_read` | Прочитать файл | `agent`, `path` |
| `flowlink_write` | Записать файл | `agent`, `path`, `content` |
| `flowlink_list` | Список файлов/директорий | `agent`, `path` |
| `flowlink_sysinfo` | Системная информация | `agent` |
| `flowlink_task` | Запустить автономную задачу | `agent`, `description` |
| `flowlink_task_status` | Статус задачи | `agent`, `task_id` |

## Примеры

### Получить список агентов

```json
{
  "name": "flowlink_agents",
  "arguments": { "status": "online" }
}
```

### Выполнить команду

```json
{
  "name": "flowlink_exec",
  "arguments": {
    "agent": "production-server",
    "command": "docker ps --format 'table {{.Names}}\t{{.Status}}'",
    "timeout": 30
  }
}
```

### Прочитать лог

```json
{
  "name": "flowlink_read",
  "arguments": {
    "agent": "production-server",
    "path": "/var/log/app.log"
  }
}
```

### Запустить задачу

```json
{
  "name": "flowlink_task",
  "arguments": {
    "agent": "production-server",
    "description": "Обновить зависимости и перезапустить сервис",
    "max_steps": 10
  }
}
```

## MCP Protocol

- **Transport:** Streamable HTTP (POST + GET для SSE)
- **Protocol Version:** 2024-11-05
- **Auth:** Bearer token в заголовке `Authorization` или query parameter `token`

## Тестирование

```bash
# Юнит-тесты MCP
go test ./internal/relay/ -run TestMCP -v

# Интеграционные тесты (запускают relay + mock agent)
go test ./test/integration/ -v

# Все тесты
go test ./... -count=1
```

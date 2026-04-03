# flowlink

> Open-source self-hosted серверное управление. Альтернатива FleetDeck и Dexposure.

[![Go](https://img.shields.io/badge/Go-1.24-00ADD8?logo=go)](https://go.dev)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker)](https://hub.docker.com/r/flowlink/relay)
[![Release](https://img.shields.io/github/v/release/braincreator/flowlink?color=blue)](https://github.com/braincreator/flowlink/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/braincreator/flowlink/ci.yml?branch=main&label=CI)](https://github.com/braincreator/flowlink/actions)

**FlowLink** — SaaS-платформа удалённого управления серверами через AI. Клиент устанавливает один бинарник (~5MB) — вы управляете его инфраструктурой через OpenClaw или Telegram.

🔗 **Developed by [FlowMasters](https://flow-masters.ru)** — чат-боты, ИИ-ассистенты и автоматизация для бизнеса.

---

## 🚀 Быстрый старт

### Установка одним скриптом

```bash
# На сервере клиента (Linux / macOS)
curl -sSL https://install.flowlink.dev | bash
```

Скрипт автоматически:
- Скачает бинарник для вашей платформы
- Создаст systemd service (Linux) или LaunchAgent (macOS)
- Запустит агента и подключит к реле

### Docker

```bash
# Запуск реле
docker run -d \
  --name flowlink-relay \
  -p 8443:8443 -p 8080:8080 \
  -v flowlink-data:/var/lib/flowlink \
  -e FLOWLINK_API_TOKEN=your-secret-token \
  flowlink/relay:latest
```

### Ручная установка

```bash
# Сборка из исходников
git clone https://github.com/braincreator/flowlink.git
cd flowlink
make build

# Реле (на VPS)
./bin/flowlink-relay -config relay.yaml

# Агент (на клиенте)
./bin/flowlink -config agent.yaml
```

---

## 📡 Архитектура

```
┌─────────────────────────────────────────────────────────────┐
│                    Клиентская машина                         │
│  ┌──────────────┐                                            │
│  │ flowlink     │ ←── единственный бинарник, ноль зависимостей│
│  │ (демон)      │ ←── sandbox, backup, approval, kill switch │
│  └──────┬───────┘                                            │
└─────────┼───────────────────────────────────────────────────┘
          │ WSS (outbound, пробивает NAT)
          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Реле (VPS)                                │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ WSS      │  │ MCP      │  │ API      │  │ Billing  │    │
│  │ Server   │  │ Server   │  │ Gateway  │  │ Module   │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │ Auth     │  │ Audit    │  │ Agent    │                  │
│  │ Module   │  │ Log      │  │ Registry │                  │
│  └──────────┘  └──────────┘  └──────────┘                  │
└─────────┬───────────────────────────────────────────────────┘
          │ MCP (Streamable HTTP)
          ▼
┌─────────────────────────────────────────────────────────────┐
│                 Оператор (OpenClaw)                          │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │ OpenClaw     │  │ mcporter     │                         │
│  │ (мозг)       │  │ (MCP client) │                         │
│  └──────────────┘  └──────────────┘                         │
└─────────────────────────────────────────────────────────────┘
          │ HTTP API
          ▼
┌─────────────────────────────────────────────────────────────┐
│  Telegram Bot  │  Web Dashboard  │  Let's Encrypt (TLS)     │
└─────────────────────────────────────────────────────────────┘
```

**Компоненты:**

| Компонент | Назначение |
|-----------|------------|
| **Relay** | Центральный сервер на VPS. Принимает WSS от агентов, предоставляет HTTP API и MCP для OpenClaw |
| **Agent** | Лёгкий демон на клиенте. Выполняет команды, управляет файлами, бэкапами, kill switch |
| **MCP Server** | Интеграция с OpenClaw через JSON-RPC. 8 инструментов для управления агентами |
| **Telegram Bot** | Управление через Telegram: статусы, команды, approval, emergency stop |
| **Web Dashboard** | SPA-панель с dark theme для мониторинга и управления |

---

## ✨ Возможности

- 🖥️ **Удалённое выполнение команд** — shell exec с sandbox и таймаутами
- 📁 **Файловый менеджер** — чтение, запись, листинг директорий
- 💾 **Бэкапы и восстановление** — снапшоты с retention и лимитами
- 🛑 **Kill Switch** — emergency stop, pause, readonly mode
- ✅ **Approval system** — 3 режима: auto / soft_ask / hard_ask
- 📊 **Audit log** — JSONL-логирование каждого действия с экспортом
- 🔌 **MCP Server** — интеграция с OpenClaw (8 инструментов)
- 🤖 **Telegram Bot** — 15 команд для управления через Telegram
- 🖥️ **Web Dashboard** — SPA с dark theme
- 💰 **Billing** — 4 тарифных плана, SaaS-ready
- 🔒 **TLS** — self-signed + Let's Encrypt
- 📡 **Event Streaming** — SSE для real-time уведомлений
- 🏢 **Multi-tenancy** — несколько клиентов на одном реле

---

## 🏗️ Компоненты

### flowlink-relay

Реле-сервер — центральный узел архитектуры.

| Параметр | Значение |
|----------|----------|
| Порт WSS | `:8443` |
| Порт HTTP API | `:8080` |
| MCP endpoint | `POST /mcp` |
| Dashboard | `/dashboard/` |

**Запуск:**
```bash
flowlink-relay -config relay.yaml
```

### flowlink (agent)

Лёгкий агент для клиентских машин.

**Установка:**
```bash
curl -sSL https://install.flowlink.dev | bash
# или
flowlink -install
```

**Команды:**
```bash
flowlink -install          # Установить как сервис
flowlink -uninstall        # Удалить сервис
flowlink -config path.yaml # Указать конфиг
flowlink -version          # Версия
```

### flowlink-bot (Telegram)

Управление агентами через Telegram.

| Команда | Описание |
|---------|----------|
| `/start` | Приветствие и привязка |
| `/help` | Список команд |
| `/status` | Статус агентов |
| `/servers` | Список серверов |
| `/exec <cmd>` | Выполнить команду |
| `/logs` | Последние логи |
| `/backups` | Список бэкапов |
| `/restore <id>` | Восстановить бэкап |
| `/emergency` | Emergency stop |
| `/pause` | Пауза агентов |
| `/resume` | Возобновить работу |
| `/approve <id>` | Подтвердить операцию |
| `/reject <id>` | Отклонить операцию |
| `/settings` | Настройки |

---

## 🔧 Конфигурация

### Agent — `~/.flowlink/config.yaml`

```yaml
# Идентификация
agent_id: "auto-generated"
token: "pairwise-token-from-relay"
relay_url: "wss://relay.example.com/ws"

# Настройки
heartbeat_sec: 30
label: "production-server-1"
work_dir: "/home/deploy"

# Sandbox — ограничения
sandbox:
  allowed_dirs:
    - "/home/deploy"
    - "/var/www"
  blocked_patterns:
    - "rm -rf /*"
    - "mkfs.*"
    - "dd if=*"
    - ":(){ :|:& };:"
  max_file_size: 104857600   # 100 MB
  max_exec_timeout: 300       # 5 мин
  allow_sudo: false

# Approval — подтверждение команд
approval:
  mode: "soft_ask"            # auto | soft_ask | hard_ask
  soft_ask_notify: true
  hard_ask_timeout_sec: 3600
  max_retries: 3

# Backup — резервное копирование
backup:
  enabled: true
  max_snapshots: 50
  max_total_size: 5368709120  # 5 GB
  retention_days: 7
  backup_dir: "~/.flowlink/backups"
```

### Relay — `relay.yaml`

```yaml
# Слушатели
wss_addr: ":8443"
api_addr: ":8080"

# TLS
tls_mode: "letsencrypt"       # self-signed | letsencrypt | manual
tls_domain: "relay.example.com"
tls_cache: "/var/lib/flowlink/tls-cache"

# Авторизация
api_token: "your-secret-api-token"

# Multi-tenancy
data_dir: "/var/lib/flowlink"

# Rate limiting
rate_limit_rpm: 60            # запросов в минуту
```

---

## 🔐 Безопасность

| Механизм | Описание |
|----------|----------|
| **JWT авторизация** | Pairwise токены для каждого агента, JWT для API |
| **TLS** | Шифрование всего трафика (self-signed / Let's Encrypt / manual) |
| **Rate limiting** | Ограничение запросов к API (настраиваемое) |
| **Sandbox** | Блокировка опасных команд (rm -rf, fork bombs, mkfs) |
| **File whitelist** | Ограничение доступа к директориям |
| **Approval system** | 3 режима подтверждения команд |
| **Audit log** | JSONL-логирование каждого действия |
| **Timeout** | Ограничение времени выполнения команд |

---

## 💰 Тарифные планы

| План | Цена/мес | Агенты | Команды/мес | Бэкапы | Хранилище | Фичи |
|------|---------|--------|-------------|---------|-----------|------|
| **Free** | 0 ₽ | 1 | 100 | 3 | 100 MB | Базовое выполнение |
| **Starter** | 990 ₽ | 3 | 1 000 | 10 | 1 GB | Telegram Bot, Audit |
| **Business** | 4 990 ₽ | 25 | 10 000 | 50 | 10 GB | + MCP, API, Dashboard |
| **Enterprise** | 19 990 ₽ | 100 | Безлимит | Безлимит | 100 GB | Все фичи, white-label |

---

## 📖 API Reference

### Авторизация

| Метод | Endpoint | Описание |
|-------|----------|----------|
| POST | `/api/v1/auth/login` | Авторизация, получение JWT |
| POST | `/api/v1/auth/refresh` | Обновление токена |

### Агенты

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/agents` | Список подключённых агентов |
| POST | `/api/v1/agents/register` | Регистрация нового агента |
| DELETE | `/api/v1/agents/delete/{id}` | Удаление агента |
| POST | `/api/v1/agents/exec` | Выполнить команду на агенте |
| GET | `/api/v1/agents/files/read` | Прочитать файл |
| POST | `/api/v1/agents/files/write` | Записать файл |
| GET | `/api/v1/agents/files/list` | Список файлов |
| GET | `/api/v1/agents/sysinfo` | Системная информация |
| POST | `/api/v1/agents/task` | Отправить автономную задачу |
| POST | `/api/v1/agents/task/cancel` | Отменить задачу |
| POST | `/api/v1/agents/skills/push` | Отправить скилл |
| GET | `/api/v1/agents/skills/list` | Список скиллов |
| POST | `/api/v1/agents/skills/delete` | Удалить скилл |
| POST | `/api/v1/agents/pause` | Пауза агента |
| POST | `/api/v1/agents/resume` | Возобновить агента |

### Файлы

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/files/read?agent_id=X&path=Y` | Прочитать файл |
| POST | `/api/v1/files/write` | Записать файл |
| GET | `/api/v1/files/list?agent_id=X&dir=Y` | Листинг директории |

### Бэкапы

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/backups` | Список бэкапов |
| POST | `/api/v1/backups/{id}/restore` | Восстановить из бэкапа |

### Audit

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/audit` | Запрос логов |
| GET | `/api/v1/audit/export` | Экспорт в JSON/CSV |
| GET | `/api/v1/audit/stats` | Статистика |

### Клиенты

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/clients` | Список клиентов |
| POST | `/api/v1/clients` | Создать клиента |
| GET | `/api/v1/clients/{id}` | Информация о клиенте |
| POST | `/api/v1/clients/{id}/agents` | Добавить агента клиенту |

### Billing

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/billing/usage` | Статистика использования |
| GET | `/api/v1/billing/plan` | Текущий план |
| POST | `/api/v1/billing/plan/change` | Сменить план |
| GET | `/api/v1/billing/invoices` | Список счетов |
| POST | `/api/v1/billing/invoices/{id}/pay` | Оплатить счёт |
| GET | `/api/v1/billing/payment-methods` | Способы оплаты |

### Events

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/events` | SSE stream событий |

### MCP

| Метод | Endpoint | Описание |
|-------|----------|----------|
| POST | `/mcp` | JSON-RPC MCP endpoint |

### Dashboard

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/dashboard/` | Web Dashboard (SPA) |

---

## 🔌 MCP Integration

Подключение flowlink к OpenClaw через mcporter:

```json
{
  "mcpServers": {
    "flowlink": {
      "url": "https://relay.example.com/mcp",
      "headers": {
        "Authorization": "Bearer your-api-token"
      }
    }
  }
}
```

**Доступные инструменты:**

| Инструмент | Описание |
|-----------|----------|
| `flowlink_agents` | Список подключённых агентов |
| `flowlink_exec` | Выполнить команду на агенте |
| `flowlink_read` | Прочитать файл |
| `flowlink_write` | Записать файл |
| `flowlink_list` | Листинг директории |
| `flowlink_sysinfo` | Системная информация |
| `flowlink_task` | Отправить автономную задачу |
| `flowlink_task_status` | Статус задачи |

---

## 🐳 Docker

```yaml
# docker-compose.yml
version: '3.8'
services:
  relay:
    image: flowlink/relay:latest
    ports:
      - "8443:8443"
      - "8080:8080"
    environment:
      - FLOWLINK_API_TOKEN=your-secret-token
      - FLOWLINK_TLS_MODE=letsencrypt
      - FLOWLINK_TLS_DOMAIN=relay.example.com
    volumes:
      - flowlink-data:/var/lib/flowlink
    restart: unless-stopped

volumes:
  flowlink-data:
```

---

## 🤝 Участие

См. [CONTRIBUTING.md](CONTRIBUTING.md) для информации о том, как начать разработку.

---

## 📄 Лицензия

[MIT](LICENSE) © 2026

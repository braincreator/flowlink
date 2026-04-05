<p align="center">
  <img src="https://flowlink.flow-masters.ru/logo.svg" width="120" alt="FlowLink" />
</p>

<h1 align="center">FlowLink</h1>

<p align="center">
  <strong>AI-Native удалённое управление серверами</strong><br/>
  Open-source инструмент для управления серверами через ИИ-ассистентов.
</p>

<p align="center">
  <a href="#-быстрый-старт"><strong>Быстрый старт</strong></a> ·
  <a href="#-возможности">Возможности</a> ·
  <a href="#-архитектура">Архитектура</a> ·
  <a href="#-api-документация">API</a> ·
  <a href="#-тарифные-планы">Тарифы</a> ·
  <a href="#-навыки-devops--мониторинг">Навыки</a> ·
  <a href="README.md">English</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Go-1.24-00ADD8?logo=go" alt="Go" />
  <img src="https://img.shields.io/badge/License-BSL%201.1-blue.svg" alt="License" />
  <img src="https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker" alt="Docker" />
  <img src="https://img.shields.io/github/v/release/braincreator/flowlink?color=blue" alt="Release" />
</p>

---

**FlowLink** — SaaS-платформа удалённого управления серверами через ИИ. Установите один бинарник (~5MB) на каждый сервер — и управляйте всей инфраструктурой через OpenClaw, ChatGPT, Claude, Telegram или веб-панель.

> **Как это работает:** FlowLink — это реле. Оно маршрутизирует команды от ИИ-ассистентов на ваши серверы. ИИ — вы используете свой; мы предоставляем инфраструктуру.

🔗 **Разработано [FlowMasters](https://flow-masters.ru)** — чат-боты, ИИ-ассистенты и автоматизация для бизнеса.

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

### Сборка из исходников

```bash
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
│                 Оператор (OpenClaw / ИИ)                     │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │ OpenClaw     │  │ Любой MCP    │                         │
│  │ (ИИ-мозг)    │  │ клиент       │                         │
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
| **Relay** | Центральный сервер на VPS. Принимает WSS от агентов, предоставляет HTTP API и MCP для ИИ |
| **Agent** | Лёгкий демон на клиентских машинах. Выполняет команды, управляет файлами, бэкапами, kill switch |
| **MCP Server** | Интеграция с любым MCP-совместимым ИИ (OpenClaw, Claude, ChatGPT) — 8 инструментов |
| **Telegram Bot** | Управление агентами через Telegram: статусы, команды, approval, emergency stop |
| **Web Dashboard** | SPA-панель с dark theme для мониторинга и управления |

---

## 🔐 Архитектура безопасности

FlowLink реализует **7-уровневый конвейер безопасности**, который оценивает каждую команду перед выполнением:

```
Команда получена
       │
       ▼
  ┌──────────┐   НЕТ
  │ KillSwitch├──────────► DROP (лог + уведомление)
  └────┬─────┘
       │ ДА
       ▼
  ┌──────────┐   ДА
  │ Read-only├──────────► Выполнить только чтение
  └────┬─────┘
       │ НЕТ
       ▼
  ┌──────────┐   СОВПАДЕНИЕ
  │ Blacklist├──────────► ОТКЛОНИТЬ (лог + уведомление)
  └────┬─────┘
       │ НЕТ
       ▼
  ┌──────────┐   НАРУШЕНИЕ
  │ Sandbox  ├──────────► ОТКЛОНИТЬ (выход за границы)
  └────┬─────┘
       │ OK
       ▼
  ┌──────────┐   НУЖНО ПОДТВЕРЖДЕНИЕ
  │ Approval ├──────────► Ожидание человека
  └────┬─────┘
       │ ОДОБРЕНО / AUTO
       ▼
  ┌──────────┐
 │  Backup   │──► Снапшот перед деструктивными операциями
  └────┬─────┘
       │
       ▼
  ┌──────────┐
 │  Execute  │──► Выполнить команду, залогировать результат
  └──────────┘
```

### Ключевые свойства безопасности

| Свойство | Реализация |
|----------|------------|
| **E2EE** | Обмен ключами X25519 ECDH + AES-256-GCM для всех команд и ответов |
| **Реле слепое** | Реле пересылает зашифрованные блобы — не имеет доступа к открытому тексту |
| **Доверие устройств** | Владелец явно одобряет каждое устройство через Telegram перед установкой E2EE |
| **Zero-knowledge реле** | Даже скомпрометированное реле не может читать или изменять команды |

---

## 🔒 Сквозное шифрование (E2EE)

FlowLink шифрует **все полезные нагрузки команд и ответов** с помощью сквозного шифрования. Реле никогда не видит открытый текст.

### Генерация ключей

```bash
# Каждый владелец имеет пару ключей (автогенерация при первом запуске)
flowlink-agent keys generate
# → ~/.flowlink/e2ee_private.key  (X25519 закрытый ключ)
# → ~/.flowlink/e2ee_public.key   (X25519 открытый ключ)
```

- **Алгоритм:** X25519 (Curve25519 ECDH)
- **Симметричный шифр:** AES-256-GCM
- **Вывод ключа:** HKDF-SHA256
- **Права закрытого ключа:** `0600` (только чтение/запись владельца)

### Хранение ключей

| Файл | Содержимое | Права |
|------|-----------|-------|
| `~/.flowlink/e2ee_private.key` | X25519 закрытый ключ | `0600` |
| `~/.flowlink/e2ee_public.key` | X25519 открытый ключ | `0644` |
| `~/.flowlink/e2ee_devices.json` | Общие секреты по устройствам | `0600` |

### Процесс обмена ключами

```
  Владелец (Telegram)      Реле (VPS)         Устройство (Сервер)
       │                        │                       │
  1. /keys generate          │                       │
       │─── открытый ключ ─────►│                       │
       │                        │                       │
  2. Запрос привязки         │                       │
       │                        │◄── CODE ──────────────│
       │                        │                       │
  3. /approve_device CODE     │                       │
       │                        │─── подтверждение ─────►│
       │                        │                       │
  4. E2EE хэндшейк          │                       │
       │◄═══ ECDH ══════════════╪═══════════════════════►│
       │    (общий секрет вычисляется на обеих сторонах) │
       │                        │                       │
  5. Зашифрованное общение   │                       │
       │◄══════ AES-256-GCM ════╪══════════════════════►│
       │    (реле пересылает непрозрачные блобы)         │
```

### Ротация ключей

```bash
# Ротация всех ключей (требует повторного подтверждения всех устройств)
/rotate

# Автоматическая ротация (опционально, в конфиге)
# "auto_rotate": true  — ротация каждые 30 дней
```

> **Прямая секретность:** После ротации старые ключи безопасно удаляются. Ранее перехваченный зашифрованный трафик невозможно расшифровать. Однако FlowLink **не** реализует поканальную ротацию ключей (как Signal) — ротация является ручной/явной для простоты системы в сценариях управления серверами.

---

## 📱 Управление устройствами

FlowLink использует модель **доверия при первом подтверждении**. Каждое устройство должно быть явно одобрено владельцем через Telegram перед тем, как сможет выполнять команды.

### Команды

| Команда | Описание |
|---------|----------|
| `/devices` | Список всех подключённых устройств со статусом и E2EE-инфо |
| `/approve_device CODE` | Одобрить новое устройство (запускает E2EE хэндшейк) |
| `/reject_device CODE` | Отклонить запрос на привязку |
| `/revoke NAME` | Отозвать доступ устройства (немедленное отключение) |
| `/keys` | Показать отпечаток E2EE ключа и количество устройств |
| `/rotate` | Ротация E2EE ключей (все устройства должны повторно привязаться) |
| `/device_info NAME` | Детальная информация об устройстве (ОС, IP, последний визит, отпечаток ключа) |

### Процесс привязки

```
  ┌─────────┐         ┌─────────┐         ┌─────────┐
  │Устройство│         │  Реле   │         │Владелец │
  │ (Agent)  │         │         │         │(Telegram)│
  └────┬────┘         └────┬────┘         └────┬────┘
       │                   │                   │
  1. flowlink-agent setup                      │
       │── подключение + откр. ключ ──►│        │
       │                   │                   │
  2.                  ┌── pending ──►        │
       │                   │   /devices показывает│
       │                   │   "⚠️ ОЖИДАНИЕ"    │
       │                   │                   │
  3.                   │              /approve_device ABC123
       │                   │◄───────────────────│
       │                   │                   │
  4. ◄── E2EE установлен ──│                   │
       │                   │                   │
  5. ✅ Готово — зашифрованные команды работают │
       │═══════════════════│                   │
```

---

## ✨ Возможности

- 🖥️ **Удалённое выполнение команд** — shell exec с sandbox и таймаутами
- 📁 **Файловый менеджер** — чтение, запись, листинг директорий
- 💾 **Бэкапы и восстановление** — снапшоты с retention и лимитами
- 🛑 **Kill Switch** — emergency stop, pause, readonly mode
- ✅ **Approval system** — 3 режима: auto / soft_ask / hard_ask
- 📊 **Audit log** — JSONL-логирование каждого действия с экспортом
- 🔌 **MCP Server** — интеграция с любым ИИ-ассистентом (8 инструментов)
- 🤖 **Telegram Bot** — 15 команд для управления через Telegram
- 🖥️ **Web Dashboard** — SPA с dark theme
- 💰 **Billing** — 4 тарифных плана, SaaS-ready
- 🔒 **TLS** — self-signed + Let's Encrypt
- 📡 **Event Streaming** — SSE для real-time уведомлений
- 🏢 **Multi-tenancy** — несколько клиентов на одном реле

---

## 🔧 Конфигурация

### Настройка агента

```bash
# Интерактивная настройка (генерирует конфиг + ключи)
flowlink-agent setup --relay wss://your-relay.com --token YOUR_TOKEN
```

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

### Конфигурация E2EE

```yaml
# ~/.flowlink/config.yaml — секция E2EE
e2ee:
  enabled: true               # Включить сквозное шифрование
  auto_rotate: false           # Авто-ротация ключей каждые 30 дней
  key_dir: "~/.flowlink"      # Директория для хранения ключей
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

> **Ответственность:** FlowLink — инструмент маршрутизации, как SSH. Мы не контролируем, не модифицируем и не инициируем команды. Клиент (и его ИИ-ассистент) несёт полную ответственность за все команды, выполненные на его серверах.

---

## 🛡️ Защита от ИИ-ошибок

FlowLink спроектирован так, чтобы **предотвратить повреждения от ИИ-агентов**:

| Функция | Как работает |
|---------|-------------|
| **Read-only по умолчанию** | Новые агенты запускаются в режиме только чтение |
| **Чёрный список команд** | `rm -rf /`, `mkfs`, `dd if=/dev/zero`, fork bombs — заблокированы |
| **Approval prompts** | Деструктивные команды требуют подтверждения человека (Telegram / Dashboard) |
| **Автобэкап** | Автоматический снапшот перед любой деструктивной операцией |
| **Kill switch** | Мгновенная экстренная остановка через Telegram |
| **Sandbox** | Ограниченные директории и максимальные размеры файлов |
| **Timeout** | Автоматическое завершение команд после таймаута |

---

## 💰 Тарифные планы

| План | Цена/мес | Агенты | Команды/мес | Бэкапы | Хранилище | Фичи |
|------|---------|--------|-------------|---------|-----------|------|
| **Free** | $0 | 1 | 100 | 3 | 100 MB | Базовое выполнение |
| **Starter** | $10 | 3 | 1 000 | 10 | 1 GB | + Telegram Bot, Audit |
| **Pro** | $30 | 25 | 10 000 | 50 | 10 GB | + MCP, API, Dashboard |
| **Enterprise** | По запросу | 100+ | Безлимит | Безлимит | 100+ GB | Все фичи, SLA, white-label |

> Self-hosted реле всегда бесплатен (BSL 1.1 лицензия). Cloud тарифы — за управляемую инфраструктуру.

---

## 🤖 Telegram Bot

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

## 📖 API документация

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
| POST | `/api/v1/agents/pause` | Пауза агента |
| POST | `/api/v1/agents/resume` | Возобновить агента |

### Бэкапы

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/backups` | Список бэкапов |
| POST | `/api/v1/backups/{id}/restore` | Восстановить из бэкапа |

### Аудит

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

### Биллинг

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/billing/usage` | Статистика использования |
| GET | `/api/v1/billing/plan` | Текущий план |
| POST | `/api/v1/billing/plan/change` | Сменить план |
| GET | `/api/v1/billing/invoices` | Список счетов |

### События

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/events` | SSE поток событий |

### MCP

| Метод | Endpoint | Описание |
|-------|----------|----------|
| POST | `/mcp` | JSON-RPC MCP endpoint |

---

## 🔌 MCP интеграция

Подключение FlowLink к любому MCP-совместимому ИИ-ассистенту:

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

## 🧩 Навыки (DevOps & Мониторинг)

FlowLink можно расширять **навыками** (skills) — переиспользуемыми наборами команд для управления серверами. Навыки описываются в Markdown с YAML frontmatter и загружаются агентом при старте.

### Доступные навыки

| Навык | Иконка | Описание |
|-------|--------|----------|
| **Docker Manager** | 🐳 | Деплой, рестарт, масштабирование контейнеров. Health checks и авто-восстановление. |
| **Log Analyzer** | 📋 | AI-анализ логов. Поиск ошибок, аномалий и паттернов. |
| **SSL Manager** | 🔒 | Авто-продление Let's Encrypt. Мониторинг срока действия, уведомления. |
| **Backup Agent** | 💾 | Автоматические бэкапы (tar, PostgreSQL, MySQL, MongoDB). Расписание и восстановление. |
| **Uptime Monitor** | 📡 | HTTP/TCP/DNS health checks. Алерты при даунтайме, отслеживание SLA. |
| **Cost Tracker** | 💰 | Отслеживание затрат серверов, прогноз расходов, оптимизация ресурсов. |

### Использование

```bash
# Список доступных навыков
./bin/flowlink skills list

# Выполнить команду навыка
./bin/flowlink skills run docker-manager docker_ps
./bin/flowlink skills run log-analyzer search --pattern "ERROR" --file /var/log/syslog

# Включить health checks
./bin/flowlink skills enable uptime-monitor
```

### Формат навыка

Каждый навык — Markdown файл с YAML frontmatter:

```yaml
---
name: Docker Manager
version: 0.1.0
description: Управление Docker контейнерами
categories: [devops, containers]
commands:
  - name: docker_ps
    description: Список запущенных контейнеров
    run: docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
    timeout: 10
---
```

См. [`skills/`](./skills/) для примеров.

---

## 🤝 Участие

См. [CONTRIBUTING.md](CONTRIBUTING.md) для информации о разработке.

---

## 📄 Лицензия

[BSL 1.1](LICENSE) © 2026 FlowMasters — Self-hosted бесплатно навсегда. Конкурирующий SaaS/cloud запрещён. Автоматически конвертируется в GPL-3.0 с 5 апреля 2029 года.

---

<p align="center">
  Разработано с ❤️ командой <a href="https://flow-masters.ru">FlowMasters</a>
</p>

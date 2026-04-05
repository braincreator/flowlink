# FlowLink — AI Agent Relay & Management Platform

⚡ Управляйте AI-агентами через WebSocket реле. Relay, бэкапы, дашборд, LLM-прокси, MCP.

> **Self-hosted — бесплатно навсегда.** Cloud и Enterprise — [flowlink.flow-masters.ru](https://flowlink.flow-masters.ru)

## 📜 Лицензия

FlowLink распространяется под **[Business Source License 1.1 (BSL)](LICENSE)**.

| Использование | Разрешено? |
|---------------|------------|
| Self-hosted (бесплатно) | ✅ Навсегда |
| Коммерческое использование (self-hosted) | ✅ Да |
| Модификация | ✅ С сохранением лицензии |
| Конкурентный SaaS/Cloud на базе FlowLink | ❌ Запрещено |
| Конвертация в open source | 5 апреля 2029 (GPL-3.0) |

## Быстрый старт (5 минут)

### 1. Скачайте

```bash
# Relay server (управление)
curl -sL https://github.com/braincreator/flowlink/releases/latest/download/flowlink-relay-linux-amd64 -o flowlink-relay
chmod +x flowlink-relay

# Agent (на целевой машине)
curl -sL https://github.com/braincreator/flowlink/releases/latest/download/flowlink-agent-linux-amd64 -o flowlink-agent
chmod +x flowlink-agent
```

### 2. Настройте relay

```bash
./flowlink-relay setup

# Или неинтерактивно:
./flowlink-relay setup --non-interactive \
  --client-name "MyCompany" \
  --client-email "admin@example.com" \
  --api-token "my-secret-token"
```

### 3. Запустите relay

```bash
./flowlink-relay -config ~/.flowlink/relay.json

```

Relay запустится на:
- **WSS:** `:8443` (подключение агентов)
- **API:** `:8080` (HTTP API + Dashboard)

### 4. Подключите агента

На целевой машине:

```bash
./flowlink-agent init \
  --relay ws://YOUR_SERVER:8443/ws \
  --label "production-db" \
  --token "AGENT_TOKEN_FROM_SETUP"

./flowlink-agent start
```

### 5. Откройте Dashboard

```
https://YOUR_SERVER/dashboard/?token=my-secret-token
```

## API

Все endpoints требуют `Authorization: Bearer <token>` header.

### Агенты

| Method | Endpoint | Описание |
|--------|----------|----------|
| GET | `/api/v1/agents` | Список агентов |
| POST | `/api/v1/agents/register` | Зарегистрировать агента |
| POST | `/api/v1/agents/exec` | Выполнить команду |
| POST | `/api/v1/agents/delete/{id}` | Удалить агента |
| GET | `/api/v1/agents/sysinfo` | Системная информация |

### Файлы

| Method | Endpoint | Описание |
|--------|----------|----------|
| POST | `/api/v1/agents/files/read` | Прочитать файл |
| POST | `/api/v1/agents/files/write` | Записать файл |
| POST | `/api/v1/agents/files/list` | Список файлов |

### Клиенты

| Method | Endpoint | Описание |
|--------|----------|----------|
| GET | `/api/v1/clients` | Список клиентов |
| POST | `/api/v1/clients` | Создать клиента |
| GET | `/api/v1/clients/{id}` | Информация о клиенте |
| DELETE | `/api/v1/clients/{id}` | Деактивировать клиента |

### Аудит

| Method | Endpoint | Описание |
|--------|----------|----------|
| GET | `/api/v1/audit` | Лог команд |
| GET | `/api/v1/audit/stats` | Статистика |
| GET | `/api/v1/audit/export` | Экспорт CSV |

### Approvals

| Method | Endpoint | Описание |
|--------|----------|----------|
| GET | `/api/v1/approvals` | Список запросов |
| POST | `/api/v1/approvals/{id}/approve` | Одобрить |
| POST | `/api/v1/approvals/{id}/reject` | Отклонить |

### SSE

```
GET /api/v1/events?token=<token>
```

События: `agent.connected`, `agent.disconnected`, `approval.required`, `approval.granted`, `approval.rejected`

## Архитектура

```
┌──────────┐   WSS    ┌──────────┐   HTTP   ┌──────────┐
│  Agent   │ ◄──────► │  Relay   │ ◄──────► │Dashboard │
│ (server) │          │  (Go)    │          │ (SPA)    │
└──────────┘          └──────────┘          └──────────┘
                            │
                     ┌──────┴──────┐
                     │ Registry   │
                     │ Audit Log  │
                     │ Approvals  │
                     │ Billing    │
                     └─────────────┘
```

**Полная архитектура:** [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — все модули, протоколы, конфигурация.

## Конфигурация

`~/.flowlink/relay.json`:
```json
{
  "wss_addr": ":8443",
  "api_addr": ":8080",
  "api_token": "your-secret-token",
  "tls_cert": "",
  "tls_key": ""
}
```

## Тарифы

| План | Агенты | Цена |
|------|--------|------|
| Free | 3 | Бесплатно |
| Starter | 10 | 1 990 ₽/мес |
| Pro | 50 | 4 990 ₽/мес |
| Enterprise | 100+ | По запросу |


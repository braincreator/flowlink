# FlowLink v1.0 — Полный PRD

**Дата:** 2026-03-27
**Статус:** Draft
**Цель:** Production-ready SaaS продукт для удалённого AI-управления серверами

---

## 📋 Executive Summary

**FlowLink** — SaaS платформа удалённого управления серверами через AI. Клиент устанавливает один бинарник → мы управляем его инфраструктурой через AI (OpenClaw). Монетизация — подписка через Точка Банк.

**Revenue target:** 5-15M ₽/мес через 12 месяцев

---

## 🏗️ Архитектура (финальная)

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
│                 Оператор (MacBook)                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ OpenClaw     │  │ LM Studio    │  │ mcporter     │      │
│  │ (мозг)       │  │ (LLM)        │  │ (MCP client) │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
          │ HTTP API
          ▼
┌─────────────────────────────────────────────────────────────┐
│                 Внешние сервисы                              │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ Точка    │  │ Telegram Bot │  │ Let's Encrypt│          │
│  │ Банк API │  │ (клиента)    │  │ (TLS)        │          │
│  └──────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

---

## 📦 Блоки реализации (12 блоков, параллельно)

### Блок 1: E2E Testing Framework
**Что:** Unit + integration + E2E тесты для всего существующего кода
**Файлы:** `*_test.go`, `test/integration/`
**Зависимости:** нет (базовый)
**Время:** ~2ч
**Результат:** `go test ./...` зелёный

### Блок 2: Auth Module (Реле)
**Что:** Генерация pairwise токенов, JWT для API, ротация, rate limiting
**Файлы:** `internal/relay/auth.go`, `internal/relay/middleware.go`
**Зависимости:** нет
**Время:** ~2ч
**Результат:** Безопасная аутентификация

### Блок 3: TLS & Certificate Management
**Что:** Let's Encrypt auto-TLS, cert pinning, cert rotation
**Файлы:** `internal/relay/tls.go`, `internal/agent/tls.go`
**Зависимости:** нет
**Время:** ~1.5ч
**Результат:** WSS с реальным TLS

### Блок 4: Install Script & Systemd Service
**Что:** Полный install.sh, uninstall.sh, systemd service, update mechanism
**Файлы:** `scripts/install.sh`, `scripts/uninstall.sh`, `scripts/update.sh`
**Зависимости:** нет
**Время:** ~1.5ч
**Результат:** `curl -sSL https://install.flowlink.dev | bash` работает

### Блок 5: Audit Log
**Что:** JSON-лог каждого действия, хранение, API для запроса, экспорт
**Файлы:** `internal/relay/audit.go`, `internal/agent/audit.go`
**Зависимости:** нет
**Время:** ~1.5ч
**Результат:** Каждое exec/read/write логируется

### Блок 6: Agent Registry & Multi-tenancy
**Что:** Регистрация клиентов, разделение данных, API ключи
**Файлы:** `internal/relay/registry.go`, `internal/relay/tenant.go`
**Зависимости:** Auth (Блок 2)
**Время:** ~2ч
**Результат:** Несколько клиентов на одном реле

### Блок 7: Telegram Bot (клиента)
**Что:** Approval notifications, kill switch, status, backup restore
**Файлы:** `internal/tgbot/bot.go`, `internal/tgbot/handlers.go`
**Зависимости:** нет
**Время:** ~3ч
**Результат:** Клиент управляет через Telegram

### Блок 8: Billing (Точка Банк)
**Что:** Подписки (Starter/Business/Enterprise), счётчики, вебхуки
**Файлы:** `internal/billing/plans.go`, `internal/billing/tochka.go`, `internal/billing/webhook.go`
**Зависимости:** Registry (Блок 6)
**Время:** ~3ч
**Результат:** Оплата через Точка Банк

### Блок 9: MCP Transport Fix & Validation
**Что:** Тестирование MCP с mcporter, фикс багов, добавление недостающих инструментов
**Файлы:** `internal/relay/mcp_server.go` (фиксы)
**Зависимости:** E2E Tests (Блок 1)
**Время:** ~2ч
**Результат:** mcporter подключается, все 8 инструментов работают

### Блок 10: Event Streaming
**Что:** SSE для real-time вывода команд, file transfer (chunked upload/download)
**Файлы:** `internal/relay/stream.go`, `internal/agent/stream.go`
**Зависимости:** нет
**Время:** ~2ч
**Результат:** Real-time вывод длинных команд

### Блок 11: Dashboard Web UI
**Что:** Статус агентов, audit log, billing, управление (React SPA)
**Файлы:** `web/dashboard/`
**Зависимости:** API, Audit, Billing
**Время:** ~4ч
**Результат:** Веб-панель для оператора

### Блок 12: Documentation & Landing
**Что:** API docs, integration guide, landing page, demo
**Файлы:** `docs/`, `web/landing/`
**Зависимости:** всё остальное
**Время:** ~2ч
**Результат:** docs.flowlink.dev + flowlink.dev

---

## 🗓️ Параллельная декомпозиция

### Волна 1 (параллельно, ~2ч)
```
├── Блок 1: E2E Testing     ──┐
├── Блок 2: Auth Module      ──┤  Независимые
├── Блок 3: TLS              ──┤
├── Блок 4: Install Script   ──┤
└── Блок 5: Audit Log        ──┘
```

### Волна 2 (после ревью Волны 1, ~3ч)
```
├── Блок 6: Registry         ──┐  Зависит от Auth
├── Блок 7: Telegram Bot     ──┤  Независимый
├── Блок 9: MCP Fix          ──┤  Зависит от Tests
└── Блок 10: Event Streaming ──┘  Независимый
```

### Волна 3 (после ревью Волны 2, ~4ч)
```
├── Блок 8: Billing          ──┐  Зависит от Registry
└── Блок 11: Dashboard       ──┘  Зависит от API
```

### Волна 4 (финал, ~2ч)
```
└── Блок 12: Docs & Landing  ──  Зависит от всего
```

**Общее время:** ~11ч (3-4 дня при параллельной работе)

---

## 💳 Точка Банк Integration

### Подписки

| План | Цена/мес | Агенты | LLM запросы | Features |
|------|---------|--------|-------------|----------|
| Starter | 15 000 ₽ | 1 | 5 000 | Мониторинг, базовые команды |
| Business | 50 000 ₽ | 5 | 25 000 | Автономные задачи, бэкапы |
| Enterprise | 150 000 ₽ | Безлимит | Безлимит | White-label, SLA 99.9% |

### API Точка Банк
- **Оплата:** `/api/v1/payments/create` → redirect на оплату
- **Вебхук:** `/api/v1/webhooks/tochka` → confirm/cancel
- **Рекуррент:** подписка через `recurring_payment_id`
- **Чеки:** email-чек после оплаты

### Billing Flow
```
1. Клиент → Реле: POST /api/v1/billing/subscribe {plan: "business"}
2. Реле → Точка Банк: создать платёж
3. Точка Банк → Клиент: платёжная страница
4. Клиент → платит
5. Точка Банк → Реле: webhook (success)
6. Реле → Telegram Bot: "Подписка активирована ✅"
7. Реле → Agent: обновить tier (ограничения)
```

---

## 🧪 Quality Gates (после каждого блока)

```bash
# Обязательно:
go build ./...           # Компиляция
go test ./...            # Тесты
make lint                # Линтинг (если есть)

# Для специфичных блоков:
make build               # Билд бинарников
mcporter call flowlink.flowlink_agents  # MCP тест
curl -s https://relay/api/v1/health     # Health check
```

---

## 📊 KPI

| Метрика | Target |
|---------|--------|
| Test coverage | > 70% |
| E2E scenarios | 10+ |
| Build time | < 30 сек |
| Binary size (agent) | < 10MB |
| Binary size (relay) | < 12MB |
| Response latency (exec) | < 2 сек |
| MCP tool call latency | < 5 сек |

---

## 🛡️ Security Checklist

- [x] TLS (WSS)
- [x] Pairwise токены
- [x] Sandbox (blocked patterns)
- [x] Approval modes (auto/soft/hard)
- [x] Kill switch (emergency)
- [x] Backup before destructive
- [x] Circuit breaker
- [ ] JWT rotation
- [ ] Rate limiting (per client)
- [ ] IP whitelist (optional)
- [ ] Audit log (immutability)
- [ ] Cert pinning
- [ ] E2EE (v2)

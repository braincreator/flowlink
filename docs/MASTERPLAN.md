# FlowLink — Master Plan to Production

**Дата:** 2026-04-04
**Версия:** v0.2.1-dev → v1.0.0
**Статус:** Active
**Текущее состояние:** MVP с критическими пробелами в self-host опыте, безопасности и биллинге

---

## 📊 Текущее состояние (Аудит v0.2.1)

### Что работает (10 флоу)
- ✅ Agent WS подключение (heartbeat, ping/pong)
- ✅ Agent register (multi-tenant)
- ✅ Remote exec (streaming)
- ✅ File I/O (read/write/list)
- ✅ Sysinfo (CPU/RAM/OS)
- ✅ Skills (push/list/delete)
- ✅ Task submit/cancel
- ✅ LLM Proxy
- ✅ SSE Events (починено)
- ✅ Dashboard SPA (починено)

### Что сломано / отсутствует
- 🔴 Telegram Bot — не компилируется (6 ошибок в payment_handlers.go)
- 🔴 E2EE — changelog говорит что есть, но в коде 0 строк реализации
- 🔴 Device Pairing — tgbot вызывает несуществующие relay endpoints
- 🔴 Approval Queue — нет API, нет SSE event, relay не знает куда направлять
- 🔴 Onboarding — нет setup wizard, пользователь должен собирать конфиг вручную
- 🟡 Billing — plans/usage работают, оплата через Точку — credentials не настроены
- 🟡 Backup — relay не имеет endpoints, только через integration (Python)
- 🟡 TLS — конфиг есть, но нет guide для self-host
- 🟡 Graceful shutdown — нет signal.NotifyContext

---

## 🗺️ Roadmap

```
v0.2.1 (текущий)  →  v0.3.0 (self-host MVP)  →  v0.5.0 (SaaS beta)  →  v1.0.0 (production)
      1-2 дня            5-7 дней                   7-10 дней              3-5 дней
```

---

## 📦 ФАЗА 1: v0.3.0 — Self-Host MVP

**Цель:** Пользователь скачивает бинарник, запускает setup wizard, подключает агента, управляет через dashboard. **Zero dependencies.**

**Оценка:** 25-35h работы

---

### Волна 1.1: Foundation (8-10h)

Все задачи параллельны, можно делать одновременно.

#### Задача 1.1.1: Починить Telegram Bot компиляцию
**Приоритет:** 🔴 P0 | **Время:** 1-2h | **Файлы:** `internal/tgbot/payment_handlers.go`, `internal/tgbot/bot.go`

**Проблема:** 6 compile errors — дубликат `sendMessageWithKeyboard`, несовпадение типов `[][]InlineButton` vs `*tgInlineKeyboard`.

**Что сделать:**
- [ ] Удалить дубликат `sendMessageWithKeyboard` из `payment_handlers.go`
- [ ] Исправить типы keyboard (использовать существующий тип из bot.go)
- [ ] Удалить unused import `strconv`
- [ ] Запустить `go build ./internal/tgbot/...` — 0 ошибок
- [ ] Если payment_handlers зависят от несуществующих billing endpoints — сделать заглушки (return "coming soon")

**Тест:** `go build ./...` — чистая компиляция

---

#### Задача 1.1.2: Graceful Shutdown
**Приоритет:** 🔴 P0 | **Время:** 1h | **Файлы:** `internal/relay/relay.go`

**Проблема:** Kill -9 может повредить JSONL файлы реестра.

**Что сделать:**
- [ ] Добавить `signal.NotifyContext(os.Interrupt, syscall.SIGTERM)` в `Start()`
- [ ] При сигнале: (1) остановить принимать новые WS соединения, (2) дождаться текущих команд, (3) вызвать `registry.Save()` (compaction), (4) закрыть listener
- [ ] Добавить `shutdownTimeout` (default: 30s)
- [ ] Логировать shutdown progress

**Тест:** `kill -SIGTERM <pid>` → clean shutdown, JSONL не повреждён

---

#### Задача 1.1.3: Onboarding Wizard (CLI setup)
**Приоритет:** 🔴 P0 | **Время:** 4-5h | **Файлы:** `cmd/setup/main.go` (новый), `cmd/relay/main.go`

**Проблема:** Self-host пользователь должен вручную создавать relay.json и делать curl для регистрации.

**Что сделать:**
- [ ] Создать `cmd/setup/main.go` — интерактивный CLI wizard
- [ ] Шаг 1: API Token — сгенерировать или ввести свой
- [ ] Шаг 2: Ports — WSS (default :8443) + API (default :8080)
- [ ] Шаг 3: TLS — none / self-signed / manual cert path
- [ ] Шаг 4: Admin — имя, email
- [ ] Шаг 5: First Client — создать первого клиента, показать API token
- [ ] Шаг 6: First Agent — создать агента, показать connection string
- [ ] Вывести: `relay.json`, client API token, agent token, connection URL
- [ ] Объединить в один бинарник: `flowlink-relay setup` для wizard, `flowlink-relay serve` для запуска
- [ ] Добавить `--non-interactive` режим с env vars для автоматизации

**Тест:** `./flowlink-relay setup` → конфиг создан, первый клиент + агент готовы

---

#### Задача 1.1.4: Per-Client Rate Limiting
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/relay/middleware.go`

**Проблема:** Rate limiter global — один клиент может забить весь relay.

**Что сделать:**
- [ ] Добавить `ClientID` в контекст (уже делается в auth middleware)
- [ ] Создать `PerClientRateLimiter` — map[clientID]*tokenBucket
- [ ] Лимиты из тарифа (Free: 10 RPM, Starter: 60 RPM, Pro: 300 RPM)
- [ ] Redis fallback для multi-instance (опционально, P2)
- [ ] HTTP 429 с `Retry-After` header

**Тест:** Free клиент → 10 запросов → 429. Starter клиент → 60 запросов → 429.

---

#### Задача 1.1.5: Audit Log Immutability
**Приоритет:** 🟡 P1 | **Время:** 1h | **Файлы:** `internal/relay/audit.go`

**Проблема:** Audit log хранится в памяти + JSONL, но нет защиты от подмены.

**Что сделать:**
- [ ] Добавить HMAC-SHA256 подпись для каждой записи аудита
- [ ] Проверять цепочку при загрузке (integrity check)
- [ ] Добавить `GET /api/v1/audit/integrity` endpoint
- [ ] Логировать integrity violations

**Тест:** Изменить строку в audit JSONL → integrity check fails

---

### Волна 1.2: Agent Experience (6-8h)

#### Задача 1.2.1: Agent Connection Flow E2E
**Приоритет:** 🔴 P0 | **Время:** 2h | **Файлы:** `internal/relay/relay.go` (handleAgentWS), `internal/protocol/protocol.go`

**Проблема:** Непонятно какой токен использует агент для WS подключения. `allowed_tokens` в конфиге vs token из registry.

**Что сделать:**
- [ ] Документировать flow: setup wizard генерирует agent token → агент подключается с этим токеном
- [ ] В `handleAgentWS`: искать token в registry (not just `allowed_tokens` map)
- [ ] Добавить `agent_id` в URL query: `/ws?token=XXX` (already works) + fallback to `allowed_tokens`
- [ ] При подключении: отправить `connected` message с agent metadata
- [ ] При отключении: обновить `IsOnline=false`, отправить SSE event
- [ ] Написать E2E тест: agent connects → relay registers → exec → output → done

**Тест:** Запустить relay → создать клиента → создать агента → агент подключается → exec работает

---

#### Задача 1.2.2: Approval Queue API
**Приоритет:** 🟡 P1 | **Время:** 3h | **Файлы:** `internal/relay/approval.go` (новый), `internal/relay/relay.go`, `internal/relay/sse.go`

**Проблема:** Agent отправляет `approval_request` через WS, но relay не знает куда его направить.

**Что сделать:**
- [ ] Создать `ApprovalQueue` — in-memory + JSONL persistence
- [ ] Relay получает `approval_request` от агента → сохраняет в очередь → отправляет SSE event
- [ ] SSE event: `{type: "approval_request", id, agent_id, command, risk_level}`
- [ ] `POST /api/v1/approvals/{id}/approve` — отправить `approval_response` агенту
- [ ] `POST /api/v1/approvals/{id}/reject` — отклонить
- [ ] `GET /api/v1/approvals` — список pending approvals
- [ ] Auto-expire после `hard_ask_timeout_sec`
- [ ] TG Bot: при получении approval_request → отправить inline buttons (approve/reject)

**Тест:** hard_ask команда → approval_request в очереди → approve через API → агент выполняет

---

#### Задача 1.2.3: Agent Binary (демон)
**Приоритет:** 🟡 P1 | **Время:** 2-3h | **Файлы:** `cmd/agent/main.go` (новый)

**Проблема:** Нет отдельного агент-бинарника. Agent code есть в `internal/agent/`, но нет entrypoint.

**Что сделать:**
- [ ] Создать `cmd/agent/main.go` — entrypoint для agent daemon
- [ ] Флаги: `--relay-url`, `--token`, `--label`, `--work-dir`
- [ ] При первом запуске: создать `~/.flowlink/config.json` из defaults
- [ ] Systemd service: `scripts/flowlink-agent.service`
- [ ] macOS LaunchAgent: `scripts/com.flowlink.agent.plist`
- [ ] Логирование в `~/.flowlink/logs/agent.log`

**Тест:** `./flowlink-agent --relay-url wss://relay/ws --token XXX` → подключается, heartbeat OK

---

### Волна 1.3: Dashboard & Docs (5-7h)

#### Задача 1.3.1: Dashboard — Onboarding Page
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/dashboard/static/`

**Проблема:** Dashboard пустой при первом запуске — нет wizard.

**Что сделать:**
- [ ] Добавить "Welcome" экран если нет клиентов
- [ ] Кнопка "Create First Client" → inline форма
- [ ] После создания клиента → "Add Agent" → показать connection string
- [ ] Quick start guide (5 шагов)
- [ ] Прогресс-бар: setup wizard completion

**Тест:** Новый relay → открыть dashboard → see welcome → create client → see agent setup

---

#### Задача 1.3.2: Dashboard — Agent Detail Page
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/dashboard/static/`

**Что сделать:**
- [ ] Клик по агенту → detail page
- [ ] Показать: sysinfo (CPU/RAM/Disk), online status, last seen
- [ ] Встроенный terminal: ввод команды → exec → streaming output
- [ ] File browser: дерево файлов → read/write
- [ ] Skills list + push/delete
- [ ] Approvals queue (pending → approve/reject)

**Тест:** Открыть агент → see sysinfo → exec command → see output

---

#### Задача 1.3.3: Documentation Update
**Приоритет:** 🟢 P2 | **Время:** 2h | **Файлы:** `docs/`

**Что сделать:**
- [ ] Обновить `README.md` — self-host quick start (5 команд)
- [ ] Обновить `API.md` — актуальные endpoints (убрать мёртвые)
- [ ] Создать `docs/SELF_HOST.md` — полное руководство
- [ ] Создать `docs/AGENT_SETUP.md` — как подключить агента
- [ ] Обновить `docs/ARCHITECTURE.md` — актуальная архитектура
- [ ] Обновить `docs/DEPLOYMENT.md` — production deployment guide

---

### Волна 1.4: Testing (3-4h)

#### Задача 1.4.1: Unit Tests
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/relay/*_test.go`, `internal/billing/*_test.go`

**Что сделать:**
- [ ] `registry_test.go` — CRUD clients/agents, persistence, deactivation
- [ ] `billing_test.go` — plan limits, period discounts, usage tracking
- [ ] `middleware_test.go` — auth (Bearer + query param), rate limit, CORS
- [ ] `sse_test.go` — SSE connection, event delivery
- [ ] `approval_test.go` — queue, approve, reject, timeout
- [ ] Target: > 70% coverage

**Тест:** `go test ./...` — all green

---

#### Задача 1.4.2: Integration Tests
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `test/integration/`

**Что сделать:**
- [ ] Test suite: запустить relay → создать клиента → создать агента → mock WS → exec → verify
- [ ] Test auth: неверный токен → 401, верный → 200
- [ ] Test rate limit: превысить лимит → 429
- [ ] Test graceful shutdown: SIGTERM → clean exit
- [ ] CI-ready: `go test -tags=integration ./test/...`

**Тест:** `go test -tags=integration ./...` — all green

---

### Итого Фаза 1: v0.3.0

| Волна | Задачи | Время | Зависимости |
|-------|--------|-------|-------------|
| 1.1 Foundation | 1.1.1-1.1.5 | 8-10h | Все параллельны |
| 1.2 Agent | 1.2.1-1.2.3 | 6-8h | 1.2.2 после 1.2.1 |
| 1.3 Dashboard | 1.3.1-1.3.3 | 5-7h | После 1.1 |
| 1.4 Testing | 1.4.1-1.4.2 | 3-4h | После всех |
| **Итого** | **14 задач** | **22-29h** | |

---

## 📦 ФАЗА 2: v0.5.0 — SaaS Beta

**Цель:** Полный SaaS продукт — биллинг, автоскейлинг, мульти-инстанс. Готов к первым платящим клиентам.

**Оценка:** 30-40h работы

---

### Волна 2.1: Billing & Payments (8-10h)

#### Задача 2.1.1: Relay ↔ Integration Bridge
**Приоритет:** 🔴 P0 | **Время:** 3h | **Файлы:** `internal/relay/relay.go`, `internal/integration/`

**Проблема:** Relay (Go) и Integration (Python) — две отдельные системы. Нет единого API.

**Что сделать:**
- [ ] Решить: объединить integration в relay (Go) или оставить отдельным
- [ ] Если отдельный: proxy все billing endpoints через relay → integration
- [ ] Единый API token для обоих сервисов
- [ ] Nginx: единый entry point (`api.flow-masters.ru`)
- [ ] Health check: relay проверяет integration, integration проверяет Supabase

**Рекомендация:** Оставить отдельным. Python integration уже работает с Supabase/S3. Добавить proxy.

---

#### Задача 2.1.2: Точка Банк — Real Payment Flow
**Приоритет:** 🔴 P0 | **Время:** 3-4h | **Файлы:** `internal/integration/`, `internal/billing/tochka.go`

**Проблема:** `create_tochka_payment()` использует неправильный API URL. Credentials не настроены.

**Что сделать:**
- [ ] Получить Tochka API credentials (Terminal Key + Password)
- [ ] Исправить API URL (production: `https://securepay.tinkoff.ru/v2/` или актуальный Tochka endpoint)
- [ ] Реализовать: create payment → redirect → webhook → confirm
- [ ] Webhook verification (signature check)
- [ ] Test: создать тестовый платёж → оплатить → получить webhook → обновить статус
- [ ] Email receipt (опционально)

---

#### Задача 2.1.3: Subscription Lifecycle
**Приоритет:** 🟡 P1 | **Время:** 2-3h | **Файлы:** `internal/integration/`, `internal/billing/subscription.go`

**Что сделать:**
- [ ] Создать подписку: plan + period (monthly/quarterly/yearly)
- [ ] Auto-renewal: за 3 дня до expiry → создать платёж
- [ ] Grace period: 7 дней после expiry → readonly mode
- [ ] Plan change: mid-cycle proration
- [ ] Cancellation: до конца периода active, после → deactivate
- [ ] Webhook notification на каждый lifecycle event

---

#### Задача 2.1.4: Usage Metering
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/billing/usage.go`, `internal/relay/relay.go`

**Что сделать:**
- [ ] Считать: exec commands, LLM requests, file operations, backup size
- [ ] Хранить usage в JSONL (per client, per day)
- [ ] `GET /api/v1/billing/usage` — текущее использование + лимиты
- [ ] Warning при 80% лимита (SSE event + TG notification)
- [ ] Hard limit при 100% → reject с 429 + message

---

### Волна 2.2: Backup & Recovery (5-7h)

#### Задача 2.2.1: Backup API (Relay)
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/relay/relay.go`, `internal/agent/backup.go`

**Проблема:** Relay не имеет backup endpoints. Backup engine в `internal/agent/` не подключён к relay API.

**Что сделать:**
- [ ] `POST /api/v1/agents/backup` — trigger backup на агенте
- [ ] `GET /api/v1/agents/backup/list` — список снапшотов
- [ ] `POST /api/v1/agents/backup/{id}/restore` — восстановление
- [ ] `DELETE /api/v1/agents/backup/{id}` — удалить снапшот
- [ ] Backup before destructive commands (auto-trigger)
- [ ] WS protocol: `backup_request` / `backup_response` / `backup_progress`

---

#### Задача 2.2.2: Cloud Backup (S3)
**Приоритет:** 🟢 P2 | **Время:** 3h | **Файлы:** `internal/integration/`

**Что сделать:**
- [ ] Автоматическая загрузка бэкапов в S3 (Timeweb Cloud Storage)
- [ ] Per-tenant bucket isolation
- [ ] Backup scheduling: daily/weekly (из тарифа)
- [ ] Restore from cloud: download → extract → apply
- [ ] Storage quota enforcement
- [ ] Cleanup: retention policy + auto-delete

---

#### Задача 2.2.3: Disaster Recovery
**Приоритет:** 🟢 P2 | **Время:** 2h

**Что сделать:**
- [ ] Relay data backup: registry + audit + billing → S3
- [ ] Schedule: каждые 6 часов
- [ ] Restore procedure: новый VPS → download → start
- [ ] Documentation: runbook для восстановления

---

### Волна 2.3: Multi-Instance & Scaling (5-7h)

#### Задача 2.3.1: Autoscaling (Timeweb Cloud)
**Приоритет:** 🟢 P2 | **Время:** 3h | **Файлы:** `internal/integration/billing_autoscale.go`, `flowlink-autoscale/`

**Проблема:** Модуль `github.com/braincreator/flowlink-autoscale` — private, не в этом репо.

**Что сделать:**
- [ ] Перенести autoscale logic в этот репо (или сделать public)
- [ ] Scale up: при N agents per server → создать новый Timeweb instance
- [ ] Scale down: при < N/3 agents → drain + destroy
- [ ] Traffic routing: nginx upstream + health checks
- [ ] Cooldown: минимум 10 мин между scaling events
- [ ] Cost tracking: показывать стоимость инфраструктуры

---

#### Задача 2.3.2: Multi-Relay Coordination
**Приоритет:** 🟢 P2 | **Время:** 3h

**Что сделать:**
- [ ] Shared state: Redis или PostgreSQL для registry (вместо JSONL)
- [ ] Agent routing: load balancer → least-loaded relay
- [ ] Session affinity: sticky sessions для WS connections
- [ ] Failover: relay down → agents reconnect to another
- [ ] Config: `cluster.mode = single | multi`

---

### Волна 2.4: Security (6-8h)

#### Задача 2.4.1: E2EE Implementation
**Приоритет:** 🟡 P1 | **Время:** 4h | **Файлы:** `internal/relay/crypto.go`, `internal/agent/crypto.go` (новые)

**Проблема:** Changelog v0.2.0 говорит что E2EE есть, но в коде 0 строк реализации.

**Что сделать:**
- [ ] X25519 ECDH key exchange при WS handshake
- [ ] AES-256-GCM для шифрования payloads
- [ ] Key storage: `~/.flowlink/keys/` (chmod 0600)
- [ ] Key rotation: `/keys rotate` (TG bot) + `/api/v1/agents/keys/rotate`
- [ ] Relay forwards encrypted blobs — zero-knowledge
- [ ] Fallback: plain mode если E2EE не настроен (backward compat)

**Wire format:**
```json
{
  "type": "exec_request",
  "encrypted": true,
  "nonce": "base64...",
  "payload": "base64..."  // AES-256-GCM encrypted
}
```

---

#### Задача 2.4.2: Device Pairing Endpoints
**Приоритет:** 🟢 P2 | **Время:** 2h | **Файлы:** `internal/relay/relay.go`, `internal/relay/devices.go` (новый)

**Проблема:** TG bot вызывает `/devices`, `/approve_device` и т.д., но этих endpoints нет в relay.

**Что сделать:**
- [ ] `GET /api/v1/devices` — список зарегистрированных устройств
- [ ] `POST /api/v1/devices/pair` — запрос на pair (6-digit code, 10min TTL)
- [ ] `POST /api/v1/devices/{id}/approve` — одобрить
- [ ] `POST /api/v1/devices/{id}/reject` — отклонить
- [ ] `DELETE /api/v1/devices/{id}` — revoke
- [ ] Persistence: JSONL

---

#### Задача 2.4.3: JWT Token Rotation
**Приоритет:** 🟢 P2 | **Время:** 1h | **Файлы:** `internal/relay/auth.go`

**Что сделать:**
- [ ] JWT с expiry (24h) + refresh token (7d)
- [ ] `POST /api/v1/auth/refresh` — обновить access token
- [ ] Token blacklist при logout
- [ ] Token revocation API для admin

---

### Волна 2.5: Landing & Onboarding (4-5h)

#### Задача 2.5.1: Landing Page Update
**Приоритет:** 🟢 P2 | **Время:** 2h | **Файлы:** `docs/landing/`, `web/landing/`

**Что сделать:**
- [ ] Обновить flowlink.flow-masters.ru — актуальные фичи из v0.3
- [ ] Pricing page: планы + цены + FAQ
- [ ] CTA: "Try Free" → registration → setup wizard
- [ ] Demo video / GIF (опционально)

---

#### Задача 2.5.2: Self-Host Install Script
**Приоритет:** 🟢 P2 | **Время:** 1.5h | **Файлы:** `scripts/install.sh`

**Что сделать:**
- [ ] `curl -sSL https://get.flowlink.dev | bash`
- [ ] Detect OS/arch → download correct binary
- [ ] Systemd service (Linux) / LaunchAgent (macOS)
- [ ] Auto-open dashboard after setup
- [ ] Uninstall script

---

#### Задача 2.5.3: Nginx Config Generator
**Приоритет:** 🟢 P2 | **Время:** 1h

**Что сделать:**
- [ ] `flowlink-relay setup-nginx` — генерирует nginx config
- [ ] Auto-configure TLS (Let's Encrypt)
- [ ] WebSocket proxy headers
- [ ] SSE proxy headers (`X-Accel-Buffering: no`)

---

### Итого Фаза 2: v0.5.0

| Волна | Задачи | Время | Зависимости |
|-------|--------|-------|-------------|
| 2.1 Billing | 2.1.1-2.1.4 | 10-12h | 2.1.2 после 2.1.1 |
| 2.2 Backup | 2.2.1-2.2.3 | 7-9h | 2.2.2 после 2.2.1 |
| 2.3 Scaling | 2.3.1-2.3.2 | 6h | После 2.1 |
| 2.4 Security | 2.4.1-2.4.3 | 7-8h | Все параллельны |
| 2.5 Landing | 2.5.1-2.5.3 | 4-5h | После 1.3 |
| **Итого** | **15 задач** | **34-40h** | |

---

## 📦 ФАЗА 3: v1.0.0 — Production

**Цель:** Production-ready SaaS. SLA 99.9%, мониторинг, алертинг, документация.

**Оценка:** 10-15h работы

---

### Волна 3.1: Observability (4-5h)

#### Задача 3.1.1: Prometheus Metrics
**Приоритет:** 🟡 P1 | **Время:** 2h | **Файлы:** `internal/relay/metrics.go` (новый)

**Что сделать:**
- [ ] `GET /metrics` — Prometheus format
- [ ] Метрики: `flowlink_agents_online`, `flowlink_commands_total`, `flowlink_command_duration_seconds`, `flowlink_api_requests_total`
- [ ] Per-client breakdown (labels)
- [ ] Dashboard: Grafana template (JSON)

---

#### Задача 3.1.2: Health Checks & Alerting
**Приоритет:** 🟡 P1 | **Время:** 2h

**Что сделать:**
- [ ] `GET /health` — deep health (relay + integration + Supabase + S3)
- [ ] Liveness vs Readiness probes
- [ ] Alert rules: agents offline > 5min, command failure rate > 10%, disk > 80%
- [ ] TG notification для critical alerts

---

#### Задача 3.1.3: Structured Logging
**Приоритет:** 🟢 P2 | **Время:** 1h

**Что сделать:**
- [ ] JSON logging (включается через env `FLOWLINK_LOG_FORMAT=json`)
- [ ] Log levels: debug/info/warn/error
- [ ] Request ID tracing
- [ ] Log rotation

---

### Волна 3.2: Production Hardening (4-5h)

#### Задача 3.2.1: Configuration Validation
**Приоритет:** 🟡 P1 | **Время:** 1h | **Файлы:** `internal/config/config.go`

**Что сделать:**
- [ ] Валидация конфига при загрузке (ports range, token length, TLS cert existence)
- [ ] Warning для insecure settings (no TLS, default token)
- [ ] `flowlink-relay validate-config` — проверка без запуска

---

#### Задача 3.2.2: Error Handling Standardization
**Приоритет:** 🟡 P1 | **Время:** 2h

**Что сделать:**
- [ ] Единый error format: `{"code": "ERR_xxx", "message": "...", "details": {}}`
- [ ] Error codes enum
- [ ] Все handlers используют единый формат
- [ ] Documentation: error code reference

---

#### Задача 3.2.3: Performance Optimization
**Приоритет:** 🟢 P2 | **Время:** 2h

**Что сделать:**
- [ ] WS connection pooling
- [ ] JSONL write buffering (flush every N seconds or N records)
- [ ] Memory profiling: `pprof` endpoint (debug mode)
- [ ] Load test: 100 concurrent agents, 1000 exec/min

---

### Волна 3.3: Release (3-4h)

#### Задача 3.3.1: Version Management
**Приоритет:** 🟡 P1 | **Время:** 1h

**Что сделать:**
- [ ] Semver: v1.0.0
- [ ] Git tags + release notes
- [ ] Changelog auto-generation
- [ ] GitHub Releases: binaries for linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

---

#### Задача 3.3.2: Final Documentation
**Приоритет:** 🟡 P1 | **Время:** 2h

**Что сделать:**
- [ ] API Reference (OpenAPI 3.0 spec)
- [ ] Self-host guide (from zero to production)
- [ ] SaaS operator guide
- [ ] Agent setup guide (all OS)
- [ ] Troubleshooting guide
- [ ] Architecture decision records (ADRs)

---

### Итого Фаза 3: v1.0.0

| Волна | Задачи | Время |
|-------|--------|-------|
| 3.1 Observability | 3.1.1-3.1.3 | 5h |
| 3.2 Hardening | 3.2.1-3.2.3 | 5h |
| 3.3 Release | 3.3.1-3.3.2 | 3h |
| **Итого** | **8 задач** | **13h** |

---

## 📊 СВОДНАЯ ТАБЛИЦА

| Фаза | Задач | Время | Milestone |
|------|-------|-------|-----------|
| **v0.3.0** Self-Host MVP | 14 | 22-29h | Первые self-host пользователи |
| **v0.5.0** SaaS Beta | 15 | 34-40h | Первые платящие клиенты |
| **v1.0.0** Production | 8 | 13h | Production-ready SaaS |
| **ИТОГО** | **37 задач** | **69-82h** | |

---

## 🎯 Critical Path (Минимальный путь к MVP)

Если нужно **быстро** — вот минимальный набор для работающего self-host:

```
1.1.1 Починить tgbot compile          (1-2h)  ← блокирует всё
1.1.3 Onboarding wizard               (4-5h)  ← главный флоу
1.2.1 Agent Connection Flow E2E       (2h)    ← ядро продукта
1.3.1 Dashboard Onboarding Page       (2h)    ← первый экран
1.4.1 Unit Tests                      (2h)    ← качество
────────────────────────────────────────────
Итого: 11-12h → работающий self-host MVP
```

---

## 📅 Предложенный порядок работы

### День 1-2: Foundation
- [ ] 1.1.1 Починить tgbot
- [ ] 1.1.2 Graceful shutdown
- [ ] 1.1.4 Per-client rate limiting
- [ ] 1.1.5 Audit immutability

### День 3-4: Agent Experience
- [ ] 1.2.1 Agent Connection Flow E2E
- [ ] 1.2.2 Approval Queue API
- [ ] 1.2.3 Agent Binary (daemon)

### День 5: Onboarding
- [ ] 1.1.3 Setup Wizard
- [ ] 1.3.1 Dashboard Onboarding

### День 6: Testing + Release
- [ ] 1.4.1 Unit Tests
- [ ] 1.4.2 Integration Tests
- [ ] Tag v0.3.0

### День 7-10: SaaS Beta
- [ ] 2.1.x Billing
- [ ] 2.2.x Backup
- [ ] 2.4.1 E2EE

### День 11-14: Polish + Production
- [ ] 2.3.x Scaling
- [ ] 2.5.x Landing
- [ ] 3.x Production hardening
- [ ] Tag v1.0.0

---

**Last Updated:** 2026-04-04 | **Author:** OpenClaw Audit | **v1.0**

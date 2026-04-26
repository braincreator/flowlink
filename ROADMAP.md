# FlowLink Feature Roadmap

## Текущий статус (2026-04-26)

**Версия:** 0.3.1-dev | **Крейты:** 12 | **Строк:** ~158K | **Тестов:** ~1187

**Готово:** Relay, Agent, Shield (L1-L7), Policy Engine, MCP (12 tools), Auth (OAuth VK/Yandex/GitHub + 2FA + SAML), Billing (Точка Банк), API Keys, Pattern Learning, SIEM (CEF/LEEF/JSON + RuSIEM + MaxPatrol), K8s Operator (draft), GitOps (19K строк, feature-gated), Compliance API, Forensics Timeline, AI Ops, Change Management, Service Catalog, Zero-Trust Secret Injection, Discovery (80+ сервисов), Infra Map, Telegram Bot

**Крейты:**
- `core` (~15K) — типы сообщений, конфигурация, каналы
- `crypto` (~3K) — X25519 + AES-256-GCM
- `db` (~12K) — PostgreSQL репозитории (sqlx)
- `billing` (~8K) — планы, счета, usage, Точка Банк
- `agent` (~25K) — диспетчер, политики, sandbox, killswitch, exec
- `relay` (~35K) — WS сервер, REST API, RBAC, E2EE, MCP
- `shield` (~20K) — eBPF/macOS ES, анализ угроз, L1-L7
- `gitops` (~19K) — drift detection, ServerGuard, backup engine (feature-gated)
- `k8s` (~5K) — Operator, CRD, admission webhooks (draft)
- `mcp` (~3K) — MCP протокол
- `sentinel` (~5K) — AI Ops ассистент, pattern learning
- `cli` (~8K) — бинарник, MCP сервер

---

## 📋 План разработки

### Фаза 1: Custom RBAC Roles
**Оценка:** 2-3 дня
**Зависимости:** нет

**Что добавить:**
- DB: таблица `custom_roles` (id, org_id, name, description, permissions JSON, created_by, created_at)
- API: CRUD `/api/v1/roles` — create/list/update/delete custom ролей
- API: назначение ролей юзерам `/api/v1/roles/{id}/assign`
- Middleware: проверка кастомных ролей вместе с built-in (owner/admin/viewer)
- Dashboard: UI страница Roles (создание, назначение, управление)
- Связь с API keys: ключ наследует кастомную роль если назначена

**Интеграция:**
- `crates/core/src/rbac.rs` — расширить Role enum, добавить CustomRole struct
- `crates/relay/src/middleware.rs` — обновить rbac_layer для кастомных ролей
- `crates/relay/src/server.rs` — новые endpoints
- Migration 035

---

### Фаза 2: Webhook Notifications ✅
**Статус:** РЕАЛИЗОВАНО

Webhooks реализованы в relay. Поддерживаются shield.block, shield.warn, approval.pending, approval.resolved, agent.online, agent.offline, policy.changed.
**Оценка:** 2 дня
**Зависимости:** нет

**Что добавить:**
- DB: таблица `webhooks` (id, org_id, url, events[], secret, active, created_at)
- API: CRUD `/api/v1/webhooks`
- Events: `shield.block`, `shield.warn`, `approval.pending`, `approval.resolved`, `agent.online`, `agent.offline`, `policy.changed`, `rate_limit.exceeded`
- Delivery: async HTTP POST с HMAC-SHA256 подписью, retry 3x с backoff
- Dashboard: UI настройки вебхуков + delivery log

**Интеграция:**
- Новый крейт или модуль: `crates/relay/src/webhooks.rs`
- Вызовы из `handler.rs` (exec result), `approval.rs`, `pool.rs` (agent status)
- Migration 036

---

### Фаза 3: Agent Fleet Tags & Filtering ✅
**Статус:** РЕАЛИЗОВАНО

Теги реализованы. API: `/api/v1/agents/{id}/tags`, MCP: фильтр по tags в flowlink_exec.
**Оценка:** 1-2 дня
**Зависимости:** нет

**Что добавить:**
- DB: таблица `agent_tags` (agent_id, tag) + индекс
- API: `/api/v1/agents/{id}/tags` — set/get/delete
- MCP tool: `flowlink_exec` — параметр `tags` для batch-exec по группе
- MCP tool: `flowlink_list_agents` — фильтр по tags
- Dashboard: фильтр серверов по тегам, batch-select

**Интеграция:**
- `crates/db/src/migrations.rs` — migration 037
- `crates/relay/src/pool.rs` — добавить поле tags к AgentRecord
- `crates/relay/src/mcp.rs` — расширить flowlink_exec с tags
- Dashboard ServersContent — tag badges, filter bar

---

### Фаза 4: Audit Log Timeline UI ✅
**Статус:** РЕАЛИЗОВАНО

Audit Timeline реализован в Dashboard. API: `/api/v1/audit` с фильтрами, SIEM экспорт.
**Оценка:** 2 дня
**Зависимости:** нет

**Что добавить:**
- API: `/api/v1/audit?agent_id=&org_id=&from=&to=&event_type=` — пагинированный лог
- DB: таблица `audit_log` (уже есть approval_log, расширить или новая)
- Dashboard: страница Audit Timeline — визуальный таймлайн, фильтры, экспорт CSV/JSON
- Event types: exec, approval, shield_block, shield_warn, policy_change, agent_connect, agent_disconnect, login, api_key_created, api_key_revoked

**Интеграция:**
- Migration 038 (audit_log если отдельная от approval_log)
- `crates/relay/src/server.rs` — audit endpoints
- `crates/relay/src/handler.rs` — логирование событий
- Новый dashboard page: `app/dashboard/audit/`

---

### Фаза 5: Agent Health Monitoring
**Оценка:** 2-3 дня
**Зависимости:** Фаза 3 (tags)

**Что добавить:**
- Агент отправляет: CPU%, RAM%, disk%, uptime, load avg — каждые 60s
- DB: таблица `agent_metrics` (agent_id, timestamp, cpu, ram, disk, uptime, load_avg)
- API: `/api/v1/agents/{id}/metrics?from=&to=&interval=` — временные ряды
- MCP tool: `flowlink_agent_status` — текущие метрики
- Dashboard: графики CPU/RAM/Disk per agent, alert пороги
- Alert: уведомление если agent offline > N минут или CPU > 90%

**Интеграция:**
- `crates/agent/src/connection.rs` — отправка метрик по WS
- `crates/relay/src/pool.rs` — приём и хранение
- Migration 039
- `crates/relay/src/server.rs` — metrics endpoints
- Dashboard: новый компонент с графиками (chart.js или recharts)

---

### Фаза 6: Command Replay & Dry-Run
**Оценка:** 1-2 дня
**Зависимости:** нет

**Что добавить:**
- DB: таблица `command_history` (id, agent_id, command, args, exit_code, duration, shield_result, approval_id, executed_at)
- MCP tool: `flowlink_replay` — повторить команду с опциями: confirm (через approval), dry_run (только shield scan)
- MCP tool: `flowlink_history` — история команд по агенту с фильтрами
- Dashboard: страница Command History с replay/dry-run кнопками

**Интеграция:**
- `crates/relay/src/handler.rs` — логировать каждую exec в command_history
- `crates/relay/src/mcp.rs` — новые tools
- Migration 040
- Dashboard: новый page `app/dashboard/history/`

---

### Фаза 7: Interactive Session / Chat with Agent
**Оценка:** 3-4 дня
**Зависимости:** Фаза 6 (history)

**Что добавить:**
- MCP: интерактивная сессия через JSON-RPC — session ID, multistep команды
- Stdout streaming: exec возвращает output по мере выполнения (не только в конце)
- MCP tools: `flowlink_session_create`, `flowlink_session_exec`, `flowlink_session_close`
- Dashboard: терминал-виджет с live stdout (WebSocket → browser)

**Интеграция:**
- `crates/agent/src/executor.rs` — стримить stdout через WS
- `crates/relay/src/mcp.rs` — session management
- `crates/relay/src/handler.rs` — streaming exec
- Dashboard: WebSocket компонент для live terminal

---

### Фаза 8: Secrets Vault Integration ✅
**Статус:** РЕАЛИЗОВАНО

Zero-Trust Secret Injection через HashiCorp Vault. Секреты никогда не попадают в контекст агента.
**Оценка:** 2-3 дня
**Зависимости:** нет

**Что добавить:**
- MCP tools: `flowlink_secret_get`, `flowlink_secret_set`, `flowlink_secret_list`, `flowlink_secret_delete`
- Backend providers: HashiCorp Vault, 1Password Connect,環境変数 (env vars как fallback)
- Агент не хранит секреты — запрашивает у relay по необходимости
- Dashboard: UI для управления secret providers и маппингов

**Интеграция:**
- `crates/relay/src/secrets.rs` — новый модуль (provider trait + impls)
- `crates/relay/src/mcp.rs` — 4 новых tools
- Dashboard: `app/dashboard/secrets/`

---

### Фаза 9: Compliance Reports
**Оценка:** 2-3 дня
**Зависимости:** Фаза 4 (audit log)

**Что добавить:**
- API: `/api/v1/compliance/report?from=&to=&format=pdf|html|json`
- Данные: команды, violations, approvals, policy changes за период
- PDF генерация: rust crate (genpdf или printpdf)
- Шаблоны: стандартный + кастомный
- Auto-generate: ежемесячный отчёт по cron

**Интеграция:**
- `crates/relay/src/compliance.rs` — новый модуль
- Dashboard: кнопка "Скачать отчёт" на странице Audit
- Cargo зависимости: genpdf или аналогичный

---

### Фаза 10: GitOps Policy Deployment
**Оценка:** 2-3 дня
**Зависимости:** Policy Engine (готов)

**Что добавить:**
- YAML schema для политик (agent bindings, rules, approval modes)
- API: `/api/v1/gitops/config` — настройки Git repo, branch, webhook secret
- Webhook receiver: GitHub/GitLab push → парсинг YAML → apply к DB
- Diff preview: показать diff перед применением
- Rollback: версия политик в DB, откат на любую версию

**Интеграция:**
- `crates/gitops/` — расширить существующий крейт
- `crates/relay/src/policy_db.rs` — добавить versioning
- Dashboard: GitOps config page, diff viewer

---

### Фаза 11: Multi-Tenant Isolation
**Оценка:** 3-4 дня
**Зависимости:** Custom RBAC (Фаза 1), Tags (Фаза 3)

**Что добавить:**
- Org-level isolation: каждый org видит только свои агентов, политики, ключи
- Row-level security на уровне DB запросов (org_id filter)
- Resource quotas: макс агентов, ключей, политик per plan
- API rate limits per org (не только per key)
- Admin panel для super-admins

**Интеграция:**
- `crates/relay/src/middleware.rs` — org context extraction
- Все DB-запросы — добавить org_id фильтр
- `crates/relay/src/server.rs` — quota enforcement
- Migration 041 (org quotas)

---

### Фаза 12: Telegram Bot for Approvals ✅
**Статус:** РЕАЛИЗОВАНО

Telegram бот с inline кнопками approve/deny. Webhook mode.
**Оценка:** 1-2 дня
**Зависимости:** Webhooks (Фаза 2)

**Что добавить:**
- Telegram bot: receive approval requests, inline buttons approve/deny
- Binding: user ↔ Telegram chat_id
- API: `/api/v1/settings/telegram` — привязка ТГ аккаунта
- Commands: /approve, /deny, /status, /help
- Webhook mode (не polling)

**Интеграция:**
- `crates/relay/src/telegram_bot.rs` — новый модуль
- Вызовы из `approval.rs` → отправка в ТГ
- Dashboard: настройки page для привязки ТГ

---

### Фаза 13: Russian SIEM Connectors (RuSIEM / MaxPatrol) ✅
**Статус:** РЕАЛИЗОВАНО

RuSIEM (syslog UDP/TLS) и MaxPatrol (REST API) коннекторы реализованы.
**Оценка:** 2-3 дня
**Зависимости:** SIEM export (CEF/LEEF/JSON — готово)

**Что добавить:**
- RuSIEM connector: syslog UDP/TLS transport, формат событий поRuSIEM spec
- MaxPatrol SIEM connector: REST API push, формат JSON с маппингом полей
- Настройка: `/api/v1/integrations/siem` — выбор типа, endpoint, credentials
- Dashboard: SIEM integration config page
- Delivery status мониторинг

**Интеграция:**
- `crates/relay/src/audit.rs` — расширить с RuSIEM/MaxPatrol форматтерами
- `crates/relay/src/siem_delivery.rs` — async delivery manager
- Migration 042 (siem_configs)

---

### Фаза 14: ФСТЭК/152-ФЗ Compliance Mode ✅
**Статус:** РЕАЛИЗОВАНО

Compliance API с security_audit, policy_compliance, exec_summary, fstek endpoints.
**Оценка:** 3-5 дней
**Зависимости:** Audit Log (Фаза 4), SIEM (Фаза 13)

**Что добавить:**
- Compliance mode toggle: включает все требования 152-ФЗ автоматически
- Обязательное логирование: все команды, все изменения политик, все логины
- Хранение логов: минимум 1 год (настраиваемый retention)
- Цепочка доверия: HMAC подпись каждого лог-записа (неизменяемость)
- Журнал учёта: кто, когда, с какого IP, что сделал
- Аттестация: генерация пакета документов для ФСТЭК аттестации

**Интеграция:**
- `crates/relay/src/compliance.rs` — расширить с 152-ФЗ режимом
- `crates/db/` — audit log с HMAC, retention policy
- Migration 043
- Dashboard: compliance status page, attestation export

---

### Фаза 15: Yandex Cloud Integration
**Оценка:** 2-3 дня
**Зависимости:** нет

**Что добавить:**
- Yandex Cloud Container Registry: Docker image с агентом
- Yandex Compute: Terraform модуль для деплоя relay + agent
- Yandex IAM: авторизация через Yandex Passport/OAuth
- Marketplace: подготовка к публикации в Yandex Cloud Marketplace
- Docs: инструкция деплоя в Yandex Cloud

**Интеграция:**
- Dockerfile для агента + relay
- `terraform/yandex/` — Terraform конфигурация
- `crates/relay/src/auth.rs` — Yandex OAuth provider
- Docs: новая страница в wiki

---

## 🗓 Приоритетная очередь

| Порядок | Фича | Дней | Зависимости |
|---------|------|------|-------------|
| 1 | Custom RBAC Roles | 2-3 | — |
| 2 | Webhook Notifications | 2 | — |
| 3 | Agent Fleet Tags | 1-2 | — |
| 4 | Audit Log Timeline | 2 | — |
| 5 | Command Replay/Dry-Run | 1-2 | — |
| 6 | Agent Health Monitoring | 2-3 | Tags |
| 7 | Telegram Bot Approvals | 1-2 | Webhooks |
| 8 | Compliance Reports | 2-3 | Audit |
| 9 | GitOps Policies | 2-3 | Policy Engine |
| 10 | Secrets Vault | 2-3 | — |
| 11 | Multi-Tenant Isolation | 3-4 | RBAC, Tags |
| 12 | Interactive Sessions | 3-4 | History |
| 13 | RuSIEM/MaxPatrol | 2-3 | SIEM |
| 14 | ФСТЭК/152-ФЗ | 3-5 | Audit, SIEM |
| 15 | Yandex Cloud | 2-3 | — |

**Итого:** ~35-50 дней работы (1 фича = 1 сессия)

---

## 📝 Принципы

1. **Каждая фича = отдельная migration** — можно деплоить независимо
2. **Backward compatible** — новые фичи не ломают существующие API
3. **Dashboard + API одновременно** — фича не считается готовой без UI
4. **Tests** — минимум unit-тесты для нового модуля
5. **Docs** — обновить wiki при добавлении фичи

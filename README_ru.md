<p align="center">
  <img src="https://flowlink.flow-masters.ru/logo.svg" width="120" alt="FlowLink" />
</p>

<h1 align="center">FlowLink</h1>

<p align="center">
  <strong>MCP Gateway + AI-Native SecOps для агентов</strong><br/>
  Zero-trust контрольная плоскость между AI-агентами и вашей инфраструктурой.
</p>

<p align="center">
  <a href="#-быстрый-старт"><strong>Быстрый старт</strong></a> ·
  <a href="#-возможности">Возможности</a> ·
  <a href="#-архитектура">Архитектура</a> ·
  <a href="#-api-документация">API</a> ·
  <a href="#-тарифные-планы">Тарифы</a> ·
  <a href="README.md">English</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.80+-orange?logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/License-Proprietary-red.svg" alt="License" />
  <img src="https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker" alt="Docker" />
  <img src="https://img.shields.io/badge/tests-1187+-green" alt="Tests" />
</p>

---

**FlowLink** — zero-trust контрольная плоскость между AI-агентами и вашей инфраструктурой. Тот же класс что Envoy AI Gateway и Operant MCP Gateway — с фокусом на глубокую безопасность и runtime-контроль.

**Три столпа:**
- 🔍 **Visibility** — карта MCP-инструментов, живой аудит-лог, forensic timeline, pattern learning, карта инфраструктуры
- 🔑 **Governance** — Policy Engine, approval workflow, RBAC, zero-trust секреты, Shield-профили, SSO
- 🛡️ **Protection** — 7-уровневый Shield, eBPF перехват на уровне ядра, runtime-блокировки, redaction, sandbox, SIEM

🔗 **Разработано [FlowMasters](https://flow-masters.ru)** — чат-боты, ИИ-ассистенты и автоматизация для бизнеса.

---

## 🚀 Быстрый старт

### Установка одним скриптом

```bash
# Linux / macOS
curl -fsSL https://flowlink.flow-masters.ru/install.sh | sh
```

### Регистрация агента

```bash
flowlink agent register --name my-server
# ✓ Agent registered: ag_abc123
```

### MCP подключение

```json
// ~/.claude/mcp.json (или ~/.cursor/mcp.json)
{
  "mcpServers": {
    "flowlink": {
      "command": "flowlink",
      "args": ["mcp"]
    }
  }
}
```

### Проверка

```bash
flowlink version       # v0.3.1-dev
flowlink agent list    # ag_abc123  my-server  connected
flowlink mcp --test    # ✓ 12 tools available
```

📖 [Hello, Secure Agent — 10 минут до первой блокировки](https://flowlink.flow-masters.ru/docs/quickstart)

---

## 📡 Архитектура

```
    AI-агенты                    FlowLink                         Инфраструктура
  ┌───────────┐            ┌───────────────┐                 ┌──────────────────┐
  │ Claude    │            │  Cloudflare    │                 │  Серверы (Linux) │
  │ Cursor    │──MCP/WS───→│  CDN + DNS    │                 │  ┌────────────┐  │
  │ Copilot   │            └───────┬───────┘                 │  │ Agent      │  │
  │ Windsurf  │                    │                         │  │ (exec/IO)  │  │
  │ Codex     │            ┌───────▼────────┐                │  └────────────┘  │
  │ Cline     │            │  VPS (93.93)   │                │  ┌────────────┐  │
  │ Aider     │            │  ┌────┐ ┌─────┐│                │  │ ServerGuard│  │
  └───────────┘            │  │nginx│→│Relay││                │  │ (GitOps)   │  │
                           │  │(SSL)│ │:8080││                │  └────────────┘  │
                           │  └────┘ └──┬──┘││                └──────────────────┘
                           │   ┌────────▼──┐│
                           │   │ PostgreSQL ││                  ┌──────────────┐
                           │   │  :5432     ││                  │ K8s Cluster  │
                           │   └───────────┘│                  │ (CRD+Webhook)│
                           └────────────────┘                  └──────────────┘
```

**12 крейтов, ~158K строк Rust, ~1187 тестов.**

| Крейт | Назначение |
|-------|------------|
| `core` | Типы сообщений, конфигурация, каналы |
| `crypto` | X25519 + AES-256-GCM шифрование |
| `db` | PostgreSQL репозитории (sqlx) |
| `billing` | Планы, счета,.usage, Точка Банк |
| `agent` | Диспетчер, политики, sandbox, killswitch |
| `relay` | WS сервер, REST API, RBAC, E2EE, MCP |
| `shield` | eBPF/macOS ES, анализ угроз, L1-L7 |
| `gitops` | Drift detection, ServerGuard, backup |
| `k8s` | Operator, CRD, admission webhooks |
| `mcp` | MCP протокол (12 инструментов) |
| `sentinel` | AI Ops ассистент, pattern learning |
| `cli` | Бинарник, MCP сервер |

---

## 🛡️ Shield — 7 уровней защиты

```
KillSwitch → ReadOnly → Blacklist → Policy → Sandbox → Approval → Backup → Execute
```

| Уровень | Метод | Пример |
|---------|-------|--------|
| L0 KillSwitch | Экстренная блокировка | Агент на паузе |
| L1 Pattern | Regex матччинг | `rm -rf /`, `curl | bash` |
| L2 AST | Структурный разбор | `$(dangerous)`, globs |
| L3 Deep | Literal enforcement, tempo | Переписывание `chmod 777` → `755` |
| L4 Policy | Кастомные правила | Allow `apt update`, deny `apt remove` |
| L5 Sandbox | Изоляция выполнения | namespace/container |
| L6 Approval | Подтверждение человеком | Telegram alert → Approve/Reject |
| L7 Backup | Авто-сохранение | Снапшот перед разрушительными операциями |

---

## 🔐 Безопасность

### E2EE (опционально)
- **Алгоритм:** X25519 + AES-256-GCM
- Агенты без ключей работают в plaintext

### RBAC
- **admin** — полный доступ
- **operator** — управление агентами, просмотр биллинга
- **viewer** — только чтение

### Zero-Trust Secret Injection
- Секреты инжектятся через HashiCorp Vault
- Агент **никогда не видит** значение секрета
- Короткоживущие references, resolved server-side

### Device Trust Score
- 0-100, auto-deny < 20
- +10 за успешное подключение, -15 за failed attempt

---

## 📡 MCP инструменты (12)

| Инструмент | Описание |
|------------|----------|
| `flowlink_agents` | Список подключённых агентов |
| `flowlink_exec` | Выполнить команду на агенте |
| `flowlink_read` | Прочитать файл с агента |
| `flowlink_write` | Записать файл на агент |
| `flowlink_list` | Список директории на агенте |
| `flowlink_sysinfo` | Системная информация |
| `flowlink_kill` | Убить процесс на агенте |
| `flowlink_deregister` | Отключить агента |
| `flowlink_health` | Health check |
| `flowlink_config_update` | Обновить конфиг агента |
| `flowlink_approve` | Одобрить ожидающую команду |
| `flowlink_policy` | Управление политиками безопасности |

---

## 📊 Наблюдаемость

- **Audit trail** — тройная запись: память + JSONL + PostgreSQL
- **SIEM экспорт** — CEF, LEEF, JSON + RuSIEM + MaxPatrol
- **Карта инфраструктуры** — 80+ типов сервисов, живая топология
- **Discovery** — автоматический каталог сервисов
- **Forensic Timeline** — полная реконструкция инцидентов

---

## 📋 Governance

- **Approval workflow** — Block → Alert → Approve/Reject (Telegram, Dashboard)
- **Change Management** — отслеживание, согласование, аудит изменений
- **Compliance** — ФСТЭК/152-ФЗ, OWASP MCP risk mapping
- **SSO** — SAML 2.0 (Enterprise)

---

## ⚙️ GitOps

- **Config drift detection** — semantic diff текущего vs желаемого состояния
- **Auto-remediation** — классификация, auto-fix, backup перед exec
- **Circuit breaker** — tempo control для rate AI-агентов
- **ServerGuard** — file watching, Docker events, canary tokens

---

## ☸️ Kubernetes

- **CRD** — `FlowLinkShieldPolicy` для декларативной конфигурации
- **Operator** — reconciliation loop с обновлением статуса
- **Admission webhook** — MutatingWebhook (sidecar) + ValidatingWebhook (enforcement)

---

## 💰 Тарифные планы

| План | Цена | Серверы | Пользователи | Логи |
|------|------|---------|-------------|------|
| Starter | 4 990 ₽/мес | 2 | 2 | 14 дней |
| Pro «Популярный» | 39 990 ₽/мес | 10 | 10 | 90 дней |
| Business | 79 990 ₽/мес | 50 | 50 | 365 дней |
| Enterprise | по запросу | ∞ | ∞ | ∞ |

**Разница между планами:**

| Возможность | Starter | Pro | Business | Enterprise |
|-------------|---------|-----|----------|------------|
| MCP Gateway | ✅ | ✅ | ✅ | ✅ |
| Shield L1-L2 | ✅ basic | ✅ advanced | ✅ full | ✅ full |
| Policy Engine | ✅ | ✅ | ✅ | ✅ |
| Approval Workflow | — | ✅ | ✅ | ✅ |
| RBAC | — | ✅ | ✅ | ✅ |
| Pattern Learning | — | ✅ | ✅ | ✅ |
| Forensics | — | ✅ | ✅ | ✅ |
| SIEM Export | — | ✅ | ✅ | ✅ |
| SSO / SAML | — | — | ✅ | ✅ |
| AI Ops | — | — | ✅ | ✅ |
| Change Management | — | — | ✅ | ✅ |
| On-Premise | — | — | — | ✅ |

---

## 📚 API-документация

### Core

| Метод | Путь | Описание |
|-------|------|----------|
| GET | `/healthz` | Health check |
| GET | `/api/v1/agents` | Список агентов |
| POST | `/api/v1/agents/:id/commands` | Отправить команду |
| POST | `/api/v1/auth/signup` | Регистрация |
| POST | `/api/v1/auth/login` | Вход |

### Billing

| Метод | Путь | Описание |
|-------|------|----------|
| GET | `/api/v1/billing/usage` | Статистика использования |
| GET | `/api/v1/billing/plans` | Доступные планы |
| POST | `/api/v1/billing/subscribe` | Подписка на план |

### Security & Observability

| Метод | Путь | Описание |
|-------|------|----------|
| GET | `/api/v1/audit` | Аудит-события |
| GET | `/api/v1/audit/export?format=cef` | SIEM экспорт |
| GET | `/api/v1/audit/stats` | Статистика аудита |
| GET | `/api/v1/devices/:id/trust` | Trust score устройства |
| GET | `/api/v1/compliance/audit` | Compliance аудит |
| GET | `/api/v1/forensics/timeline` | Forensic timeline |

### GitOps

| Метод | Путь | Описание |
|-------|------|----------|
| GET | `/api/v1/gitops/drift/:id` | Статус drift |
| POST | `/api/v1/gitops/backup/:id` | Запустить backup |
| GET | `/api/v1/gitops/backups/:id` | Список backup'ов |
| POST | `/api/v1/gitops/restore/:id` | Восстановить из backup |
| GET | `/api/v1/gitops/guard/:id` | Статус ServerGuard |

Rate limit: 100 запросов / 10 секунд на IP.

---

## 🔗 Полезные ссылки

- 🌐 [Сайт](https://flowlink.flow-masters.ru)
- 📚 [Документация](https://flowlink.flow-masters.ru/docs)
- 💰 [Тарифы](https://flowlink.flow-masters.ru/pricing)
- 🎮 [Playground](https://flowlink.flow-masters.ru/playground)
- 🚀 [Quickstart](https://flowlink.flow-masters.ru/docs/quickstart)
- ⚖️ [Сравнение с конкурентами](https://flowlink.flow-masters.ru/docs/comparison)

---

## 📄 Лицензия

Proprietary — © 2026 FlowMasters.

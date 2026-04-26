# FlowLink — Master Plan to Production

**Дата:** 2026-04-26
**Версия:** v0.3.1-dev → v1.0.0
**Статус:** Active
**Язык:** Rust (12 crates, ~158K строк, ~1360 тестов)

---

## Текущий статус

### ✅ Готово (production-ready)

- **Relay** — WS сервер, REST API, RBAC, E2EE, MCP (12 tools), billing, auth (OAuth + 2FA + SAML)
- **Agent** — connection, exec, fileops, backup, policy, killswitch, approval, sandbox, pattern learning
- **Shield** — 7-уровневый pipeline (KillSwitch → ReadOnly → Blacklist → Policy → Sandbox → Approval → Backup → Execute)
- **Billing** — plans, invoices, usage, Tochka Bank payments
- **K8s Operator** — CRD + AdmissionWebhook + SidecarInjection + relay reporting
- **GitOps** — ServerGuard + BackupEngine + DriftDetector (feature-gated)
- **CLI** — agent, relay, mcp, gitops, policy, approve, devices, discover, doctor
- **Website** — 79 doc pages, 23 dashboard pages, RU/EN, playground, pricing

### 🟡 В разработке

- GitOps ServerGuard → agent event loop (wired, needs E2E testing)
- K8s Operator → production cluster testing
- E2E тесты с `--features gitops`

### 🔴 Не начато

- Custom RBAC Roles (ROADMAP Phase 1)
- Command Replay & Dry-Run (ROADMAP Phase 6)
- Interactive Sessions (ROADMAP Phase 7)
- Multi-Tenant Isolation (ROADMAP Phase 11)

---

## Планы и цены

| План | Цена | Серверы | Пользователи | Логи |
|------|------|---------|-------------|------|
| Starter | 4 990 ₽/мес | 2 | 2 | 14 дней |
| Professional | 39 990 ₽/мес | 10 | 10 | 90 дней |
| Scale | 79 990 ₽/мес | 50 | 50 | 365 дней |
| Enterprise | по запросу | ∞ | ∞ | ∞ |

---

## Архитектура

```
12 crates:
core → crypto → db → billing → agent → relay → shield → gitops → k8s → mcp → sentinel → cli
```

| Crate | Lines | Tests | Status |
|-------|-------|-------|--------|
| core | ~15K | 105 | ✅ stable |
| crypto | ~3K | 62 | ✅ stable |
| db | ~12K | 65 | ✅ stable |
| billing | ~8K | 72 | ✅ stable |
| agent | ~25K | 130 | ✅ stable |
| relay | ~35K | 222 | ✅ stable |
| shield | ~20K | 253 | ✅ stable |
| gitops | ~19K | 218 | 🟡 feature-gated |
| k8s | ~5K | 76 | 🟡 draft |
| mcp | ~3K | — | ✅ stable |
| sentinel | ~5K | — | ✅ stable |
| cli | ~8K | — | ✅ stable |

---

## Путь к v1.0.0

### P0 — Stability (1-2 недели)
1. E2E тесты с gitops feature
2. K8s operator в test cluster
3. Load testing relay (1000+ concurrent WS)
4. Security audit (dependency scan, cargo audit)

### P1 — Polish (2-4 недели)
5. Custom RBAC Roles
6. Command Replay & Dry-Run
7. Interactive Sessions (streaming exec)
8. Multi-Tenant Isolation

### P2 — Growth (1-2 месяца)
9. Yandex Cloud Marketplace
10. Terraform modules
11. Helm chart
12. Public API docs (OpenAPI/Swagger)

---

## Деплой

```bash
# Production (VPS 93.93.207.44)
./scripts/deploy.sh

# С GitOps
cargo build --release --features gitops

# K8s
kubectl apply -f config/crd.yaml
./flowlink-k8s --relay-url https://flowlink.flow-masters.ru
```

---

## Сайт

- **URL:** https://flowlink.flow-masters.ru
- **Репо:** /Users/braincoder/Projects/flowlink-website
- **Технология:** Next.js + Tailwind + i18n (RU/EN)
- **Деплой:** pm2 на VPS + Cloudflare CDN

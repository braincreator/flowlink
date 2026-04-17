# FlowLink — Дорожная карта

> Обновлено: 2026-04-17

---

## 🔴 ТЕКУЩИЕ ПРОБЛЕМЫ

### 1. Sessions page (фронтенд) — заглушка
**Сейчас:** Sessions.tsx подключается к `api.getSessions()` → `/api/sessions` (relay terminal sessions), а НЕ к `/api/auth/sessions` (JWT auth sessions).
**Нужно:** Новая страница или вкладка для auth sessions (JWT), с endpoint `/api/auth/sessions`.

### 2. 2FA verify в login flow
**Сейчас:** Login.tsx проверяет `data.requires_2fa` и вызывает `setTwoFATempToken()`, но UI ввода 6-значного кода НЕ рендерится — токен просто сохраняется в localStorage и login висит.
**Нужно:** Показывать форму ввода TOTP кода после `requires_2fa`, вызвать `/api/auth/2fa/complete` с temp_token + code.

### 3. RBAC page — заглушка
**Сейчас:** RBAC.tsx подключается к `api.getRbacUsers()` → `/api/rbac/users` (endpoint не существует в backend). Page показывает mock/skeleton.
**Нужно:** Это про управление доступом внутри dashboard. Сейчас нет понятия "команда/организация" — каждый аккаунт независим. Нужно либо: (a) убрать страницу пока нет org концепции, либо (b) реализовать org/team.

### 4. Billing page — частично работает
**Сейчас:** Backend billing endpoints есть (plans, invoices, subscriptions, orders). Frontend показывает 3 тарифа (hardcoded) но не дергает реальные данные.
**Нужно:** Подключить к реальному API, показать текущий план юзера.

### 5. Onboarding — привязан к relay, не к dashboard auth
**Сейчас:** 4 шага: подключить relay → deploy agent → create policy → done. Не учитывает что юзер может быть уже залогинен через OAuth/email.
**Нужно:** Адаптировать под dashboard auth flow.

### 6. Email verification codes — нет очистки
**Сейчас:** Коды пишутся в `email_verification_codes`, но нет TTL/очистки старых.

---

## 🟡 АРХИТЕКТУРНЫЕ ПРОБЛЕМЫ

### Нет понятия организации/команды
- `accounts` = 1 аккаунт = 1 юзер (или 1 Telegram bot)
- `users` таблица привязана к `account_id` (1:many), но не используется в auth
- `tenants` таблица есть (legacy от Telegram), не связана с dashboard auth
- RBAC невозможен без multi-user → нужен концепт workspace/org

### Нет связи accounts ↔ agents
- `agents` таблица есть, но не привязана к account_id
- Юзер не видит "свои" агенты

### Plans — статические
- 3 плана в DB, но нет UI для админа чтобы их редактировать
- Нет auto-trial → юзеры создаются с `plan_id='free'` (а не 'trial')

---

## 📋 ДОРОЖНАЯ КАРТА

### Фаза 1: Починка текущего (2-3 дня)

#### 1.1 Auth Sessions UI
- [ ] Новая страница `/dashboard/security/sessions` (или вкладка в Profile)
- [ ] Подключить к `/api/auth/sessions` (GET/DELETE)
- [ ] Таблица: IP, UserAgent, created_at, actions (revoke)
- [ ] Кнопка "Завершить все другие сессии"

#### 1.2 2FA Verify в Login
- [ ] После `requires_2fa: true` — показать форму ввода кода
- [ ] Вызов `/api/auth/2fa/complete` с `temp_token` + `code`
- [ ] Обработка ошибок (неверный код → retry)
- [ ] Кнопка "Назад" (отмена login)

#### 1.3 Plans CRUD в админке
- [ ] GET/POST/PUT/DELETE `/api/admin/plans`
- [ ] Таблица: id, name, price, limits (JSON), features, is_active
- [ ] Форма редактирования limits/features
- [ ] Seed: auto-create trial/free/starter/pro если пусто

#### 1.4 Invoices + Subscriptions в админке
- [ ] Вкладка в Admin: список всех подписок (account, plan, status, dates)
- [ ] Вкладка: список заказов (account, amount, status, paid_at)
- [ ] Фильтры по аккаунту, статусу, дате

#### 1.5 Billing page — подключить к реальному API
- [ ] Показать текущий план из `/api/billing`
- [ ] Показать реальные features/limits
- [ ] Кнопка upgrade → Tochka payment

---

### Фаза 2: Организации и команды (3-5 дней)

#### 2.1 DB Schema: Organizations
```
organizations
  - id (UUID)
  - name
  - slug (unique)
  - owner_account_id → accounts
  - plan_id → plans
  - limits (JSONB)
  - created_at

org_members
  - id (UUID)
  - org_id → organizations
  - account_id → accounts
  - role (owner, admin, member, viewer)
  - invited_by → accounts
  - joined_at
  - invited_email

org_invitations
  - id (UUID)
  - org_id → organizations
  - email (nullable)
  - role (admin, member, viewer)
  - token (unique)
  - expires_at
  - accepted_by → accounts (nullable)
  - accepted_at
  - created_at
```

#### 2.2 Backend
- [ ] CRUD `/api/orgs` (create, list, get, update, delete)
- [ ] Members `/api/orgs/{id}/members` (list, invite, remove, change role)
- [ ] Invitations `/api/org/invite` (send email), `/api/org/accept` (by token)
- [ ] Switch context: `/api/org/switch` — юзер может быть в нескольких orgs
- [ ] Enforce limits: кол-во members ≤ plan.max_users, agents ≤ plan.max_hosts

#### 2.3 Frontend
- [ ] Org switcher в header (dropdown с текущей org)
- [ ] Settings → Organization: name, members, invites, billing
- [ ] Invite flow: ввести email → отправить → принять по ссылке
- [ ]RBAC: owner/admin/member/viewer роли внутри org

#### 2.4 Onboarding 2.0
- [ ] После login: если нет org → "Create organization" (или join по invite link)
- [ ] Выбрать план (free trial)
- [ ] Invite members (optional)
- [ ] Deploy first agent
- [ ] Done → dashboard

---

### Фаза 3: Полноценный Billing (2-3 дня)

#### 3.1 Trial → Paid
- [ ] Auto-trial при создании org (7 дней)
- [ ] Trial bar: "Осталось X дней, upgrade now"
- [ ] Grace period после trial (read-only, 3 дня)
- [ ] Auto-downgrade to free при expiry

#### 3.2 Payment flow
- [ ] Tochka payment integration (уже частично есть)
- [ ] Invoice generation при каждом payment
- [ ] Webhook processing (tochka)
- [ ] Receipt/акт в личном кабинете

#### 3.3 Usage tracking
- [ ] `usage_daily` — уже есть таблица, нужен periodic counter
- [ ] Показать usage vs limits в Billing page
- [ ] Rate limiting по limits (кол-во агентов, юзеров, запросов)

---

### Фаза 4: Security & Polish (1-2 дня)

#### 4.1 Auth improvements
- [ ] Password reset flow (email)
- [ ] Password change (Profile)
- [ ] Email change verification
- [ ] Device fingerprinting
- [ ] Login notifications (email/telegram)

#### 4.2 API improvements
- [ ] Rate limiting на auth endpoints
- [ ] CORS headers (если нужен)
- [ ] API versioning

#### 4.3 Frontend polish
- [ ] Loading states для всех страниц
- [ ] Error boundaries
- [ ] Mobile responsive (текущий sidebar = hamburger)
- [ ] Dark/light theme toggle (уже есть в settings)

---

## 📊 ПРИОРИТЕТЫ

| # | Задача | Сложность | Влияние |
|---|--------|-----------|---------|
| 1 | 2FA verify в login flow | Low | Critical (фича сломана) |
| 2 | Auth Sessions UI | Low | Medium |
| 3 | Plans CRUD admin | Medium | High (бизнес) |
| 4 | Admin invoices/subs | Low | High |
| 5 | Billing page real data | Low | High |
| 6 | Organizations | High | Critical (фундамент) |
| 7 | Invite flow | Medium | High |
| 8 | Onboarding 2.0 | Medium | High |
| 9 | Trial automation | Medium | High |
| 10 | Usage tracking | Medium | Medium |

---

## 🏗️ АРХИТЕКТУРНЫЕ РЕШЕНИЯ

### Два Dashboard
- **Relay Dashboard (SPA)** — техническая панель для self-hosted: admin, shield, audit, metrics, billing. Встроена в binary.
- **Website Dashboard (Next.js)** — пользовательский кабинет: лендинг, pricing, signup, profile, security. Для SaaS.
- Обе подключаются к одному relay API (`/api/*`)

### Auth → Org mapping
- `account` = личность (человек), может быть в нескольких orgs
- `org` = workspace/компания с планом и лимитами
- JWT содержит `current_org_id` (переключаемый)
- Middleware проверяет org membership

### RBAC Roles
- **owner** — полный контроль, billing, delete org
- **admin** — управление members, policies, agents
- **member** — управление своими агентами, view policies
- **viewer** — только чтение

### Plans → Orgs (не Accounts)
- Plan привязан к org, не к account
- Юзер может быть в free org и в pro org одновременно
- Billing = org-level

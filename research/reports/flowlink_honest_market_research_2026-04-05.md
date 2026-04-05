# FlowLink — Честный Market Research

> Дата: 2026-04-05 | Источник: реальные данные сайтов, GitHub API, знание рынка

---

## 1. Конкуренты (РЕАЛЬНЫЕ данные)

### Прямые конкуренты — "remote agent execution"

| Продукт | Что делает | Цены | GitHub | РФ? |
|---------|-----------|-------|--------|-----|
| **E2B** | Sandbox-окружения для AI agents (code execution) | $21-250/mo | 11.5K ⭐ | Нет |
| **Modal** | Serverless GPU/compute для ML | Pay-per-use (сотые $/час) | N/A (closed) | Нет |
| **Fly.io** | Деплой приложений | $5-27+/mo | 9K ⭐ | Ограниченно |
| **Render** | Хостинг + background workers | $7-85+/mo | 10K ⭐ | Нет |

### Косвенные конкуренты — "workflow / automation"

| Продукт | Что делает | Цены | GitHub | РФ? |
|---------|-----------|-------|--------|-----|
| **n8n** | Workflow automation, self-hosted/cloud | $0 (self) / $20-250 (cloud) | **182K ⭐** | Self-host да |
| **Dify** | LLM app builder | $0 (self) / $59-159/mo (cloud) | **136K ⭐** | Self-host да |
| **Windmill** | Workflow automation (dev-first) | $0 (self) / $10/seat (cloud) | 16K ⭐ | Self-host да |
| **Temporal** | Workflow orchestration engine | $0 (self) / custom (cloud) | 19K ⭐ | Self-host да |
| **Prefect** | Data workflow orchestration | $0 (self) / custom (cloud) | 22K ⭐ | Self-host да |

### Ключевой вывод по конкурентам

**У FlowLink НЕТ прямых конкурентов с таким же позиционированием.**

- E2B — ближе всего (sandbox для агентов), но это cloud-only, нет relay, нет self-hosted
- n8n/Dify — workflow builders, НЕ relay для remote command execution
- Windmill — ближе всего по духу (dev-first), но это workflow automation, не agent relay

**FlowLink уникален в комбинации:** WebSocket relay + self-hosted agent + security sandbox + multi-tenant + РФ-фокус

---

## 2. Что на самом деле делает FlowLink (честно)

FlowLink = **WebSocket relay для удалённого выполнения команд через AI agents**

Это НЕ:
- ❌ LLM app builder (не Dify)
- ❌ Workflow automation (не n8n)
- ❌ Orchestration engine (не Temporal)
- ❌ Code sandbox API (не E2B)

Это:
- ✅ Реле между AI-агентом и целевой машиной
- ✅ Multi-tenant управление агентами
- ✅ Audit log + rate limiting + billing
- ✅ Self-hosted deployment

---

## 3. TAM/SAM — ЧЕСТНАЯ оценка

### TAM (World)
"Agent infrastructure" — это не отдельный рынок. Это часть:
- DevOps tools market: ~$15B (2025)
- AI infrastructure: ~$25B (2025)
- Но FlowLink берёт микронишу внутри них

**Реалистичный TAM: ~$500M-$1B** (agent execution infrastructure, не весь AI)

### SAM (Россия)
- AI/ML рынок РФ: ~58 млрд ₽ (2025) — это ВЕСЬ рынок
- Из них "инфраструктура для агентов" — максимум 2-5%
- **Реалистичный SAM: ~1-2 млрд ₽** — И ЭТО ОЧЕНЬ ОПТИМИСТИЧНО

### SOM (достижимая доля)
- У FlowLink нет marketing, нет brand, 0 клиентов
- Год 1: 10-50 клиентов реалистично при активном маркетинге
- **Реалистичный SOM Year 1: ~$500-5000 ARR**

### ⚠️ Важно
**SAM 1.5-2 млрд ₽ из прошлого ресерча — НЕВОЗМОЖНО достичь.** Это как сказать "рынок еды в Москве = $10B, наш SOM = $100M". Рынок есть, но не для этого продукта.

---

## 4. Спрос — что мы РЕАЛЬНО знаем

### Положительные сигналы:
- E2B привлёк $32M funding — значит спрос на "secure code execution for AI agents" есть
- n8n 182K stars — огромный интерес к automation
- Dify 136K stars — огромный интерес к LLM tools
- Хабр/VC.ru — регулярные статьи про AI agents

### Негативные сигналы:
- НЕТ Product Hunt launches в этой нише с высоким рейтингом
- НЕТ обсуждений "нужен relay для агентов" на Хабре/TG
- E2B — единственный well-funded игрок, и он cloud-only (санкции для РФ)
- Большинство российских компаний используют самописные решения

### Вывод: спрос есть, но он LATENT (скрытый)
Люди пока не знают, что им нужен "agent relay". Они решают проблему через:
- SSH + custom scripts
- n8n self-hosted + SSH nodes
- Telegram bots + direct SSH
- Custom WebSocket servers

---

## 5. Ценообразование — ЧЕСТНО

### Что стоят конкуренты:

| Продукт | Min | Mid | Max |
|---------|-----|-----|-----|
| E2B | $21/mo | ~$100/mo | $250+/mo |
| n8n Cloud | $20/mo | $50/mo | $250+/mo |
| Dify Cloud | $59/mo | $159/mo | Custom |
| Windmill | $10/seat | ~$50/seat | Custom |

### FlowLink сейчас:
- Starter: $19/mo (3 agents)
- Pro: $49/mo (25 agents)

### Честная оценка:
**Цены АДЕКВАТНЫЕ для рынка.** Ниже Dify, на уровне n8n, выше Windmill (per seat).

**НО:** поднимать цены при 0 клиентах = самоубийство. Сначала traction, потом price increase.

---

## 6. Что РЕАЛЬНО мешает (не trial и не доки)

### 🔴 Проблема #1: Нет product-market fit
Нет 3-5 клиентов, которые ПОЛЬЗУЮТСЯ продуктом платно. Без этого всё остальное — пустая трата.

### 🔴 Проблема #2: Нет clear value proposition
"WebSocket relay для AI agents" — это непонятно 95% потенциальных клиентов.
Нужен конкретный use case: "Запускай команды на 100 серверах через Telegram за 5 минут"

### 🔴 Проблема #3: Нет distribution channel
Как люди узнают о FlowLink?
- Хабр? Одна статья = ~500-2000 просмотров, ~5-20 signups
- TG каналы? Нужны контакты
- GitHub? 0 stars сейчас
- Product Hunt? Можно попробовать

### 🟡 Проблема #4: "Build it and they will come" — не работает
Нужен активный outreach. Поиск конкретных людей/компаний, которым это нужно.

---

## 7. ЧТО ДЕЛАТЬ (приоритеты)

### Неделя 1-2: Валидация
1. Написать 3 конкретных use case (с примерами "до/после")
2. Найти 10 людей через TG/Habr, которые решают похожую задачу
3. Показать им продукт, собрать feedback
4. Цель: хотя бы 1 человек скажет "да, я бы заплатил"

### Неделя 3-4: MVP Landing
1. Простая landing page с 1 use case
2. CTA: "Оставь email" (не "купи", а "подпишись")
3. Запустить на Хабр + 2-3 TG канала
4. Цель: 50 email signups, 5-10 demo requests

### Месяц 2: Traction
1. Добавить trial (да, это важно)
2. Onboard 3-5 beta-клиентов
3. Собрать testimonials
4. Цель: 1-3 paying customers

### НЕ ДЕЛАТЬ сейчас:
- ❌ Поднимать цены (нет клиентов)
- ❌ Писать документацию (нет юзеров)
- ❌ Добавлять фичи (нет валидации)
- ❌ Ресерч рынка (уже сделан)

---

## 8. Bottom line

| Вопрос | Ответ |
|--------|-------|
| Есть ли рынок? | Да, но маленький и latent |
| Есть ли конкуренты? | Нет прямых, есть косвенные |
| Уникальность? | Да (relay + self-hosted + РФ) |
| Готов ли продукт? | Технически да, коммерчески нет |
| Главная проблема? | Нет PMF — никто не платит |
| Главный риск? | Рынок слишком маленький для бизнеса |

**Честный прогноз ARR через 12 мес при активной работе: $2,000-10,000**
(10-50 клиентов × $20-200/mo, конверсия 2-5% из signups)

**Не $50K+ как в прошлом ресерче — это нереально без marketing team и funding.**

# FlowLink — GTM & Growth Plan

> Дата: 2026-04-22
> Статус: Actionable plan на 6-8 недель

---

## 1. Value-предложение

**Формула:** «Falco для AI-агентов»

eBPF-gateway, который перехватывает syscalls от команд, пришедших из Claude Code / Cursor / Copilot и других MCP-клиентов, и применяет 7-уровневый security-pipeline:
1. Kill-switch
2. Read-only
3. Blacklist
4. Policy engine
5. Sandbox
6. Approval workflow
7. Backup-then-execute

### Threat Model (для лендинга)

| Класс угрозы | Пример | Уровень защиты |
|---|---|---|
| Destructive actions | `rm -rf /`, `DROP TABLE` | Kill-switch, Blacklist |
| Data exfiltration | `cat .env \| curl`, SSH туннели | Policy, Approval |
| Lateral movement | `ssh -R`, port forwarding | Policy, Sandbox |
| Resource abuse | Crypto mining, fork bombs | Blacklist, Kill-switch |
| Privilege escalation | `chmod 777`, `sudo` | Policy, Approval |

### Архитектурная схема (для лендинга)

```
IDE/Agent → MCP → FlowLink → eBPF (kernel) → Сервер
                         ↓
                   7-Level Pipeline
                         ↓
              Block / Sandbox / Approve / Execute
```

**Ключевой посыл:** FlowLink работает на уровне ядра через eBPF и не может быть обойдён shell-хаками.

---

## 2. Тарифы

### Текущая сетка (ок для старта)

| План | Цена | Серверы | Пользователи | Логи |
|---|---|---|---|---|
| Free «Знакомство» | 0 ₽ | 1 | 1 | 30 дней |
| Pro «Профессионал» | 1 990 ₽/мес | 5 | 5 | 60 дней |
| Scale «Масштаб» | 4 990 ₽/мес | 20 | 10 | 90 дней |

### Что добавить

1. **K8s маппинг** — пояснить что «серверы» = worker ноды в кластере
2. **Enterprise кнопка** — «>20 серверов / >1 кластера / >90 дней логов», SSO, кастомные интеграции, прайс $2-5k/мес
3. **Годовые планы** — 10-20% скидка (закрепляет прогретых клиентов)
4. **Dev/Sandbox позиционирование** для Free — «безопасный режим наблюдения для dev/stage»

---

## 3. GTM план — 6-8 недель

### Недели 1-2: Упаковка и доки

- [ ] Добавить Threat Model секцию на лендинг
- [ ] Мини-таблица «тип команды → уровень защиты»
- [ ] Гайд «How to protect Claude Code» в /docs
- [ ] Гайд «How to protect Cursor» в /docs
- [ ] FAQ: сравнение с Falco/Tetragon/Cilium
- [ ] Архитектурная SVG-схема

### Недели 3-4: Developer-led рост

**Цель:** 10-20 активных Free-установок

- [ ] Тех-статья: «Защита LLM-агентов за <1мс: eBPF-gateway между Claude Code и вашим сервером»
- [ ] Скринкаст: Claude Code → опасная команда → FlowLink блокирует
- [ ] Каждые материал → playground (без регистрации) + Free план
- [ ] Публикация: Habr, Telegram-каналы, Reddit r/LocalLLaMA

### Недели 5-8: Design-партнёры

**Цель:** 3-5 платящих команд на Pro/Scale

- [ ] Список компаний с публичными AI-agent инициативами (банки, финтех, SaaS)
- [ ] Персонализированные outreach:
  > «Мы сделали eBPF-gateway для AI-агентов. 2-4 недели пилот — покажу отчёт, что агенты реально делают»
- [ ] Pilot условия: 1-2 месяца, метрики (опасные команды, покрытые сервера)
- [ ] Формализованные кейсы после первых 2-х платящих

---

## 4. Чего НЕ делать сейчас

- ❌ Не усложнять биллинг по командам/LLM-запросам
- ❌ Не продавать SMB «ради моды на AI»
- ❌ Не тратить время на enterprise-фичи (SSO, сложные интеграции) до явного запроса
- ❌ Не перепридумывать продукт — докручивать позиционирование вокруг существующего

---

## 5. Формула роста

```
Dev'и через playground/статьи
    ↓
CTO/команды → Pro/Scale (1 990/4 990 ₽/мес)
    ↓
2-3 design-партнёра → кастомный прайс (x3-5 от Scale)
    ↓
Кейсы → Enterprise уровень
```

---

## 6. Институциональные возможности

### Сколково
- Гранты: 1.5-30М ₽
- Налоговые льготы: 0% прибыль, сниженные страховые взносы
- Требуется: юрлицо, технологический проект
- Заявка: sk.ru/applicants-actions/

### Реестр Минцифры
- Госзакупки без конкуренции
- Требования: российское ПО, совместимость

---

## 7. Конкурентное позиционирование

| Решение | Фокус | Разница с FlowLink |
|---|---|---|
| Falco | Контейнеры | Следит за контейнерами, не за AI-агентами |
| Tetragon | eBPF security | Kernel-level, но не знает про MCP/LLM |
| Cilium | Networking | CNI, не security gateway |
| Prompt Security | LLM I/O | Проверяет промпты, не системные вызовы |
| **FlowLink** | **AI agents → syscalls** | **eBPF + MCP-aware + 7-level pipeline** |

**Уникальная ниша:** единственный eBPF-gateway, специализированный на AI-агентах.

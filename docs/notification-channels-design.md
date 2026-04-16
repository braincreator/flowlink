# Notification Channels — Full Design

## User Journey

### 1. Привязка канала (Binding Flow)

Каждый канал имеет свой flow, но финальный шаг одинаковый:
`upsert(account_id, channel_type, channel_address) → verify → set_primary?`

#### Telegram
```
Пользователь → /settings в TG боте
Бот → "Ваши каналы уведомлений:\n\n1. 🔵 Telegram (основной) ✅\n2. ⬜ MAX Messenger\n3. ⬜ Slack\n\n[Привязать MAX] [Привязать Slack]"
```
При первом /start → автоматическая привязка TG:
```
User: /start
Bot: upsert(account_id, "telegram", chat_id, verified=true, is_primary=first_binding)
```

#### MAX Messenger
```
User: [Привязать MAX]
Bot: "Отправьте код ниже боту MAX: 847291\n\nБот MAX поддержит: /link 847291"
User: (идёт в MAX, пишет /link 847291)
MAX Bot: → upsert(account_id, "max", max_user_id, verified=true)
MAX Bot: "✅ FlowLink привязан!"
→ callback в TG: "✅ MAX Messenger привязан"
```

#### Slack
```
User: [Привязать Slack]
Bot: "Откройте: https://flowlink.flow-masters.ru/api/notifications/slack/install"
→ OAuth flow → Slack webhook URL → upsert(account_id, "slack", webhook_url, verified=true)
```

#### Email
```
Всегда привязан автоматически из account.email (при регистрации).
EmailChannel — не в user_notification_channels, а в accounts.email напрямую.
```

### 2. Настройки каналов (Per-Channel Settings)

Каждый binding в `user_notification_channels` имеет:

| Поле | Описание | UI |
|------|----------|-----|
| `is_primary` | Основной канал (критичные always) | Toggle/radio |
| `verified` | Привязка подтверждена | Badge ✅/⏳ |
| `mute_categories` | Замьютить категории | Чекбоксы: ☐ Shield ☑ System ☐ Billing ☐ Audit ☐ Agent |
| `min_severity` | Мин. уровень уведомлений | Radio: ☐ Info ☐ Warning ☑ Alert ☐ Critical |

#### TG Bot UI для настроек

```
/settings
┌─────────────────────────────────────┐
│ 📢 Настройки уведомлений           │
│                                     │
│ Каналы:                             │
│ 1. 🔵 Telegram       ✅ основн.     │
│    Уровень: Alert                     │
│    Мьют: System                      │
│    [Настроить] [Убрать]              │
│                                     │
│ 2. 💬 MAX Messenger  ✅              │
│    Уровень: Warning                  │
│    Мьют: (нет)                       │
│    [Настроить] [Убрать]              │
│                                     │
│ [➕ Привязать канал]                 │
└─────────────────────────────────────┘
```

Нажимает "Настроить" на канале:
```
🔔 Настройки: Telegram

Минимальный уровень:
[Info] [Warning] [Alert ✅] [Critical]

Замьютить категории:
[☐ Shield] [☑ System] [☐ Billing] [☐ Audit] [☐ Agent]

[⬅️ Назад] [Сохранить]
```

Inline buttons с callback data:
- `notif:level:telegram:warning` — set min_severity
- `notif:mute:telegram:shield` — toggle category mute
- `notif:primary:telegram` — set as primary
- `notif:unbind:telegram` — remove channel
- `notif:bind:max` — start MAX binding flow
- `notif:bind:slack` — start Slack binding flow

### 3. REST API Endpoints

```
GET    /api/notifications/channels          — list user's channels
POST   /api/notifications/channels          — bind new channel
DELETE /api/notifications/channels/:id       — unbind channel
PATCH  /api/notifications/channels/:id       — update settings
POST   /api/notifications/channels/:id/verify — verify binding
POST   /api/notifications/channels/:id/primary — set as primary
POST   /api/notifications/test              — send test notification
```

#### POST /api/notifications/channels
```json
{
  "channel_type": "telegram",
  "channel_address": "477112098",
  "display_name": "Aleksandr"
}
```

#### PATCH /api/notifications/channels/:id
```json
{
  "min_severity": "warning",
  "mute_categories": ["system", "audit"],
  "is_primary": false
}
```

### 4. Test Notification

`POST /api/notifications/test` → отправляет тестовое уведомление на все привязанные каналы:
```
ℹ️ [Test] FlowLink уведомления работают! ✅
Канал: Telegram
Время: 2026-04-16 11:00:00
```

### 5. DB Schema (migration 016)

```sql
CREATE TABLE user_notification_channels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(255) NOT NULL REFERENCES accounts(id),
    channel_type VARCHAR(30) NOT NULL,       -- telegram, max, slack, webhook
    channel_address VARCHAR(255) NOT NULL,    -- chat_id, user_id, webhook_url
    display_name VARCHAR(255),
    is_primary BOOLEAN DEFAULT FALSE,
    verified BOOLEAN DEFAULT FALSE,
    mute_categories JSONB DEFAULT '[]',       -- ["system", "audit"]
    min_severity VARCHAR(20) DEFAULT 'info',  -- info, warning, alert, critical
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(account_id, channel_type, channel_address)
);
```

### 6. NotificationRouter: Full Resolution Flow

```
notification.send()
│
├─ 1. Resolve target
│   ├─ account_id present → DB: user_notification_channels
│   ├─ account_id empty   → global channels (FLOWLINK_NOTIFY_* env)
│   └─ account_id + global_fallback tag → both
│
├─ 2. Filter per binding
│   ├─ verified = false → skip
│   ├─ severity < min_severity → skip
│   └─ category in mute_categories → skip
│
├─ 3. Deliver
│   ├─ find channel impl by channel_type
│   ├─ channel.deliver_to(address, notification)
│   └─ log failures, don't propagate
│
└─ 4. Return ok count
```

### 7. Auto-bind on TG /start

В `cmd_start()` → после нахождения/создания аккаунта:
```
upsert(account_id, "telegram", chat_id.to_string(), display_name, is_primary=no_other_primary)
```

### 8. Implementation Checklist

- [x] DB migration (016_user_notification_channels)
- [x] UserChannelRepo (upsert, list, verify, delete, set_mute, set_severity)
- [x] Notification trait (channel_type, deliver_to)
- [x] NotificationRouter (per-user + global, DB-aware)
- [x] TelegramChannel implementation
- [x] Shield → per-user via agent_id → account_id
- [x] Approval → per-user via agent_id → account_id
- [ ] REST API endpoints (/api/notifications/*)
- [ ] TG bot commands (/settings, /notif_test, inline settings UI)
- [ ] Auto-bind TG on /start
- [ ] Test notification endpoint
- [ ] NotificationChannel for email (via EmailService)
- [ ] MAX Messenger channel (future)
- [ ] Slack channel (future)

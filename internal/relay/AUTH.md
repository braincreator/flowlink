# Auth Module — Аутентификация и Авторизация

## Обзор

Модуль предоставляет JWT-like аутентификацию на HMAC-SHA256 без внешних зависимостей.

## Компоненты

### 1. AuthManager (`auth.go`)

**Функции:**
- `GenerateAgentToken(agentID, expiresInSeconds)` — генерирует pairwise токен для агента
- `ValidateAgentToken(agentID, token)` — проверяет токен агента
- `GenerateAPIToken(clientID, expiresInSeconds)` — генерирует API токен для HTTP клиента
- `ValidateAPIToken(token)` — проверяет API токен, возвращает client_id
- `RotateTokens(agentID, expiresInSeconds)` — ротация токена (инвалидация старых)
- `RevokeToken(token)` — отзыв токена

**Формат токена:**
```
base64(JSON(Token)) "." signature
```

### 2. RateLimiter (`auth.go`)

**Лимиты:**
- 30 команд/минута
- 200 команд/час

**Алгоритм:** Sliding window на sync.Map

**Методы:**
- `Check(clientID)` — проверяет лимит, возвращает (allowed, retryAfter)

### 3. Middleware (`middleware.go`)

**Цепочка middleware:**
```go
Chain(
    RecoveryMiddleware,    // Восстановление после panic
    RequestLoggerMiddleware, // Логирование запросов (audit)
    CORSMiddleware,        // CORS для MCP
    RateLimitMiddleware,   // Rate limiting
    AuthMiddleware,        // Аутентификация
)
```

**AuthMiddleware:**
- Проверяет токены через AuthManager (динамические)
- Fallback на статический токен из конфига
- Добавляет `X-Client-ID` в заголовки

**RateLimitMiddleware:**
- Возвращает 429 при превышении
- Заголовки: `Retry-After`, `X-RateLimit-Limit`

**CORSMiddleware:**
- Разрешает все origins (можно ограничить)
- Поддержка preflight OPTIONS

**RequestLoggerMiddleware:**
- Логирует все запросы (кроме /health, /metrics)
- Audit trail: method, path, status, duration, client_id, remote_addr

## Интеграция в Relay

```go
// Создание Relay
relay := NewRelay(cfg)

// Публичные методы для управления токенами
token, err := relay.GenerateAgentToken("agent-123", 0) // бессрочный
apiToken, err := relay.GenerateAPIToken("client-456", 86400) // 24 часа
err := relay.RevokeToken(token)
newToken, err := relay.RotateAgentTokens("agent-123", 0)
```

## Использование

### Генерация токена для агента
```bash
# Через HTTP API (если добавите endpoint)
POST /api/v1/tokens/generate
{
  "client_id": "agent-123",
  "type": "agent",
  "expires_in": 0
}
```

### Подключение агента
```bash
# WSS handshake с токеном
wss://relay.example.com/ws
{
  "type": "connect",
  "payload": {
    "agent_id": "agent-123",
    "token": "xxx.yyy",
    ...
  }
}
```

### HTTP API запрос
```bash
curl -H "Authorization: Bearer <api_token>" \
     https://relay.example.com/api/v1/agents
```

## Конфигурация

```json
{
  "api_token": "static-token-for-backward-compat",
  "allowed_tokens": {
    "legacy-token": "agent-123"
  }
}
```

## Безопасность

1. **HMAC-SHA256 подпись** — каждый токен подписан клиентским секретом
2. **Client isolation** — токены привязаны к client_id
3. **Rate limiting** — защита от DDoS и брутфорса
4. **Revocation** — мгновенный отзыв токенов
5. **Rotation** — безопасная смена токенов без простоя

## TODO (будущие улучшения)

- [ ] Persistence (сохранение токенов в БД)
- [ ] Token introspection endpoint (OAuth2-like)
- [ ] Scope-based авторизация (permissions)
- [ ] JWT standard format для совместимости

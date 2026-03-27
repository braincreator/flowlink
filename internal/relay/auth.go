// Package relay — модуль аутентификации и авторизации.
// JWT-like токены на HMAC-SHA256, rate limiting, multi-tenancy.
package relay

import (
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"sync"
	"time"
)

// === Token Types ===

// TokenType — тип токена.
type TokenType string

const (
	TokenTypeAgent TokenType = "agent" // Pairwise токен для агентов
	TokenTypeAPI   TokenType = "api"   // API токен для HTTP клиентов
)

// Token — структура токена.
type Token struct {
	ID        string    `json:"id"`         // Уникальный ID токена
	Type      TokenType `json:"type"`       // Тип токена
	ClientID  string    `json:"client_id"`  // ID клиента/агента
	CreatedAt int64     `json:"created_at"` // Unix timestamp
	ExpiresAt int64     `json:"expires_at"` // Unix timestamp (0 = бессрочный)
	Revoked   bool      `json:"revoked"`    // Отозван ли токен
}

// AuthManager — менеджер аутентификации.
type AuthManager struct {
	mu       sync.RWMutex
	tokens   map[string]*Token        // token_id → Token
	clientTokens map[string][]string  // client_id → []token_id
	secrets  map[string][]byte        // client_id → HMAC secret
	revoked  map[string]bool          // token_id → revoked
	logger   *slog.Logger
}

// NewAuthManager — создаёт новый менеджер аутентификации.
func NewAuthManager(logger *slog.Logger) *AuthManager {
	if logger == nil {
		logger = slog.Default()
	}
	return &AuthManager{
		tokens:       make(map[string]*Token),
		clientTokens: make(map[string][]string),
		secrets:      make(map[string][]byte),
		revoked:      make(map[string]bool),
		logger:       logger,
	}
}

// === Agent Tokens (Pairwise) ===

// GenerateAgentToken — генерирует pairwise токен для агента.
// Формат: base64(JSON(Token)) + "." + signature
func (am *AuthManager) GenerateAgentToken(agentID string, expiresInSeconds int64) (string, error) {
	am.mu.Lock()
	defer am.mu.Unlock()

	// Генерируем ID токена
	tokenID := generateTokenID()

	// Создаём токен
	token := &Token{
		ID:       tokenID,
		Type:     TokenTypeAgent,
		ClientID: agentID,
		CreatedAt: time.Now().Unix(),
	}

	if expiresInSeconds > 0 {
		token.ExpiresAt = time.Now().Unix() + expiresInSeconds
	}

	// Генерируем или получаем секрет для клиента
	secret := am.secrets[agentID]
	if secret == nil {
		secret = generateSecret()
		am.secrets[agentID] = secret
	}

	// Сериализуем токен
	tokenJSON, err := json.Marshal(token)
	if err != nil {
		return "", fmt.Errorf("сериализация токена: %w", err)
	}

	// Создаём подпись
	signature := signToken(tokenJSON, secret)

	// Формируем итоговый токен
	tokenStr := base64.RawURLEncoding.EncodeToString(tokenJSON) + "." + signature

	// Сохраняем токен
	am.tokens[tokenID] = token
	am.clientTokens[agentID] = append(am.clientTokens[agentID], tokenID)

	am.logger.Debug("сгенерирован токен агента", "agent_id", agentID, "token_id", tokenID)

	return tokenStr, nil
}

// ValidateAgentToken — проверяет токен агента, возвращает agent_id.
func (am *AuthManager) ValidateAgentToken(agentID, tokenStr string) (bool, error) {
	am.mu.RLock()
	defer am.mu.RUnlock()

	// Парсим токен
	token, err := am.parseToken(tokenStr)
	if err != nil {
		return false, err
	}

	// Проверяем тип
	if token.Type != TokenTypeAgent {
		return false, fmt.Errorf("неверный тип токена")
	}

	// Проверяем agent_id
	if token.ClientID != agentID {
		return false, fmt.Errorf("agent_id не совпадает")
	}

	// Проверяем отзыв
	if am.revoked[token.ID] {
		return false, fmt.Errorf("токен отозван")
	}

	// Проверяем expiration
	if token.ExpiresAt > 0 && time.Now().Unix() > token.ExpiresAt {
		return false, fmt.Errorf("токен истёк")
	}

	// Проверяем подпись
	secret := am.secrets[agentID]
	if secret == nil {
		return false, fmt.Errorf("секрет не найден")
	}

	if !verifyTokenSignature(tokenStr, secret) {
		return false, fmt.Errorf("неверная подпись")
	}

	return true, nil
}

// === API Tokens (Multi-tenancy) ===

// GenerateAPIToken — генерирует API токен для HTTP клиента.
func (am *AuthManager) GenerateAPIToken(clientID string, expiresInSeconds int64) (string, error) {
	am.mu.Lock()
	defer am.mu.Unlock()

	// Генерируем ID токена
	tokenID := generateTokenID()

	// Создаём токен
	token := &Token{
		ID:       tokenID,
		Type:     TokenTypeAPI,
		ClientID: clientID,
		CreatedAt: time.Now().Unix(),
	}

	if expiresInSeconds > 0 {
		token.ExpiresAt = time.Now().Unix() + expiresInSeconds
	}

	// Генерируем или получаем секрет для клиента
	secret := am.secrets[clientID]
	if secret == nil {
		secret = generateSecret()
		am.secrets[clientID] = secret
	}

	// Сериализуем токен
	tokenJSON, err := json.Marshal(token)
	if err != nil {
		return "", fmt.Errorf("сериализация токена: %w", err)
	}

	// Создаём подпись
	signature := signToken(tokenJSON, secret)

	// Формируем итоговый токен
	tokenStr := base64.RawURLEncoding.EncodeToString(tokenJSON) + "." + signature

	// Сохраняем токен
	am.tokens[tokenID] = token
	am.clientTokens[clientID] = append(am.clientTokens[clientID], tokenID)

	am.logger.Info("сгенерирован API токен", "client_id", clientID, "token_id", tokenID)

	return tokenStr, nil
}

// ValidateAPIToken — проверяет API токен, возвращает client_id.
func (am *AuthManager) ValidateAPIToken(tokenStr string) (string, error) {
	am.mu.RLock()
	defer am.mu.RUnlock()

	// Парсим токен
	token, err := am.parseToken(tokenStr)
	if err != nil {
		return "", err
	}

	// Проверяем тип
	if token.Type != TokenTypeAPI {
		return "", fmt.Errorf("неверный тип токена")
	}

	// Проверяем отзыв
	if am.revoked[token.ID] {
		return "", fmt.Errorf("токен отозван")
	}

	// Проверяем expiration
	if token.ExpiresAt > 0 && time.Now().Unix() > token.ExpiresAt {
		return "", fmt.Errorf("токен истёк")
	}

	// Проверяем подпись
	secret := am.secrets[token.ClientID]
	if secret == nil {
		return "", fmt.Errorf("секрет не найден")
	}

	if !verifyTokenSignature(tokenStr, secret) {
		return "", fmt.Errorf("неверная подпись")
	}

	return token.ClientID, nil
}

// === Token Management ===

// RotateTokens — ротация токена агента (инвалидация старых, генерация нового).
func (am *AuthManager) RotateTokens(agentID string, expiresInSeconds int64) (string, error) {
	am.mu.Lock()
	defer am.mu.Unlock()

	// Отзываем все старые токены агента
	tokenIDs := am.clientTokens[agentID]
	for _, tokenID := range tokenIDs {
		am.revoked[tokenID] = true
	}

	// Очищаем список
	am.clientTokens[agentID] = nil

	am.mu.Unlock()
	defer am.mu.Lock() // Re-lock для defer

	// Генерируем новый токен
	return am.GenerateAgentToken(agentID, expiresInSeconds)
}

// RevokeToken — отзывает токен по его строковому представлению.
func (am *AuthManager) RevokeToken(tokenStr string) error {
	am.mu.Lock()
	defer am.mu.Unlock()

	token, err := am.parseToken(tokenStr)
	if err != nil {
		return err
	}

	am.revoked[token.ID] = true
	am.logger.Info("токен отозван", "token_id", token.ID, "client_id", token.ClientID)

	return nil
}

// === Helpers ===

// parseToken — парсит токен из строки.
func (am *AuthManager) parseToken(tokenStr string) (*Token, error) {
	parts := strings.Split(tokenStr, ".")
	if len(parts) != 2 {
		return nil, fmt.Errorf("неверный формат токена")
	}

	tokenJSON, err := base64.RawURLEncoding.DecodeString(parts[0])
	if err != nil {
		return nil, fmt.Errorf("декодирование токена: %w", err)
	}

	var token Token
	if err := json.Unmarshal(tokenJSON, &token); err != nil {
		return nil, fmt.Errorf("парсинг токена: %w", err)
	}

	return &token, nil
}

// generateTokenID — генерирует уникальный ID токена.
func generateTokenID() string {
	b := make([]byte, 16)
	rand.Read(b)
	return base64.RawURLEncoding.EncodeToString(b)
}

// generateSecret — генерирует секретный ключ для HMAC.
func generateSecret() []byte {
	b := make([]byte, 32)
	rand.Read(b)
	return b
}

// signToken — подписывает токен с помощью HMAC-SHA256.
func signToken(tokenJSON, secret []byte) string {
	h := hmac.New(sha256.New, secret)
	h.Write(tokenJSON)
	return base64.RawURLEncoding.EncodeToString(h.Sum(nil))
}

// verifyTokenSignature — проверяет подпись токена.
func verifyTokenSignature(tokenStr string, secret []byte) bool {
	parts := strings.Split(tokenStr, ".")
	if len(parts) != 2 {
		return false
	}

	tokenJSON, err := base64.RawURLEncoding.DecodeString(parts[0])
	if err != nil {
		return false
	}

	expectedSig := signToken(tokenJSON, secret)
	return hmac.Equal([]byte(parts[1]), []byte(expectedSig))
}

// === Rate Limiting ===

// RateLimiter — rate limiter с sliding window.
type RateLimiter struct {
	mu         sync.RWMutex
	windows    sync.Map // client_id → *rateWindow
	maxPerMin  int
	maxPerHour int
	logger     *slog.Logger
}

type rateWindow struct {
	minuteWindow  []int64 // timestamps в последнюю минуту
	hourWindow    []int64 // timestamps в последний час
	mu            sync.Mutex
}

// NewRateLimiter — создаёт новый rate limiter.
func NewRateLimiter(maxPerMin, maxPerHour int, logger *slog.Logger) *RateLimiter {
	if logger == nil {
		logger = slog.Default()
	}
	return &RateLimiter{
		maxPerMin:  maxPerMin,
		maxPerHour: maxPerHour,
		logger:     logger,
	}
}

// Check — проверяет, можно ли выполнить запрос.
// Возвращает (allowed, retryAfterSeconds).
func (rl *RateLimiter) Check(clientID string) (bool, int) {
	window := rl.getWindow(clientID)
	window.mu.Lock()
	defer window.mu.Unlock()

	now := time.Now().Unix()
	minuteAgo := now - 60
	hourAgo := now - 3600

	// Очищаем старые записи
	window.minuteWindow = filterTimestamps(window.minuteWindow, minuteAgo)
	window.hourWindow = filterTimestamps(window.hourWindow, hourAgo)

	// Проверяем лимиты
	if len(window.minuteWindow) >= rl.maxPerMin {
		// Retry after: конец текущей минуты
		retryAfter := int(window.minuteWindow[0] + 60 - now)
		if retryAfter < 1 {
			retryAfter = 1
		}
		rl.logger.Warn("rate limit превышен (minute)", "client_id", clientID, "count", len(window.minuteWindow))
		return false, retryAfter
	}

	if len(window.hourWindow) >= rl.maxPerHour {
		// Retry after: конец текущего часа
		retryAfter := int(window.hourWindow[0] + 3600 - now)
		if retryAfter < 1 {
			retryAfter = 1
		}
		rl.logger.Warn("rate limit превышен (hour)", "client_id", clientID, "count", len(window.hourWindow))
		return false, retryAfter
	}

	// Добавляем текущий запрос
	window.minuteWindow = append(window.minuteWindow, now)
	window.hourWindow = append(window.hourWindow, now)

	return true, 0
}

// getWindow — получает или создаёт окно для клиента.
func (rl *RateLimiter) getWindow(clientID string) *rateWindow {
	if v, ok := rl.windows.Load(clientID); ok {
		return v.(*rateWindow)
	}

	window := &rateWindow{
		minuteWindow: make([]int64, 0, 30),
		hourWindow:   make([]int64, 0, 200),
	}

	actual, _ := rl.windows.LoadOrStore(clientID, window)
	return actual.(*rateWindow)
}

// filterTimestamps — фильтрует timestamps, оставляя только недавние.
func filterTimestamps(timestamps []int64, cutoff int64) []int64 {
	// Находим первый индекс >= cutoff
	start := 0
	for i, ts := range timestamps {
		if ts >= cutoff {
			start = i
			break
		}
		if i == len(timestamps)-1 {
			// Все старые
			return timestamps[:0]
		}
	}

	if start == 0 {
		return timestamps
	}

	// Сдвигаем влево
	copy(timestamps, timestamps[start:])
	return timestamps[:len(timestamps)-start]
}

// === HTTP Helpers ===

// GetClientIDFromContext — извлекает client_id из контекста запроса.
func GetClientIDFromContext(r *http.Request) string {
	if id := r.Header.Get("X-Client-ID"); id != "" {
		return id
	}
	return "anonymous"
}

// SetClientIDInContext — устанавливает client_id в заголовки ответа.
func SetClientIDInContext(w http.ResponseWriter, clientID string) {
	w.Header().Set("X-Client-ID", clientID)
}

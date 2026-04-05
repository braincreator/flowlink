// Package relay — модуль аутентификации и авторизации.
// JWT-like токены на HMAC-SHA256, rate limiting, multi-tenancy.
package relay

import (
	"github.com/braincreator/flowlink/internal/protocol"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"strings"
	"sync"
	"time"
)

// === Token Types ===

// TokenType — тип токена.
type TokenType string

const (
	TokenTypeAgent   TokenType = "agent"   // Pairwise токен для агентов
	TokenTypeAPI     TokenType = "api"     // API токен для HTTP клиентов
	TokenTypeRefresh TokenType = "refresh" // Refresh токен для rotation
)

// Token expiry defaults
const (
	AccessTokenExpiry  = 24 * time.Hour // Access token: 24 hours
	RefreshTokenExpiry = 7 * 24 * time.Hour // Refresh token: 7 days
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

// TokenPair — пара access + refresh токенов.
type TokenPair struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresAt    int64  `json:"expires_at"`
	TokenType    string `json:"token_type"` // всегда "Bearer"
}

// blacklistEntry — запись в blacklist.
type blacklistEntry struct {
	tokenID   string
	expiresAt int64 // когда entry можно удалить
}

// AuthManager — менеджер аутентификации.
type AuthManager struct {
	mu            sync.RWMutex
	tokens        map[string]*Token        // token_id → Token
	clientTokens  map[string][]string     // client_id → []token_id
	secrets       map[string][]byte       // client_id → HMAC secret
	revoked       map[string]bool         // token_id → revoked
	blacklist     map[string]int64        // token_id → expiresAt (blacklisted tokens)
	refreshTokens map[string]string       // refresh_token_id → access_token_id (for rotation)
	logger        *slog.Logger
	stopCleanup   chan struct{}           // канал для остановки cleanup goroutine
}

// Close — останавливает фоновые горутины AuthManager (safe to call multiple times).
func (am *AuthManager) Close() {
	select {
	case <-am.stopCleanup:
		// already closed
		return
	default:
		close(am.stopCleanup)
	}
}

// NewAuthManager — создаёт новый менеджер аутентификации.
func NewAuthManager(logger *slog.Logger) *AuthManager {
	if logger == nil {
		logger = slog.Default()
	}
	am := &AuthManager{
		tokens:        make(map[string]*Token),
		clientTokens:  make(map[string][]string),
		secrets:       make(map[string][]byte),
		revoked:       make(map[string]bool),
		blacklist:     make(map[string]int64),
		refreshTokens: make(map[string]string),
		logger:        logger,
		stopCleanup:   make(chan struct{}),
	}

	// Запускаем periodic cleanup blacklist (skip in test mode)
	if os.Getenv("FLOWLINK_TEST_MODE") == "" {
		go am.cleanupBlacklistLoop()
	}

	return am
}

// Stop — останавливает background goroutines (для graceful shutdown).
func (am *AuthManager) Stop() {
	close(am.stopCleanup)
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
		return "", protocol.ErrCause(protocol.CodeTokenSerializeFail, err)
	}

	// Создаём подпись
	signature := signToken(tokenJSON, secret)

	// Формируем итоговый токен
	tokenStr := base64.RawURLEncoding.EncodeToString(tokenJSON) + "." + signature

	// Сохраняем токен
	am.tokens[tokenID] = token
	am.clientTokens[agentID] = append(am.clientTokens[agentID], tokenID)

	am.logger.Debug("agent token generated", "agent_id", agentID, "token_id", tokenID)

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
		return false, protocol.Err(protocol.CodeTokenTypeInvalid)
	}

	// Проверяем agent_id
	if token.ClientID != agentID {
		return false, fmt.Errorf("agent_id не совпадает")
	}

	// Проверяем отзыв
	if am.revoked[token.ID] {
		return false, protocol.Err(protocol.CodeTokenRevoked)
	}

	// Проверяем expiration
	if token.ExpiresAt > 0 && time.Now().Unix() > token.ExpiresAt {
		return false, protocol.Err(protocol.CodeTokenExpired)
	}

	// Проверяем подпись
	secret := am.secrets[agentID]
	if secret == nil {
		return false, protocol.Err(protocol.CodeSecretNotFound)
	}

	if !verifyTokenSignature(tokenStr, secret) {
		return false, protocol.Err(protocol.CodeSignatureInvalid)
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
		return "", protocol.ErrCause(protocol.CodeTokenSerializeFail, err)
	}

	// Создаём подпись
	signature := signToken(tokenJSON, secret)

	// Формируем итоговый токен
	tokenStr := base64.RawURLEncoding.EncodeToString(tokenJSON) + "." + signature

	// Сохраняем токен
	am.tokens[tokenID] = token
	am.clientTokens[clientID] = append(am.clientTokens[clientID], tokenID)

	am.logger.Info("API token generated", "client_id", clientID, "token_id", tokenID)

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
		return "", protocol.Err(protocol.CodeTokenTypeInvalid)
	}

	// Проверяем отзыв
	if am.revoked[token.ID] {
		return "", protocol.Err(protocol.CodeTokenRevoked)
	}

	// Проверяем expiration
	if token.ExpiresAt > 0 && time.Now().Unix() > token.ExpiresAt {
		return "", protocol.Err(protocol.CodeTokenExpired)
	}

	// Проверяем подпись
	secret := am.secrets[token.ClientID]
	if secret == nil {
		return "", protocol.Err(protocol.CodeSecretNotFound)
	}

	if !verifyTokenSignature(tokenStr, secret) {
		return "", protocol.Err(protocol.CodeSignatureInvalid)
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
	am.logger.Info("token revoked", "token_id", token.ID, "client_id", token.ClientID)

	return nil
}

// === JWT Token Rotation ===

// GenerateTokenPair — генерирует пару access + refresh токенов.
// Access token: 24h expiry
// Refresh token: 7d expiry, содержит type="refresh"
func (am *AuthManager) GenerateTokenPair(clientID string) (*TokenPair, error) {
	am.mu.Lock()
	defer am.mu.Unlock()

	now := time.Now()
	accessExpiry := now.Add(AccessTokenExpiry).Unix()
	refreshExpiry := now.Add(RefreshTokenExpiry).Unix()

	// Генерируем или получаем секрет для клиента
	secret := am.secrets[clientID]
	if secret == nil {
		secret = generateSecret()
		am.secrets[clientID] = secret
	}

	// Генерируем access token
	accessID := generateTokenID()
	accessToken := &Token{
		ID:        accessID,
		Type:      TokenTypeAPI,
		ClientID:  clientID,
		CreatedAt: now.Unix(),
		ExpiresAt: accessExpiry,
	}

	accessJSON, err := json.Marshal(accessToken)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTokenSerializeFail, err)
	}
	accessSig := signToken(accessJSON, secret)
	accessStr := base64.RawURLEncoding.EncodeToString(accessJSON) + "." + accessSig

	// Генерируем refresh token
	refreshID := generateTokenID()
	refreshToken := &Token{
		ID:        refreshID,
		Type:      TokenTypeRefresh,
		ClientID:  clientID,
		CreatedAt: now.Unix(),
		ExpiresAt: refreshExpiry,
	}

	refreshJSON, err := json.Marshal(refreshToken)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTokenSerializeFail, err)
	}
	refreshSig := signToken(refreshJSON, secret)
	refreshStr := base64.RawURLEncoding.EncodeToString(refreshJSON) + "." + refreshSig

	// Сохраняем токены
	am.tokens[accessID] = accessToken
	am.tokens[refreshID] = refreshToken
	am.clientTokens[clientID] = append(am.clientTokens[clientID], accessID, refreshID)
	am.refreshTokens[refreshID] = accessID // связь refresh → access

	am.logger.Info("token pair generated",
		"client_id", clientID,
		"access_id", accessID,
		"refresh_id", refreshID,
		"access_expiry", time.Unix(accessExpiry, 0).Format(time.RFC3339),
		"refresh_expiry", time.Unix(refreshExpiry, 0).Format(time.RFC3339),
	)

	return &TokenPair{
		AccessToken:  accessStr,
		RefreshToken: refreshStr,
		ExpiresAt:    accessExpiry,
		TokenType:    "Bearer",
	}, nil
}

// RefreshToken — валидирует refresh token и генерирует новую пару (rotation).
// Старый refresh token инвалидируется.
func (am *AuthManager) RefreshToken(refreshTokenStr string) (*TokenPair, error) {
	// Парсим refresh token
	token, err := am.parseToken(refreshTokenStr)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTokenParseFailed, err)
	}

	// Проверяем тип
	if token.Type != TokenTypeRefresh {
		return nil, protocol.Err(protocol.CodeTokenTypeInvalid)
	}

	am.mu.Lock()
	defer am.mu.Unlock()

	// Проверяем blacklist
	if _, blacklisted := am.blacklist[token.ID]; blacklisted {
		return nil, fmt.Errorf("refresh токен в blacklist")
	}

	// Проверяем отзыв
	if am.revoked[token.ID] {
		return nil, fmt.Errorf("refresh токен отозван")
	}

	// Проверяем expiration
	if token.ExpiresAt > 0 && time.Now().Unix() > token.ExpiresAt {
		return nil, fmt.Errorf("refresh токен истёк")
	}

	// Проверяем подпись
	secret := am.secrets[token.ClientID]
	if secret == nil {
		return nil, protocol.Err(protocol.CodeSecretNotFound)
	}

	if !verifyTokenSignature(refreshTokenStr, secret) {
		return nil, protocol.Err(protocol.CodeSignatureInvalid)
	}

	// Rotation: добавляем старые токены в blacklist
	oldAccessID := am.refreshTokens[token.ID]
	if oldAccessID != "" {
		am.blacklist[oldAccessID] = time.Now().Add(AccessTokenExpiry).Unix()
		delete(am.tokens, oldAccessID)
	}
	am.blacklist[token.ID] = time.Now().Add(RefreshTokenExpiry).Unix()
	delete(am.tokens, token.ID)
	delete(am.refreshTokens, token.ID)

	// Генерируем новую пару
	am.mu.Unlock() // Unlock для рекурсивного вызова
	pair, err := am.GenerateTokenPair(token.ClientID)
	am.mu.Lock() // Re-lock для defer

	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTokenGenerateError, err)
	}

	am.logger.Info("token refresh completed",
		"client_id", token.ClientID,
		"old_refresh_id", token.ID,
		"old_access_id", oldAccessID,
	)

	return pair, nil
}

// IsBlacklisted — проверяет, находится ли токен в blacklist.
func (am *AuthManager) IsBlacklisted(tokenID string) bool {
	am.mu.RLock()
	defer am.mu.RUnlock()

	_, blacklisted := am.blacklist[tokenID]
	return blacklisted
}

// AddToBlacklist — добавляет токен в blacklist.
func (am *AuthManager) AddToBlacklist(tokenStr string) error {
	token, err := am.parseToken(tokenStr)
	if err != nil {
		return err
	}

	am.mu.Lock()
	defer am.mu.Unlock()

	// Blacklist expiry = token expiry (или max TTL если бессрочный)
	expiry := token.ExpiresAt
	if expiry == 0 {
		expiry = time.Now().Add(7 * 24 * time.Hour).Unix() // max 7 days
	}

	am.blacklist[token.ID] = expiry
	am.logger.Info("token added to blacklist", "token_id", token.ID, "client_id", token.ClientID)

	return nil
}

// Logout — logout клиента: добавляет access token в blacklist.
func (am *AuthManager) Logout(accessTokenStr string) error {
	return am.AddToBlacklist(accessTokenStr)
}

// RevokeByClientID — отзывает все токены клиента (admin operation).
func (am *AuthManager) RevokeByClientID(clientID string) int {
	am.mu.Lock()
	defer am.mu.Unlock()

	tokenIDs := am.clientTokens[clientID]
	count := 0

	for _, tokenID := range tokenIDs {
		if !am.revoked[tokenID] {
			am.revoked[tokenID] = true
			// Также добавляем в blacklist для быстрой проверки
			if token, ok := am.tokens[tokenID]; ok && token.ExpiresAt > 0 {
				am.blacklist[tokenID] = token.ExpiresAt
			}
			count++
		}
	}

	// Очищаем связь refresh → access
	for refreshID, accessID := range am.refreshTokens {
		if accessID != "" {
			for _, tokenID := range tokenIDs {
				if tokenID == accessID {
					am.blacklist[refreshID] = time.Now().Add(RefreshTokenExpiry).Unix()
					delete(am.refreshTokens, refreshID)
					break
				}
			}
		}
	}

	am.logger.Info("client tokens revoked", "client_id", clientID, "count", count)

	return count
}

// ValidateTokenWithBlacklist — проверяет токен с учётом blacklist.
// Используется в middleware.
func (am *AuthManager) ValidateTokenWithBlacklist(tokenStr string) (string, error) {
	// Парсим токен
	token, err := am.parseToken(tokenStr)
	if err != nil {
		return "", err
	}

	am.mu.RLock()
	defer am.mu.RUnlock()

	// Проверяем blacklist (быстрая проверка)
	if _, blacklisted := am.blacklist[token.ID]; blacklisted {
		return "", protocol.Err(protocol.CodeTokenBlacklisted)
	}

	// Проверяем тип (только API токены для HTTP)
	if token.Type != TokenTypeAPI {
		return "", protocol.Err(protocol.CodeTokenTypeInvalid)
	}

	// Проверяем отзыв
	if am.revoked[token.ID] {
		return "", protocol.Err(protocol.CodeTokenRevoked)
	}

	// Проверяем expiration
	if token.ExpiresAt > 0 && time.Now().Unix() > token.ExpiresAt {
		return "", protocol.Err(protocol.CodeTokenExpired)
	}

	// Проверяем подпись
	secret := am.secrets[token.ClientID]
	if secret == nil {
		return "", protocol.Err(protocol.CodeSecretNotFound)
	}

	if !verifyTokenSignature(tokenStr, secret) {
		return "", protocol.Err(protocol.CodeSignatureInvalid)
	}

	return token.ClientID, nil
}

// ParseTokenStr — экспортированный метод для парсинга токена.
func (am *AuthManager) ParseTokenStr(tokenStr string) (*Token, error) {
	return am.parseToken(tokenStr)
}

// doCleanupBlacklist — single-pass cleanup of expired blacklist entries.
func (am *AuthManager) doCleanupBlacklist() {
	am.mu.Lock()
	defer am.mu.Unlock()
	now := time.Now().Unix()
	for tokenID, expiry := range am.blacklist {
		if now > expiry {
			delete(am.blacklist, tokenID)
		}
	}
}

// cleanupBlacklistLoop — background goroutine: periodic cleanup of expired blacklist entries.
func (am *AuthManager) cleanupBlacklistLoop() {
	ticker := time.NewTicker(1 * time.Hour)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			am.doCleanupBlacklist()
			am.logger.Debug("blacklist cleanup completed")
		case <-am.stopCleanup:
			return
		}
	}
}

// === Helpers ===

// parseToken — парсит токен из строки.
func (am *AuthManager) parseToken(tokenStr string) (*Token, error) {
	parts := strings.Split(tokenStr, ".")
	if len(parts) != 2 {
		return nil, protocol.Err(protocol.CodeTokenDecodeFailed)
	}

	tokenJSON, err := base64.RawURLEncoding.DecodeString(parts[0])
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTokenDecodeFailed, err)
	}

	var token Token
	if err := json.Unmarshal(tokenJSON, &token); err != nil {
		return nil, protocol.ErrCause(protocol.CodeTokenParseFailed, err)
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

	// Statistics
	totalRequests    int64
	rejectedRequests int64
	lastReset        time.Time

	// Per-client limits (override defaults)
	clientLimits sync.Map // client_id → *ClientLimitConfig
}

// ClientLimitConfig — конфигурация лимитов для конкретного клиента.
type ClientLimitConfig struct {
	MaxPerMin  int
	MaxPerHour int
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
	if maxPerMin <= 0 {
		maxPerMin = 30
	}
	if maxPerHour <= 0 {
		maxPerHour = 200
	}
	return &RateLimiter{
		maxPerMin:  maxPerMin,
		maxPerHour: maxPerHour,
		logger:     logger,
		lastReset:  time.Now(),
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

	// Получаем лимиты для клиента (override или defaults)
	maxPerMin, maxPerHour := rl.getClientLimits(clientID)

	// Проверяем лимиты
	if len(window.minuteWindow) >= maxPerMin {
		// Retry after: конец текущей минуты
		retryAfter := int(window.minuteWindow[0] + 60 - now)
		if retryAfter < 1 {
			retryAfter = 1
		}
		rl.mu.Lock()
		rl.rejectedRequests++
		rl.mu.Unlock()
		rl.logger.Warn("rate limit превышен (minute)", "client_id", clientID, "count", len(window.minuteWindow))
		return false, retryAfter
	}

	if len(window.hourWindow) >= maxPerHour {
		// Retry after: конец текущего часа
		retryAfter := int(window.hourWindow[0] + 3600 - now)
		if retryAfter < 1 {
			retryAfter = 1
		}
		rl.mu.Lock()
		rl.rejectedRequests++
		rl.mu.Unlock()
		rl.logger.Warn("rate limit превышен (hour)", "client_id", clientID, "count", len(window.hourWindow))
		return false, retryAfter
	}

	// Добавляем текущий запрос
	window.minuteWindow = append(window.minuteWindow, now)
	window.hourWindow = append(window.hourWindow, now)

	rl.mu.Lock()
	rl.totalRequests++
	rl.mu.Unlock()

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

// getClientLimits — возвращает лимиты для клиента (override или defaults).
func (rl *RateLimiter) getClientLimits(clientID string) (int, int) {
	if v, ok := rl.clientLimits.Load(clientID); ok {
		cfg := v.(*ClientLimitConfig)
		return cfg.MaxPerMin, cfg.MaxPerHour
	}
	return rl.maxPerMin, rl.maxPerHour
}

// SetClientLimits — устанавливает кастомные лимиты для клиента.
func (rl *RateLimiter) SetClientLimits(clientID string, maxPerMin, maxPerHour int) {
	cfg := &ClientLimitConfig{
		MaxPerMin:  maxPerMin,
		MaxPerHour: maxPerHour,
	}
	rl.clientLimits.Store(clientID, cfg)
	rl.logger.Info("client rate limits updated",
		"client_id", clientID,
		"max_per_min", maxPerMin,
		"max_per_hour", maxPerHour,
	)
}

// ResetClientLimits — сбрасывает кастомные лимиты клиента (return to defaults).
func (rl *RateLimiter) ResetClientLimits(clientID string) {
	rl.clientLimits.Delete(clientID)
	rl.logger.Info("client rate limits reset", "client_id", clientID)
}

// ClientStats — статистика rate limit для одного клиента.
type ClientStats struct {
	ClientID       string `json:"client_id"`
	RequestsPerMin int    `json:"requests_per_min"` // лимит
	Burst          int    `json:"burst"`            // лимит hour
	UsedMin        int    `json:"used_min"`        // использовано за минуту
	UsedHour       int    `json:"used_hour"`       // использовано за час
	Status         string `json:"status"`         // ok | warning | exceeded
}

// RateLimitStats — общая статистика rate limiting.
type RateLimitStats struct {
	TotalRequests    int64         `json:"total_requests"`
	RejectedRequests int64         `json:"rejected_requests"`
	LastReset        time.Time     `json:"last_reset"`
	DefaultMaxPerMin int           `json:"default_max_per_min"`
	DefaultMaxPerHour int          `json:"default_max_per_hour"`
	TopAbusers       []ClientStats `json:"top_abusers"` // топ по rejected
}

// GetClientStats — возвращает статистику для конкретного клиента.
func (rl *RateLimiter) GetClientStats(clientID string) ClientStats {
	window := rl.getWindow(clientID)
	window.mu.Lock()
	defer window.mu.Unlock()

	now := time.Now().Unix()
	minuteAgo := now - 60
	hourAgo := now - 3600

	// Очищаем старые записи
	window.minuteWindow = filterTimestamps(window.minuteWindow, minuteAgo)
	window.hourWindow = filterTimestamps(window.hourWindow, hourAgo)

	maxPerMin, maxPerHour := rl.getClientLimits(clientID)
	usedMin := len(window.minuteWindow)
	usedHour := len(window.hourWindow)

	// Определяем статус
	status := "ok"
	if usedMin >= maxPerMin || usedHour >= maxPerHour {
		status = "exceeded"
	} else if usedMin >= maxPerMin*8/10 || usedHour >= maxPerHour*8/10 {
		status = "warning"
	}

	return ClientStats{
		ClientID:       clientID,
		RequestsPerMin: maxPerMin,
		Burst:          maxPerHour,
		UsedMin:        usedMin,
		UsedHour:       usedHour,
		Status:         status,
	}
}

// GetAllClientStats — возвращает статистику для всех клиентов.
func (rl *RateLimiter) GetAllClientStats() []ClientStats {
	var stats []ClientStats

	rl.windows.Range(func(key, value interface{}) bool {
		clientID := key.(string)
		stats = append(stats, rl.GetClientStats(clientID))
		return true
	})

	return stats
}

// GetStats — возвращает общую статистику rate limiting.
func (rl *RateLimiter) GetStats() RateLimitStats {
	rl.mu.RLock()
	defer rl.mu.RUnlock()

	// Собираем топ абузеров
	clientStats := rl.GetAllClientStats()

	// Сортируем по rejected (в реальной реализации можно добавить rejected counter per client)
	// Пока берём топ по использованию
	topAbusers := make([]ClientStats, 0, 5)
	for _, cs := range clientStats {
		if cs.Status == "exceeded" || cs.Status == "warning" {
			topAbusers = append(topAbusers, cs)
		}
		if len(topAbusers) >= 5 {
			break
		}
	}

	return RateLimitStats{
		TotalRequests:     rl.totalRequests,
		RejectedRequests:  rl.rejectedRequests,
		LastReset:         rl.lastReset,
		DefaultMaxPerMin:  rl.maxPerMin,
		DefaultMaxPerHour: rl.maxPerHour,
		TopAbusers:        topAbusers,
	}
}

// ResetStats — сбрасывает статистику.
func (rl *RateLimiter) ResetStats() {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	rl.totalRequests = 0
	rl.rejectedRequests = 0
	rl.lastReset = time.Now()

	rl.logger.Info("rate limit stats reset")
}

// ResetClientCounters — сбрасывает счётчики для конкретного клиента.
func (rl *RateLimiter) ResetClientCounters(clientID string) {
	window := rl.getWindow(clientID)
	window.mu.Lock()
	defer window.mu.Unlock()

	window.minuteWindow = window.minuteWindow[:0]
	window.hourWindow = window.hourWindow[:0]

	rl.logger.Info("client rate counters reset", "client_id", clientID)
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

// TokenCount — количество активных токенов.
func (am *AuthManager) TokenCount() int {
	am.mu.RLock()
	defer am.mu.RUnlock()
	return len(am.tokens)
}

// BlacklistCount — количество токенов в blacklist.
func (am *AuthManager) BlacklistCount() int {
	am.mu.RLock()
	defer am.mu.RUnlock()
	return len(am.blacklist)
}

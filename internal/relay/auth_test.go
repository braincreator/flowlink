package relay

import (
	"encoding/base64"
	"encoding/json"
	"log/slog"
	"testing"
	"time"
)

func TestGenerateTokenPair(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-1"

	pair, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	if pair.AccessToken == "" {
		t.Error("AccessToken is empty")
	}
	if pair.RefreshToken == "" {
		t.Error("RefreshToken is empty")
	}
	if pair.ExpiresAt == 0 {
		t.Error("ExpiresAt is 0")
	}
	if pair.TokenType != "Bearer" {
		t.Errorf("TokenType = %s, want Bearer", pair.TokenType)
	}

	// Проверяем что access token валиден
	_, err = auth.ValidateTokenWithBlacklist(pair.AccessToken)
	if err != nil {
		t.Errorf("AccessToken validation failed: %v", err)
	}

	// Проверяем что refresh token не подходит для API access
	_, err = auth.ValidateTokenWithBlacklist(pair.RefreshToken)
	if err == nil {
		t.Error("RefreshToken should not be valid for API access")
	}
}

func TestRefreshToken(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-2"

	// Генерируем начальную пару
	pair1, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	// Refresh токены
	pair2, err := auth.RefreshToken(pair1.RefreshToken)
	if err != nil {
		t.Fatalf("RefreshToken failed: %v", err)
	}

	if pair2.AccessToken == "" {
		t.Error("RefreshToken returned empty AccessToken")
	}
	if pair2.RefreshToken == "" {
		t.Error("RefreshToken returned empty RefreshToken")
	}
	if pair2.AccessToken == pair1.AccessToken {
		t.Error("AccessToken was not rotated")
	}
	if pair2.RefreshToken == pair1.RefreshToken {
		t.Error("RefreshToken was not rotated")
	}

	// Старый access token должен быть в blacklist
	oldAccessToken, _ := auth.ParseTokenStr(pair1.AccessToken)
	if !auth.IsBlacklisted(oldAccessToken.ID) {
		t.Error("Old access token not blacklisted after refresh")
	}

	// Старый refresh token должен быть в blacklist
	oldRefreshToken, _ := auth.ParseTokenStr(pair1.RefreshToken)
	if !auth.IsBlacklisted(oldRefreshToken.ID) {
		t.Error("Old refresh token not blacklisted after refresh")
	}

	// Старый refresh token не должен работать
	_, err = auth.RefreshToken(pair1.RefreshToken)
	if err == nil {
		t.Error("Old refresh token should not work after rotation")
	}
}

func TestRefreshTokenExpired(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	// Генерируем токен напрямую с истёкшим сроком
	auth.mu.Lock()
	clientID := "test-client-3"
	tokenID := generateTokenID()
	now := time.Now()
	refreshToken := &Token{
		ID:        tokenID,
		Type:      TokenTypeRefresh,
		ClientID:  clientID,
		CreatedAt: now.Add(-2 * time.Hour).Unix(),
		ExpiresAt: now.Add(-1 * time.Hour).Unix(), // Истёк час назад
	}
	
	// Генерируем секрет
	secret := generateSecret()
	auth.secrets[clientID] = secret
	
	// Сериализуем и подписываем
	tokenJSON, _ := json.Marshal(refreshToken)
	signature := signToken(tokenJSON, secret)
	tokenStr := base64.RawURLEncoding.EncodeToString(tokenJSON) + "." + signature
	
	// Сохраняем в maps
	auth.tokens[tokenID] = refreshToken
	auth.clientTokens[clientID] = append(auth.clientTokens[clientID], tokenID)
	auth.refreshTokens[tokenID] = "access-id-placeholder"
	auth.mu.Unlock()

	// Refresh должен упасть
	_, err := auth.RefreshToken(tokenStr)
	if err == nil {
		t.Error("RefreshToken should fail with expired token")
	}
}

func TestRefreshTokenWrongType(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-4"

	// Генерируем access token (не refresh)
	pair, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	// Пытаемся refresh с access token вместо refresh
	_, err = auth.RefreshToken(pair.AccessToken)
	if err == nil {
		t.Error("RefreshToken should fail with access token")
	}
}

func TestBlacklist(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-5"

	pair, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	// Проверяем что токен валиден
	_, err = auth.ValidateTokenWithBlacklist(pair.AccessToken)
	if err != nil {
		t.Fatalf("AccessToken validation failed before blacklist: %v", err)
	}

	// Добавляем в blacklist
	err = auth.AddToBlacklist(pair.AccessToken)
	if err != nil {
		t.Fatalf("AddToBlacklist failed: %v", err)
	}

	// Теперь токен должен быть невалиден
	_, err = auth.ValidateTokenWithBlacklist(pair.AccessToken)
	if err == nil {
		t.Error("AccessToken should be invalid after blacklist")
	}
}

func TestLogout(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-6"

	pair, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	// Logout
	err = auth.Logout(pair.AccessToken)
	if err != nil {
		t.Fatalf("Logout failed: %v", err)
	}

	// Токен должен быть в blacklist
	_, err = auth.ValidateTokenWithBlacklist(pair.AccessToken)
	if err == nil {
		t.Error("AccessToken should be invalid after logout")
	}
}

func TestRevokeByClientID(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-7"

	// Генерируем несколько пар токенов
	pair1, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair 1 failed: %v", err)
	}

	pair2, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair 2 failed: %v", err)
	}

	// Оба access токена должны быть валидны
	_, err = auth.ValidateTokenWithBlacklist(pair1.AccessToken)
	if err != nil {
		t.Fatalf("AccessToken 1 validation failed: %v", err)
	}

	_, err = auth.ValidateTokenWithBlacklist(pair2.AccessToken)
	if err != nil {
		t.Fatalf("AccessToken 2 validation failed: %v", err)
	}

	// Revoke все токены клиента
	count := auth.RevokeByClientID(clientID)
	if count == 0 {
		t.Error("RevokeByClientID returned 0")
	}

	// Теперь оба токена должны быть невалидны
	_, err = auth.ValidateTokenWithBlacklist(pair1.AccessToken)
	if err == nil {
		t.Error("AccessToken 1 should be invalid after revoke")
	}

	_, err = auth.ValidateTokenWithBlacklist(pair2.AccessToken)
	if err == nil {
		t.Error("AccessToken 2 should be invalid after revoke")
	}
}

func TestTokenExpiry(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-8"

	pair, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	// Парсим access token
	accessToken, err := auth.ParseTokenStr(pair.AccessToken)
	if err != nil {
		t.Fatalf("ParseTokenStr failed: %v", err)
	}

	// Проверяем что expiry примерно 24 часа
	now := time.Now().Unix()
	expectedExpiry := now + int64(AccessTokenExpiry.Seconds())
	tolerance := int64(60) // 60 seconds tolerance

	if accessToken.ExpiresAt < expectedExpiry-tolerance || accessToken.ExpiresAt > expectedExpiry+tolerance {
		t.Errorf("Access token expiry = %d, expected ~%d (±60s)", accessToken.ExpiresAt, expectedExpiry)
	}

	// Парсим refresh token
	refreshToken, err := auth.ParseTokenStr(pair.RefreshToken)
	if err != nil {
		t.Fatalf("ParseTokenStr (refresh) failed: %v", err)
	}

	// Проверяем что expiry примерно 7 дней
	expectedRefreshExpiry := now + int64(RefreshTokenExpiry.Seconds())

	if refreshToken.ExpiresAt < expectedRefreshExpiry-tolerance || refreshToken.ExpiresAt > expectedRefreshExpiry+tolerance {
		t.Errorf("Refresh token expiry = %d, expected ~%d (±60s)", refreshToken.ExpiresAt, expectedRefreshExpiry)
	}
}

func TestBackwardCompatibility(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-9"

	// Генерируем старый API токен (без refresh)
	oldToken, err := auth.GenerateAPIToken(clientID, int64(AccessTokenExpiry.Seconds()))
	if err != nil {
		t.Fatalf("GenerateAPIToken failed: %v", err)
	}

	// Старый токен должен быть валиден
	validatedClientID, err := auth.ValidateAPIToken(oldToken)
	if err != nil {
		t.Fatalf("ValidateAPIToken failed: %v", err)
	}
	if validatedClientID != clientID {
		t.Errorf("ValidateAPIToken returned clientID = %s, want %s", validatedClientID, clientID)
	}

	// Также должен работать через ValidateTokenWithBlacklist
	validatedClientID2, err := auth.ValidateTokenWithBlacklist(oldToken)
	if err != nil {
		t.Fatalf("ValidateTokenWithBlacklist failed: %v", err)
	}
	if validatedClientID2 != clientID {
		t.Errorf("ValidateTokenWithBlacklist returned clientID = %s, want %s", validatedClientID2, clientID)
	}
}

func TestMultipleRefresh(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	clientID := "test-client-10"

	// Генерируем начальную пару
	pair, err := auth.GenerateTokenPair(clientID)
	if err != nil {
		t.Fatalf("GenerateTokenPair failed: %v", err)
	}

	// Выполняем несколько refresh
	for i := 0; i < 5; i++ {
		newPair, err := auth.RefreshToken(pair.RefreshToken)
		if err != nil {
			t.Fatalf("RefreshToken %d failed: %v", i+1, err)
		}

		// Новый access token должен быть валиден
		_, err = auth.ValidateTokenWithBlacklist(newPair.AccessToken)
		if err != nil {
			t.Fatalf("AccessToken validation after refresh %d failed: %v", i+1, err)
		}

		pair = newPair
	}
}

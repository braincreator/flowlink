// Package audit — HMAC подпись для audit логов.
package audit

import (
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

const (
	// HMACSecretLen — длина ключа в байтах.
	HMACSecretLen = 32
	// HMACField — имя поля в JSON.
	HMACField = "hmac"
)

// SignEntry — вычисляет HMAC-SHA256 подпись для entry.
// HMAC вычисляется от JSON-представления entry без поля "hmac".
func SignEntry(entry map[string]interface{}, secret []byte) string {
	// Удаляем поле hmac если есть
	data := make(map[string]interface{})
	for k, v := range entry {
		if k != HMACField {
			data[k] = v
		}
	}

	// Сериализуем в JSON (детерминированный порядок ключей)
	jsonBytes, err := json.Marshal(data)
	if err != nil {
		return ""
	}

	// Вычисляем HMAC-SHA256
	h := hmac.New(sha256.New, secret)
	h.Write(jsonBytes)
	return hex.EncodeToString(h.Sum(nil))
}

// VerifyEntry — проверяет HMAC-SHA256 подпись entry.
// Возвращает true если подпись валидна или отсутствует (legacy записи).
func VerifyEntry(entry map[string]interface{}, secret []byte) bool {
	// Получаем сохранённый HMAC
	storedHMAC, ok := entry[HMACField]
	if !ok {
		// Legacy запись без HMAC — считаем валидной
		return true
	}

	hmacStr, ok := storedHMAC.(string)
	if !ok {
		return false
	}

	// Вычисляем ожидаемый HMAC
	expectedHMAC := SignEntry(entry, secret)

	// Сравниваем в constant-time
	return hmac.Equal([]byte(hmacStr), []byte(expectedHMAC))
}

// NewHMACSecret — генерирует случайный 32-байтный ключ.
func NewHMACSecret() ([]byte, error) {
	secret := make([]byte, HMACSecretLen)
	if _, err := rand.Read(secret); err != nil {
		return nil, fmt.Errorf("ошибка генерации ключа: %w", err)
	}
	return secret, nil
}

// LoadOrGenerateHMACSecret — загружает ключ из файла или генерирует новый.
// Файл: ~/.flowlink/audit.key
func LoadOrGenerateHMACSecret(customPath string) ([]byte, error) {
	var keyPath string
	if customPath != "" {
		keyPath = customPath
	} else {
		home, err := os.UserHomeDir()
		if err != nil {
			return nil, fmt.Errorf("ошибка получения home директории: %w", err)
		}
		keyPath = filepath.Join(home, ".flowlink", "audit.key")
	}

	// Пробуем загрузить существующий ключ
	if data, err := os.ReadFile(keyPath); err == nil {
		if len(data) >= HMACSecretLen {
			return data[:HMACSecretLen], nil
		}
	}

	// Генерируем новый ключ
	secret, err := NewHMACSecret()
	if err != nil {
		return nil, err
	}

	// Создаём директорию если нужно
	if err := os.MkdirAll(filepath.Dir(keyPath), 0700); err != nil {
		return nil, fmt.Errorf("ошибка создания директории: %w", err)
	}

	// Сохраняем ключ
	if err := os.WriteFile(keyPath, secret, 0600); err != nil {
		return nil, fmt.Errorf("ошибка сохранения ключа: %w", err)
	}

	return secret, nil
}

// VerifyResult — результат верификации записи.
type VerifyResult struct {
	ID        string `json:"id"`
	Timestamp string `json:"timestamp,omitempty"`
	Tampered  bool   `json:"tampered"`
	Error     string `json:"error,omitempty"`
}

// VerifyAllEntries — проверяет все записи в файле и возвращает результаты.
// Возвращает записи с флагом tampered: true для невалидных.
func VerifyAllEntries(entries []map[string]interface{}, secret []byte) []VerifyResult {
	results := make([]VerifyResult, 0, len(entries))

	for _, entry := range entries {
		result := VerifyResult{}

		// Извлекаем ID
		if id, ok := entry["id"].(string); ok {
			result.ID = id
		}

		// Извлекаем timestamp
		if ts, ok := entry["timestamp"].(string); ok {
			result.Timestamp = ts
		}

		// Проверяем HMAC
		if !VerifyEntry(entry, secret) {
			result.Tampered = true
			result.Error = "HMAC verification failed"
		} else {
			result.Tampered = false
		}

		results = append(results, result)
	}

	return results
}

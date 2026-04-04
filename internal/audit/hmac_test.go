package audit

import (
	"testing"
)

func TestSignEntry(t *testing.T) {
	secret := []byte("test-secret-key-32-bytes-long-123456")
	
	entry := map[string]interface{}{
		"id":        "test-123",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}

	hmac := SignEntry(entry, secret)
	
	// HMAC должен быть 64 символа (sha256 hex)
	if len(hmac) != 64 {
		t.Errorf("Expected HMAC length 64, got %d", len(hmac))
	}

	// Тот же entry должен давать тот же HMAC
	hmac2 := SignEntry(entry, secret)
	if hmac != hmac2 {
		t.Error("Same entry should produce same HMAC")
	}
}

func TestSignEntry_ExcludesHMACField(t *testing.T) {
	secret := []byte("test-secret-key-32-bytes-long-123456")
	
	entry1 := map[string]interface{}{
		"id":     "test-123",
		"action": "exec",
	}
	
	entry2 := map[string]interface{}{
		"id":     "test-123",
		"action": "exec",
		"hmac":   "some-previous-hmac", // должно игнорироваться
	}

	hmac1 := SignEntry(entry1, secret)
	hmac2 := SignEntry(entry2, secret)

	if hmac1 != hmac2 {
		t.Error("HMAC should be computed without the hmac field")
	}
}

func TestVerifyEntry_Valid(t *testing.T) {
	secret := []byte("test-secret-key-32-bytes-long-123456")
	
	entry := map[string]interface{}{
		"id":        "test-123",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}

	// Подписываем
	hmac := SignEntry(entry, secret)
	entry["hmac"] = hmac

	// Проверяем
	if !VerifyEntry(entry, secret) {
		t.Error("Valid entry should pass verification")
	}
}

func TestVerifyEntry_LegacyNoHMAC(t *testing.T) {
	secret := []byte("test-secret-key-32-bytes-long-123456")
	
	// Entry без поля hmac (legacy)
	entry := map[string]interface{}{
		"id":        "test-123",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}

	// Должен проходить как валидный (legacy support)
	if !VerifyEntry(entry, secret) {
		t.Error("Legacy entry without HMAC should be considered valid")
	}
}

func TestVerifyEntry_TamperedData(t *testing.T) {
	secret := []byte("test-secret-key-32-bytes-long-123456")
	
	entry := map[string]interface{}{
		"id":        "test-123",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}

	// Подписываем
	hmac := SignEntry(entry, secret)
	entry["hmac"] = hmac

	// Меняем данные (подмена!)
	entry["action"] = "rm -rf /"

	// Проверяем — должно FAIL
	if VerifyEntry(entry, secret) {
		t.Error("Tampered entry should NOT pass verification")
	}
}

func TestVerifyEntry_WrongSecret(t *testing.T) {
	secret1 := []byte("test-secret-key-32-bytes-long-123456")
	secret2 := []byte("different-secret-key-32-bytes!!!!")
	
	entry := map[string]interface{}{
		"id":        "test-123",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}

	// Подписываем первым секретом
	hmac := SignEntry(entry, secret1)
	entry["hmac"] = hmac

	// Проверяем вторым — должно FAIL
	if VerifyEntry(entry, secret2) {
		t.Error("Entry signed with different secret should NOT pass")
	}
}

func TestNewHMACSecret(t *testing.T) {
	secret1, err := NewHMACSecret()
	if err != nil {
		t.Fatalf("NewHMACSecret failed: %v", err)
	}

	// Должен быть 32 байта
	if len(secret1) != HMACSecretLen {
		t.Errorf("Expected secret length %d, got %d", HMACSecretLen, len(secret1))
	}

	// Должен быть уникальным при каждом вызове
	secret2, err := NewHMACSecret()
	if err != nil {
		t.Fatalf("NewHMACSecret failed: %v", err)
	}

	same := true
	for i := range secret1 {
		if secret1[i] != secret2[i] {
			same = false
			break
		}
	}
	if same {
		t.Error("NewHMACSecret should generate unique secrets")
	}
}

func TestVerifyAllEntries(t *testing.T) {
	secret := []byte("test-secret-key-32-bytes-long-123456")

	// Создаём entries
	validEntry := map[string]interface{}{
		"id":        "valid-1",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}
	validEntry["hmac"] = SignEntry(validEntry, secret)

	tamperedEntry := map[string]interface{}{
		"id":        "tampered-2",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
	}
	tamperedEntry["hmac"] = SignEntry(tamperedEntry, secret)
	tamperedEntry["action"] = "malicious" // подмена!

	legacyEntry := map[string]interface{}{
		"id":        "legacy-3",
		"action":    "exec",
		"timestamp": "2024-01-01T00:00:00Z",
		// без hmac
	}

	entries := []map[string]interface{}{validEntry, tamperedEntry, legacyEntry}
	results := VerifyAllEntries(entries, secret)

	if len(results) != 3 {
		t.Fatalf("Expected 3 results, got %d", len(results))
	}

	// validEntry должен быть OK
	if results[0].Tampered {
		t.Error("Valid entry should not be marked as tampered")
	}
	if results[0].ID != "valid-1" {
		t.Errorf("Expected ID 'valid-1', got '%s'", results[0].ID)
	}

	// tamperedEntry должен быть помечен
	if !results[1].Tampered {
		t.Error("Tampered entry should be detected")
	}
	if results[1].ID != "tampered-2" {
		t.Errorf("Expected ID 'tampered-2', got '%s'", results[1].ID)
	}

	// legacyEntry должен быть OK
	if results[2].Tampered {
		t.Error("Legacy entry should not be marked as tampered")
	}
	if results[2].ID != "legacy-3" {
		t.Errorf("Expected ID 'legacy-3', got '%s'", results[2].ID)
	}
}

package devices

import (
	"crypto/rand"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/crypto"
)

const (
	pairingCodeLength   = 6
	pairingExpiry       = 10 * time.Minute
	pairingCleanupEvery = 1 * time.Minute
)

var (
	ErrPairingNotFound = errors.New("pairing request not found")
	ErrPairingExpired  = errors.New("pairing code expired")
)

// PairingRequest — запрос на паринг устройства.
type PairingRequest struct {
	Code      string    `json:"code"`
	Device    *Device   `json:"device"`
	ExpiresAt time.Time `json:"expires_at"`
	PublicKey []byte    `json:"public_key"`
}

// PairingManager — управляет процессом паринга устройств.
type PairingManager struct {
	registry *DeviceRegistry
	keyStore *crypto.KeyStore
	e2ee     *crypto.E2EELayer
	mu       sync.RWMutex
	pending  map[string]*PairingRequest // code → request
}

// NewPairingManager — создаёт PairingManager.
func NewPairingManager(registry *DeviceRegistry, keyStore *crypto.KeyStore, e2ee *crypto.E2EELayer) *PairingManager {
	pm := &PairingManager{
		registry: registry,
		keyStore: keyStore,
		e2ee:     e2ee,
		pending:  make(map[string]*PairingRequest),
	}
	go pm.cleanupLoop()
	return pm
}

// InitiatePairing — создаёт запрос на паринг для нового устройства.
func (pm *PairingManager) InitiatePairing(deviceName string, publicKey []byte, keyID string) (*PairingRequest, error) {
	code, err := generateCode()
	if err != nil {
		return nil, fmt.Errorf("generate pairing code: %w", err)
	}

	device := &Device{
		ID:        keyID, // используем keyID как device ID
		Name:      deviceName,
		PublicKey: publicKey,
		KeyID:     keyID,
		E2EE:      true,
	}

	req := &PairingRequest{
		Code:      code,
		Device:    device,
		ExpiresAt: time.Now().Add(pairingExpiry),
		PublicKey: publicKey,
	}

	pm.mu.Lock()
	pm.pending[code] = req
	pm.mu.Unlock()

	return req, nil
}

// ApprovePairing — подтверждает паринг по коду.
func (pm *PairingManager) ApprovePairing(code string, ownerID string) (*Device, error) {
	pm.mu.Lock()
	req, ok := pm.pending[code]
	if !ok {
		pm.mu.Unlock()
		return nil, ErrPairingNotFound
	}

	if time.Now().After(req.ExpiresAt) {
		delete(pm.pending, code)
		pm.mu.Unlock()
		return nil, ErrPairingExpired
	}
	delete(pm.pending, code)
	pm.mu.Unlock()

	req.Device.OwnerID = ownerID
	if err := pm.registry.Register(req.Device); err != nil {
		return nil, fmt.Errorf("register device: %w", err)
	}
	if err := pm.registry.Approve(req.Device.ID); err != nil {
		return nil, fmt.Errorf("approve device: %w", err)
	}

	return req.Device, nil
}

// RejectPairing — отклоняет запрос на паринг.
func (pm *PairingManager) RejectPairing(code string) error {
	pm.mu.Lock()
	defer pm.mu.Unlock()

	if _, ok := pm.pending[code]; !ok {
		return ErrPairingNotFound
	}

	delete(pm.pending, code)
	return nil
}

// VerifyDevice — проверяет что публичный ключ устройства совпадает.
func (pm *PairingManager) VerifyDevice(deviceID string, publicKey []byte) bool {
	d, err := pm.registry.Get(deviceID)
	if err != nil {
		return false
	}
	if len(d.PublicKey) != len(publicKey) {
		return false
	}
	for i := range d.PublicKey {
		if d.PublicKey[i] != publicKey[i] {
			return false
		}
	}
	return true
}

// GetPending — возвращает ожидающий запрос по коду.
func (pm *PairingManager) GetPending(code string) (*PairingRequest, bool) {
	pm.mu.RLock()
	defer pm.mu.RUnlock()
	req, ok := pm.pending[code]
	return req, ok
}

func (pm *PairingManager) cleanupLoop() {
	ticker := time.NewTicker(pairingCleanupEvery)
	defer ticker.Stop()
	for range ticker.C {
		pm.mu.Lock()
		now := time.Now()
		for code, req := range pm.pending {
			if now.After(req.ExpiresAt) {
				delete(pm.pending, code)
			}
		}
		pm.mu.Unlock()
	}
}

// generateCode — генерирует 6-значный код.
func generateCode() (string, error) {
	b := make([]byte, 3)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	// 3 байта → 0-16777215, берём модуль 900000 + 100000 → 100000-999999
	n := int(b[0])<<16 | int(b[1])<<8 | int(b[2])
	code := 100000 + (n % 900000)
	return fmt.Sprintf("%06d", code), nil
}

// Package devices — реестр устройств для FlowLink.
// Управляет регистрацией, подтверждением и отзывом доступа агентов.
package devices

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// Status — возможные статусы устройства.
const (
	StatusPending = "pending"
	StatusApproved = "approved"
	StatusRevoked = "revoked"
)

var (
	ErrDeviceNotFound   = errors.New("device not found")
	ErrDeviceAlreadyExists = errors.New("device already registered")
	ErrDeviceNotPending = errors.New("device is not pending")
)

// Device — зарегистрированное устройство (агент).
type Device struct {
	ID          string    `json:"id"`
	Name        string    `json:"name"`
	PublicKey   []byte    `json:"public_key"`
	KeyID       string    `json:"key_id"`
	OwnerID     string    `json:"owner_id"`
	Status      string    `json:"status"`
	ConnectedAt time.Time `json:"connected_at"`
	LastSeenAt  time.Time `json:"last_seen_at"`
	IPAddress   string    `json:"ip_address"`
	OS          string    `json:"os"`
	E2EE        bool      `json:"e2ee"`
}

// DeviceRegistry — потокобезопасный реестр устройств.
type DeviceRegistry struct {
	mu      sync.RWMutex
	dir     string
	devices map[string]*Device // ID → Device
}

// NewDeviceRegistry — создаёт или загружает реестр из dir.
func NewDeviceRegistry(dir string) (*DeviceRegistry, error) {
	if dir == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return nil, err
		}
		dir = filepath.Join(home, ".config", "flowlink")
	}

	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, fmt.Errorf("create device registry dir: %w", err)
	}

	r := &DeviceRegistry{
		dir:     dir,
		devices: make(map[string]*Device),
	}

	if err := r.load(); err != nil {
		if !os.IsNotExist(err) {
			return nil, err
		}
	}

	return r, nil
}

// Register — регистрирует новое устройство со статусом pending.
func (r *DeviceRegistry) Register(device *Device) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if _, ok := r.devices[device.ID]; ok {
		return ErrDeviceAlreadyExists
	}

	device.Status = StatusPending
	device.ConnectedAt = time.Now()
	device.LastSeenAt = time.Now()

	r.devices[device.ID] = device
	return r.save()
}

// Approve — подтверждает устройство.
func (r *DeviceRegistry) Approve(deviceID string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	d, ok := r.devices[deviceID]
	if !ok {
		return ErrDeviceNotFound
	}

	d.Status = StatusApproved
	return r.save()
}

// Revoke — отзывает доступ устройства.
func (r *DeviceRegistry) Revoke(deviceID string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	d, ok := r.devices[deviceID]
	if !ok {
		return ErrDeviceNotFound
	}

	d.Status = StatusRevoked
	return r.save()
}

// List — возвращает список устройств владельца.
func (r *DeviceRegistry) List(ownerID string) []*Device {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var result []*Device
	for _, d := range r.devices {
		if ownerID == "" || d.OwnerID == ownerID {
			cp := *d
			result = append(result, &cp)
		}
	}
	return result
}

// Get — возвращает устройство по ID.
func (r *DeviceRegistry) Get(deviceID string) (*Device, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	d, ok := r.devices[deviceID]
	if !ok {
		return nil, ErrDeviceNotFound
	}
	cp := *d
	return &cp, nil
}

// IsApproved — проверяет что устройство подтверждено.
func (r *DeviceRegistry) IsApproved(deviceID string) bool {
	r.mu.RLock()
	defer r.mu.RUnlock()

	d, ok := r.devices[deviceID]
	return ok && d.Status == StatusApproved
}

// save — сохраняет реестр в JSON файл.
func (r *DeviceRegistry) save() error {
	data, err := json.MarshalIndent(r.devices, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(r.dir, "devices.json"), data, 0600)
}

// load — загружает реестр из JSON файла.
func (r *DeviceRegistry) load() error {
	data, err := os.ReadFile(filepath.Join(r.dir, "devices.json"))
	if err != nil {
		return err
	}
	return json.Unmarshal(data, &r.devices)
}

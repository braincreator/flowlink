package autoscale

import (
	"encoding/json"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// ManagedServer — информация об управляемом сервере.
type ManagedServer struct {
	ServerID     int       `json:"server_id"`
	Role         string    `json:"role"`          // "relay", "load_balancer"
	Status       string    `json:"status"`        // "active", "draining", "removing"
	AddedAt      time.Time `json:"added_at"`
	LastAction   string    `json:"last_action"`
	LastActionAt time.Time `json:"last_action_at"`
}

// ScaleRegistry — персистентное хранилище состояния autoscaling.
type ScaleRegistry struct {
	servers map[int]*ManagedServer
	dataDir string
	mu      sync.RWMutex
	logger  *slog.Logger
}

// NewScaleRegistry создаёт новый registry.
func NewScaleRegistry(dataDir string) (*ScaleRegistry, error) {
	if dataDir == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return nil, err
		}
		dataDir = filepath.Join(home, ".config", "flowlink")
	}

	if err := os.MkdirAll(dataDir, 0700); err != nil {
		return nil, err
	}

	r := &ScaleRegistry{
		servers: make(map[int]*ManagedServer),
		dataDir: dataDir,
		logger:  slog.Default(),
	}

	if err := r.load(); err != nil {
		r.logger.Warn("registry load failed, starting fresh", "error", err)
	}

	return r, nil
}

func (r *ScaleRegistry) filePath() string {
	return filepath.Join(r.dataDir, "autoscale.json")
}

func (r *ScaleRegistry) load() error {
	data, err := os.ReadFile(r.filePath())
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	var servers []*ManagedServer
	if err := json.Unmarshal(data, &servers); err != nil {
		return err
	}

	r.mu.Lock()
	defer r.mu.Unlock()
	for _, s := range servers {
		r.servers[s.ServerID] = s
	}
	return nil
}

// Save сохраняет состояние в файл.
func (r *ScaleRegistry) Save() error {
	r.mu.RLock()
	servers := make([]*ManagedServer, 0, len(r.servers))
	for _, s := range r.servers {
		servers = append(servers, s)
	}
	r.mu.RUnlock()

	data, err := json.MarshalIndent(servers, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(r.filePath(), data, 0600)
}

// AddServer добавляет сервер в registry.
func (r *ScaleRegistry) AddServer(s *ManagedServer) {
	r.mu.Lock()
	defer r.mu.Unlock()
	s.AddedAt = time.Now()
	r.servers[s.ServerID] = s
}

// RemoveServer удаляет сервер из registry.
func (r *ScaleRegistry) RemoveServer(id int) {
	r.mu.Lock()
	defer r.mu.Unlock()
	delete(r.servers, id)
}

// GetServer возвращает сервер по ID.
func (r *ScaleRegistry) GetServer(id int) *ManagedServer {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.servers[id]
}

// GetAllServers возвращает все управляемые серверы.
func (r *ScaleRegistry) GetAllServers() []*ManagedServer {
	r.mu.RLock()
	defer r.mu.RUnlock()
	result := make([]*ManagedServer, 0, len(r.servers))
	for _, s := range r.servers {
		result = append(result, s)
	}
	return result
}

// GetActiveServers возвращает активные серверы.
func (r *ScaleRegistry) GetActiveServers() []*ManagedServer {
	r.mu.RLock()
	defer r.mu.RUnlock()
	var result []*ManagedServer
	for _, s := range r.servers {
		if s.Status == "active" {
			result = append(result, s)
		}
	}
	return result
}

// UpdateAction обновляет последнее действие сервера.
func (r *ScaleRegistry) UpdateAction(id int, action string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if s, ok := r.servers[id]; ok {
		s.LastAction = action
		s.LastActionAt = time.Now()
	}
}

// ActiveCount возвращает количество активных серверов.
func (r *ScaleRegistry) ActiveCount() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	count := 0
	for _, s := range r.servers {
		if s.Status == "active" {
			count++
		}
	}
	return count
}

// SetStatus обновляет статус сервера.
func (r *ScaleRegistry) SetStatus(id int, status string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if s, ok := r.servers[id]; ok {
		s.Status = status
	}
}

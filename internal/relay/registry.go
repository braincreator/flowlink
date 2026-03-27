// Package relay — реестр клиентов и агентов для multi-tenancy.
// JSONL persistence, thread-safe, CRUD операции.
package relay

import (
	"bufio"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/google/uuid"
)

// === Модели данных ===

// Client — клиент (tenant) в системе.
type Client struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Email     string    `json:"email"`
	Plan      string    `json:"plan"`       // starter, business, enterprise
	APIToken  string    `json:"api_token"`  // сгенерированный токен
	MaxAgents int       `json:"max_agents"` // лимит агентов по тарифу
	CreatedAt time.Time `json:"created_at"`
	IsActive  bool      `json:"is_active"`
}

// AgentRegistration — зарегистрированный агент в реестре.
type AgentRegistration struct {
	ID         string    `json:"id"`
	ClientID   string    `json:"client_id"`
	Label      string    `json:"label"`      // hostname
	Tags       []string  `json:"tags"`       // ["production", "nginx"]
	OS         string    `json:"os"`
	Arch       string    `json:"arch"`
	Version    string    `json:"version"`
	Token      string    `json:"token"`      // токен для подключения
	CreatedAt  time.Time `json:"created_at"`
	LastSeenAt time.Time `json:"last_seen_at"`
	IsOnline   bool      `json:"is_online"`
}

// === Registry ===

// Registry — реестр клиентов и агентов с JSONL persistence.
type Registry struct {
	mu         sync.RWMutex
	clients    map[string]*Client            // client_id → Client
	agents     map[string]*AgentRegistration // agent_id → AgentRegistration
	clientIdx  map[string][]string           // client_id → []agent_id
	dataDir    string                        // директория для JSONL файлов
	logger     *slog.Logger
}

// NewRegistry — создаёт новый реестр, загружает данные из файла.
func NewRegistry(dataDir string, logger *slog.Logger) *Registry {
	if logger == nil {
		logger = slog.Default()
	}

	// Создаём директорию если нет
	os.MkdirAll(dataDir, 0700)

	reg := &Registry{
		clients:   make(map[string]*Client),
		agents:    make(map[string]*AgentRegistration),
		clientIdx: make(map[string][]string),
		dataDir:   dataDir,
		logger:    logger,
	}

	// Загружаем из persistence
	if err := reg.Load(); err != nil {
		logger.Warn("ошибка загрузки реестра, начинаем с пустого", "err", err)
	}

	return reg
}

// === Клиенты ===

// CreateClient — создаёт нового клиента с сгенерированным API токеном.
func (r *Registry) CreateClient(name, email, plan string) (*Client, error) {
	r.mu.Lock()
	defer r.mu.Unlock()

	// Лимиты агентов по тарифу
	maxAgents := 3 // starter по умолчанию
	switch plan {
	case "business":
		maxAgents = 25
	case "enterprise":
		maxAgents = 100
	}

	// Генерируем API токен
	token, err := generateAPIToken()
	if err != nil {
		return nil, fmt.Errorf("генерация токена: %w", err)
	}

	client := &Client{
		ID:        uuid.New().String(),
		Name:      name,
		Email:     email,
		Plan:      plan,
		APIToken:  token,
		MaxAgents: maxAgents,
		CreatedAt: time.Now(),
		IsActive:  true,
	}

	r.clients[client.ID] = client

	// Персистим
	if err := r.appendJSONL("clients.jsonl", client); err != nil {
		r.logger.Error("ошибка сохранения клиента", "err", err)
	}

	r.logger.Info("клиент создан", "id", client.ID, "name", name, "plan", plan)
	return client, nil
}

// GetClient — возвращает клиента по ID.
func (r *Registry) GetClient(id string) (*Client, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	c, ok := r.clients[id]
	return c, ok
}

// GetClientByAPIToken — ищет клиента по API токену.
func (r *Registry) GetClientByAPIToken(token string) (*Client, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	for _, c := range r.clients {
		if c.APIToken == token && c.IsActive {
			return c, true
		}
	}
	return nil, false
}

// ListClients — возвращает список всех клиентов.
func (r *Registry) ListClients() []*Client {
	r.mu.RLock()
	defer r.mu.RUnlock()
	result := make([]*Client, 0, len(r.clients))
	for _, c := range r.clients {
		result = append(result, c)
	}
	return result
}

// DeactivateClient — деактивирует клиента.
func (r *Registry) DeactivateClient(id string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	c, ok := r.clients[id]
	if !ok {
		return fmt.Errorf("клиент не найден: %s", id)
	}

	c.IsActive = false

	// Персистим обновление
	if err := r.appendJSONL("clients.jsonl", c); err != nil {
		r.logger.Error("ошибка сохранения деактивации", "err", err)
	}

	r.logger.Info("клиент деактивирован", "id", id)
	return nil
}

// === Агенты ===

// RegisterAgent — регистрирует нового агента для клиента, возвращает токен.
func (r *Registry) RegisterAgent(clientID, label string, tags []string, os, arch string) (*AgentRegistration, error) {
	r.mu.Lock()
	defer r.mu.Unlock()

	// Проверяем клиента
	client, ok := r.clients[clientID]
	if !ok || !client.IsActive {
		return nil, fmt.Errorf("клиент не найден или деактивирован: %s", clientID)
	}

	// Проверяем лимит агентов
	currentCount := len(r.clientIdx[clientID])
	if currentCount >= client.MaxAgents {
		return nil, fmt.Errorf("лимит агентов превышен (%d/%d)", currentCount, client.MaxAgents)
	}

	// Генерируем токен
	token, err := generateAPIToken()
	if err != nil {
		return nil, fmt.Errorf("генерация токена: %w", err)
	}

	now := time.Now()
	agent := &AgentRegistration{
		ID:         uuid.New().String(),
		ClientID:   clientID,
		Label:      label,
		Tags:       tags,
		OS:         os,
		Arch:       arch,
		Token:      token,
		CreatedAt:  now,
		LastSeenAt: now,
		IsOnline:   false,
	}

	r.agents[agent.ID] = agent
	r.clientIdx[clientID] = append(r.clientIdx[clientID], agent.ID)

	// Персистим
	if err := r.appendJSONL("agents.jsonl", agent); err != nil {
		r.logger.Error("ошибка сохранения агента", "err", err)
	}

	r.logger.Info("агент зарегистрирован", "id", agent.ID, "client", clientID, "label", label)
	return agent, nil
}

// GetAgent — возвращает агента по ID.
func (r *Registry) GetAgent(id string) (*AgentRegistration, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	a, ok := r.agents[id]
	return a, ok
}

// GetAgentByToken — ищет агента по токену подключения.
func (r *Registry) GetAgentByToken(token string) (*AgentRegistration, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	for _, a := range r.agents {
		if a.Token == token {
			return a, true
		}
	}
	return nil, false
}

// ListAgents — возвращает список агентов клиента.
func (r *Registry) ListAgents(clientID string) []*AgentRegistration {
	r.mu.RLock()
	defer r.mu.RUnlock()
	ids := r.clientIdx[clientID]
	result := make([]*AgentRegistration, 0, len(ids))
	for _, id := range ids {
		if a, ok := r.agents[id]; ok {
			result = append(result, a)
		}
	}
	return result
}

// ListAllAgents — возвращает список всех агентов.
func (r *Registry) ListAllAgents() []*AgentRegistration {
	r.mu.RLock()
	defer r.mu.RUnlock()
	result := make([]*AgentRegistration, 0, len(r.agents))
	for _, a := range r.agents {
		result = append(result, a)
	}
	return result
}

// UnregisterAgent — удаляет агента из реестра.
func (r *Registry) UnregisterAgent(id string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	agent, ok := r.agents[id]
	if !ok {
		return fmt.Errorf("агент не найден: %s", id)
	}

	// Удаляем из индекса клиента
	clientID := agent.ClientID
	ids := r.clientIdx[clientID]
	for i, aid := range ids {
		if aid == id {
			r.clientIdx[clientID] = append(ids[:i], ids[i+1:]...)
			break
		}
	}

	delete(r.agents, id)

	// Записываем tombstone
	if err := r.appendJSONL("agents.jsonl", map[string]string{"_deleted": id}); err != nil {
		r.logger.Error("ошибка сохранения удаления", "err", err)
	}

	r.logger.Info("агент удалён", "id", id, "client", clientID)
	return nil
}

// UpdateAgentTags — обновляет теги агента.
func (r *Registry) UpdateAgentTags(id string, tags []string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	agent, ok := r.agents[id]
	if !ok {
		return fmt.Errorf("агент не найден: %s", id)
	}

	agent.Tags = tags

	// Персистим
	if err := r.appendJSONL("agents.jsonl", agent); err != nil {
		r.logger.Error("ошибка сохранения тегов", "err", err)
	}

	return nil
}

// GetAgentByClientAndLabel — ищет агента по clientID и label (hostname).
func (r *Registry) GetAgentByClientAndLabel(clientID, label string) (*AgentRegistration, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	ids := r.clientIdx[clientID]
	for _, id := range ids {
		a, ok := r.agents[id]
		if ok && a.Label == label {
			return a, true
		}
	}
	return nil, false
}

// UpdateAgentOnlineStatus — обновляет LastSeenAt и IsOnline при подключении.
func (r *Registry) UpdateAgentOnlineStatus(agentID string, online bool) {
	r.mu.Lock()
	defer r.mu.Unlock()

	agent, ok := r.agents[agentID]
	if !ok {
		return
	}

	agent.IsOnline = online
	if online {
		agent.LastSeenAt = time.Now()
	}

	// Персистим
	if err := r.appendJSONL("agents.jsonl", agent); err != nil {
		r.logger.Error("ошибка сохранения статуса", "err", err)
	}
}

// === Persistence (JSONL) ===

// Load — загружает данные из JSONL файлов.
func (r *Registry) Load() error {
	// Загружаем клиентов
	if err := r.loadJSONL("clients.jsonl", func(data []byte) {
		var c Client
		if json.Unmarshal(data, &c) == nil {
			r.clients[c.ID] = &c
		}
	}); err != nil {
		return fmt.Errorf("загрузка клиентов: %w", err)
	}

	// Загружаем агентов
	if err := r.loadJSONL("agents.jsonl", func(data []byte) {
		// Проверяем tombstone
		var tombstone map[string]string
		if json.Unmarshal(data, &tombstone) == nil {
			if _, deleted := tombstone["_deleted"]; deleted {
				delete(r.agents, tombstone["_deleted"])
				return
			}
		}

		var a AgentRegistration
		if json.Unmarshal(data, &a) == nil && a.ID != "" {
			r.agents[a.ID] = &a
			// Восстанавливаем индекс
			r.clientIdx[a.ClientID] = append(r.clientIdx[a.ClientID], a.ID)
		}
	}); err != nil {
		return fmt.Errorf("загрузка агентов: %w", err)
	}

	r.logger.Info("реестр загружен", "clients", len(r.clients), "agents", len(r.agents))
	return nil
}

// Save — пересохраняет весь реестр (compaction).
func (r *Registry) Save() error {
	r.mu.RLock()
	defer r.mu.RUnlock()

	// Клиенты
	if err := r.saveJSONL("clients.jsonl", func() []any {
		list := make([]any, 0, len(r.clients))
		for _, c := range r.clients {
			list = append(list, c)
		}
		return list
	}); err != nil {
		return err
	}

	// Агенты
	if err := r.saveJSONL("agents.jsonl", func() []any {
		list := make([]any, 0, len(r.agents))
		for _, a := range r.agents {
			list = append(list, a)
		}
		return list
	}); err != nil {
		return err
	}

	return nil
}

// === Вспомогательные методы ===

// loadJSONL — читает JSONL файл и вызывает callback для каждой строки.
func (r *Registry) loadJSONL(filename string, callback func([]byte)) error {
	path := filepath.Join(r.dataDir, filename)
	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil // файла нет — нормально
		}
		return err
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	// Увеличиваем буфер для больших строк
	scanner.Buffer(make([]byte, 0, 1024*1024), 10*1024*1024)

	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		callback(line)
	}

	return scanner.Err()
}

// appendJSONL — дописывает запись в JSONL файл.
func (r *Registry) appendJSONL(filename string, record any) error {
	path := filepath.Join(r.dataDir, filename)
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		return err
	}
	defer f.Close()

	data, err := json.Marshal(record)
	if err != nil {
		return err
	}

	_, err = f.Write(append(data, '\n'))
	return err
}

// saveJSONL — полностью перезаписывает JSONL файл (compaction).
func (r *Registry) saveJSONL(filename string, records func() []any) error {
	path := filepath.Join(r.dataDir, filename)
	tmp := path + ".tmp"

	f, err := os.Create(tmp)
	if err != nil {
		return err
	}
	defer f.Close()

	enc := json.NewEncoder(f)
	for _, rec := range records() {
		if err := enc.Encode(rec); err != nil {
			return err
		}
	}

	// Атомарная замена
	return os.Rename(tmp, path)
}

// generateAPIToken — генерирует случайный API токен (24 байта, base64url).
func generateAPIToken() (string, error) {
	b := make([]byte, 24)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

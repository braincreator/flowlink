// Package relay — реле-сервер flowlink.
// Принимает WSS-подключения от агентов, предоставляет HTTP API для OpenClaw.
package relay

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/google/uuid"
	"github.com/gorilla/websocket"
)

// Relay — реле-сервер, связывающий агентов и OpenClaw.
type Relay struct {
	cfg       *config.RelayConfig
	logger    *slog.Logger
	pool      *AgentPool
	llmProxy  *LLMProxy
}

// AgentConn — подключённый агент.
type AgentConn struct {
	ID        string
	Hostname  string
	OS        string
	Arch      string
	Version   string
	Connected time.Time
	LastSeen  time.Time
	conn      *websocket.Conn
	mu        sync.Mutex
	callbacks map[string]func(any) // request_id → callback
}

// AgentPool — пул подключённых агентов.
type AgentPool struct {
	mu     sync.RWMutex
	agents map[string]*AgentConn // agentID → connection
}

// NewAgentPool — создаёт новый пул.
func NewAgentPool() *AgentPool {
	return &AgentPool{
		agents: make(map[string]*AgentConn),
	}
}

// Add — добавляет агента в пул.
func (p *AgentPool) Add(agent *AgentConn) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.agents[agent.ID] = agent
}

// Remove — удаляет агента из пула.
func (p *AgentPool) Remove(agentID string) {
	p.mu.Lock()
	defer p.mu.Unlock()
	delete(p.agents, agentID)
}

// Get — получает агента по ID.
func (p *AgentPool) Get(agentID string) (*AgentConn, bool) {
	p.mu.RLock()
	defer p.mu.RUnlock()
	agent, ok := p.agents[agentID]
	return agent, ok
}

// List — возвращает список всех подключённых агентов.
func (p *AgentPool) List() []*AgentConn {
	p.mu.RLock()
	defer p.mu.RUnlock()
	result := make([]*AgentConn, 0, len(p.agents))
	for _, ag := range p.agents {
		result = append(result, ag)
	}
	return result
}

// Count — количество подключённых агентов.
func (p *AgentPool) Count() int {
	p.mu.RLock()
	defer p.mu.RUnlock()
	return len(p.agents)
}

// SendMessage — отправляет сообщение агенту.
func (a *AgentConn) SendMessage(msg protocol.Message) error {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.conn.WriteJSON(msg)
}

// NewRelay — создаёт новый реле-сервер.
func NewRelay(cfg *config.RelayConfig) *Relay {
	return &Relay{
		cfg:    cfg,
		logger: slog.Default(),
		pool:   NewAgentPool(),
	}
}

// SetLLMProxy — устанавливает LLM proxy.
func (r *Relay) SetLLMProxy(proxy *LLMProxy) {
	r.llmProxy = proxy
}

// Start — запускает WSS-сервер для агентов.
func (r *Relay) Start() error {
	// WSS endpoint для агентов
	http.HandleFunc("/ws", r.handleAgentWS)

	// HTTP API для OpenClaw
	apiMux := http.NewServeMux()
	apiMux.HandleFunc("/api/v1/agents", r.handleListAgents)
	apiMux.HandleFunc("/api/v1/agents/exec", r.handleExecCommand)
	apiMux.HandleFunc("/api/v1/agents/files/read", r.handleFileRead)
	apiMux.HandleFunc("/api/v1/agents/files/write", r.handleFileWrite)
	apiMux.HandleFunc("/api/v1/agents/files/list", r.handleFileList)
	apiMux.HandleFunc("/api/v1/agents/sysinfo", r.handleSysInfo)
	apiMux.HandleFunc("/api/v1/agents/task", r.handleTaskSubmit)
	apiMux.HandleFunc("/api/v1/agents/task/cancel", r.handleTaskCancel)
	apiMux.HandleFunc("/api/v1/agents/skills/push", r.handleSkillPush)
	apiMux.HandleFunc("/api/v1/agents/skills/list", r.handleSkillList)
	apiMux.HandleFunc("/api/v1/agents/skills/delete", r.handleSkillDelete)
	apiMux.HandleFunc("/api/v1/llm/chat", r.handleLLMChat)
	apiMux.HandleFunc("/api/v1/llm/backends", r.handleLLMBackends)
	apiMux.HandleFunc("/api/v1/llm/health", r.handleLLMHealth)
	apiMux.HandleFunc("/mcp", r.handleMCP)

	// Auth middleware
	authMux := r.authMiddleware(apiMux)

	// Запуск
	r.logger.Info("запуск реле-сервера",
		"wss", r.cfg.WSSAddr,
		"api", r.cfg.APIAddr,
	)

	go func() {
		r.logger.Info("WSS сервер запущен", "addr", r.cfg.WSSAddr)
		if err := http.ListenAndServe(r.cfg.WSSAddr, nil); err != nil {
			r.logger.Error("WSS сервер ошибка", "err", err)
		}
	}()

	r.logger.Info("HTTP API запущен", "addr", r.cfg.APIAddr)
	return http.ListenAndServe(r.cfg.APIAddr, authMux)
}

// handleAgentWS — обрабатывает WSS-подключение от агента.
func (r *Relay) handleAgentWS(w http.ResponseWriter, req *http.Request) {
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	conn, err := upgrader.Upgrade(w, req, nil)
	if err != nil {
		r.logger.Error("ошибка апгрейда WSS", "err", err)
		return
	}

	// Читаем первое сообщение (connect)
	var connectMsg protocol.Message
	if err := conn.ReadJSON(&connectMsg); err != nil {
		r.logger.Error("ошибка чтения connect", "err", err)
		conn.Close()
		return
	}

	if connectMsg.Type != protocol.MsgConnect {
		r.logger.Error("первое сообщение не connect", "type", connectMsg.Type)
		conn.Close()
		return
	}

	// Парсим payload
	var payload protocol.ConnectPayload
	if err := json.Unmarshal(jsonMarshal(connectMsg.Payload), &payload); err != nil {
		r.logger.Error("ошибка парсинга connect payload", "err", err)
		conn.Close()
		return
	}

	// Проверяем токен
	if !r.authenticateAgent(payload.AgentID, payload.Token) {
		r.logger.Warn("агент не авторизован", "agent", payload.AgentID)
		conn.Close()
		return
	}

	// Добавляем в пул
	agent := &AgentConn{
		ID:        payload.AgentID,
		Hostname:  payload.Hostname,
		OS:        payload.OS,
		Arch:      payload.Arch,
		Version:   payload.ClientVer,
		Connected: time.Now(),
		LastSeen:  time.Now(),
		conn:      conn,
	}

	r.pool.Add(agent)
	r.logger.Info("агент подключён", "agent", payload.AgentID,
		"hostname", payload.Hostname, "os", payload.OS)

	// Отправляем подтверждение
	resp := protocol.NewMessage(protocol.MsgConnected)
	resp.Payload = protocol.ConnectedPayload{
		AgentID:    payload.AgentID,
		RelayID:    "relay-1",
		Interval:   30,
		ServerTime: time.Now().Unix(),
	}
	conn.WriteJSON(resp)

	// Цикл чтения сообщений от агента
	defer func() {
		r.pool.Remove(payload.AgentID)
		conn.Close()
		r.logger.Info("агент отключён", "agent", payload.AgentID)
	}()

	for {
		var msg protocol.Message
		if err := conn.ReadJSON(&msg); err != nil {
			r.logger.Error("ошибка чтения от агента", "agent", payload.AgentID, "err", err)
			return
		}

		msg.AgentID = payload.AgentID
		agent.LastSeen = time.Now()

		r.logger.Debug("сообщение от агента", "agent", msg.AgentID, "type", msg.Type)

		// Проверяем callback (для MCP sendAndWait)
		if msg.Payload != nil {
			if m, ok := msg.Payload.(map[string]any); ok {
				if reqID, ok := m["request_id"].(string); ok {
					if agent.TriggerCallback(reqID, msg.Payload) {
						continue
					}
				}
			}
		}
	}
}

// authenticateAgent — проверяет токен агента.
func (r *Relay) authenticateAgent(agentID, token string) bool {
	if r.cfg.AllowedTokens == nil {
		return true // нет whitelist = принимаем всех (для dev)
	}

	allowedID, ok := r.cfg.AllowedTokens[token]
	if !ok {
		return false
	}

	if allowedID != "" && allowedID != agentID {
		return false
	}

	return true
}

// === HTTP API handlers ===

func (r *Relay) handleListAgents(w http.ResponseWriter, req *http.Request) {
	agents := r.pool.List()

	type agentInfo struct {
		ID        string `json:"id"`
		Hostname  string `json:"hostname"`
		OS        string `json:"os"`
		Arch      string `json:"arch"`
		Version   string `json:"version"`
		Connected string `json:"connected"`
		LastSeen  string `json:"last_seen"`
	}

	list := make([]agentInfo, len(agents))
	for i, ac := range agents {
		list[i] = agentInfo{
			ID:        ac.ID,
			Hostname:  ac.Hostname,
			OS:        ac.OS,
			Arch:      ac.Arch,
			Version:   ac.Version,
			Connected: ac.Connected.Format(time.RFC3339),
			LastSeen:  ac.LastSeen.Format(time.RFC3339),
		}
	}

	writeJSON(w, map[string]any{
		"agents": list,
		"count":  len(list),
	})
}

func (r *Relay) handleExecCommand(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID  string `json:"agent_id"`
		Command  string `json:"command"`
		Shell    string `json:"shell,omitempty"`
		Dir      string `json:"dir,omitempty"`
		Env      map[string]string `json:"env,omitempty"`
		Timeout  int    `json:"timeout_sec"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	if body.AgentID == "" || body.Command == "" {
		writeError(w, http.StatusBadRequest, "agent_id и command обязательны")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgExecRequest)
	msg.Payload = protocol.ExecRequestPayload{
		Command:   body.Command,
		Shell:     body.Shell,
		Dir:       body.Dir,
		Env:       body.Env,
		Timeout:   body.Timeout,
		RequestID: uuid.New().String(),
	}

	if err := agent.SendMessage(msg); err != nil {
		writeError(w, http.StatusBadGateway, "ошибка отправки команды: "+err.Error())
		return
	}

	writeJSON(w, map[string]string{
		"status":    "sent",
		"request_id": msg.Payload.(protocol.ExecRequestPayload).RequestID,
		"agent_id":  body.AgentID,
	})
}

func (r *Relay) handleFileRead(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID  string `json:"agent_id"`
		Path     string `json:"path"`
		Encoding string `json:"encoding,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgFileRead)
	msg.Payload = protocol.FileReadPayload{
		Path:     body.Path,
		Encoding: body.Encoding,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeError(w, http.StatusBadGateway, "ошибка: "+err.Error())
		return
	}

	writeJSON(w, map[string]string{"status": "sent", "agent_id": body.AgentID})
}

func (r *Relay) handleFileWrite(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID  string `json:"agent_id"`
		Path     string `json:"path"`
		Content  string `json:"content"`
		Encoding string `json:"encoding"`
		Mode     int    `json:"mode,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgFileWrite)
	msg.Payload = protocol.FileWritePayload{
		Path:     body.Path,
		Content:  body.Content,
		Encoding: body.Encoding,
		Mode:     body.Mode,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeError(w, http.StatusBadGateway, "ошибка: "+err.Error())
		return
	}

	writeJSON(w, map[string]string{"status": "sent", "agent_id": body.AgentID})
}

func (r *Relay) handleFileList(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID string `json:"agent_id"`
		Path    string `json:"path"`
		Depth   int    `json:"depth,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgFileList)
	msg.Payload = protocol.FileListPayload{
		Path:  body.Path,
		Depth: body.Depth,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeError(w, http.StatusBadGateway, "ошибка: "+err.Error())
		return
	}

	writeJSON(w, map[string]string{"status": "sent", "agent_id": body.AgentID})
}

func (r *Relay) handleSysInfo(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID string `json:"agent_id"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgSysInfo)
	if err := agent.SendMessage(msg); err != nil {
		writeError(w, http.StatusBadGateway, "ошибка: "+err.Error())
		return
	}

	writeJSON(w, map[string]string{"status": "sent", "agent_id": body.AgentID})
}

// authMiddleware — проверяет Bearer токен для HTTP API.
func (r *Relay) authMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, req *http.Request) {
		if r.cfg.APIToken == "" {
			next.ServeHTTP(w, req)
			return
		}

		token := req.Header.Get("Authorization")
		if token == "" {
			writeError(w, http.StatusUnauthorized, "токен не указан")
			return
		}

		// Убираем "Bearer " префикс
		if len(token) > 7 && token[:7] == "Bearer " {
			token = token[7:]
		}

		if token != r.cfg.APIToken {
			writeError(w, http.StatusUnauthorized, "неверный токен")
			return
		}

		next.ServeHTTP(w, req)
	})
}

// === Вспомогательные функции ===

func (r *Relay) handleTaskSubmit(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID      string `json:"agent_id"`
		SkillID      string `json:"skill_id,omitempty"`
		Description  string `json:"description"`
		LLMProvider  string `json:"llm_provider,omitempty"`
		LLMModel     string `json:"llm_model,omitempty"`
		LLMAPIKey    string `json:"llm_api_key,omitempty"`
		MaxSteps     int    `json:"max_steps,omitempty"`
		MaxDuration  int    `json:"max_duration_sec,omitempty"`
		AutoApprove  bool   `json:"auto_approve_safe,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	if body.AgentID == "" || body.Description == "" {
		writeError(w, http.StatusBadRequest, "agent_id и description обязательны")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	taskID := uuid.New().String()

	msg := protocol.NewMessage(protocol.MsgTask)
	msg.Payload = protocol.TaskPayload{
		TaskID:       taskID,
		SkillID:      body.SkillID,
		Description:  body.Description,
		LLMProvider:  body.LLMProvider,
		LLMModel:     body.LLMModel,
		LLMAPIKey:    body.LLMAPIKey,
		MaxSteps:     body.MaxSteps,
		MaxDuration:  body.MaxDuration,
		AutoApprove:  body.AutoApprove,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeError(w, http.StatusBadGateway, "ошибка: "+err.Error())
		return
	}

	writeJSON(w, map[string]string{
		"status":   "submitted",
		"task_id":  taskID,
		"agent_id": body.AgentID,
	})
}

func (r *Relay) handleTaskCancel(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID string `json:"agent_id"`
		TaskID  string `json:"task_id"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgTaskCancel)
	msg.Payload = map[string]string{"task_id": body.TaskID}
	agent.SendMessage(msg)

	writeJSON(w, map[string]string{"status": "cancel_sent", "task_id": body.TaskID})
}

func (r *Relay) handleSkillPush(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID     string `json:"agent_id"`
		SkillID     string `json:"skill_id"`
		Name        string `json:"name"`
		Description string `json:"description"`
		Instructions string `json:"instructions"`
		ToolsAllowed []string `json:"tools_allowed"`
		ForceUpdate bool   `json:"force_update,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	if body.AgentID == "" || body.SkillID == "" || body.Instructions == "" {
		writeError(w, http.StatusBadRequest, "agent_id, skill_id и instructions обязательны")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgSkillPush)
	msg.Payload = protocol.SkillPushPayload{
		SkillID:       body.SkillID,
		Name:          body.Name,
		Description:   body.Description,
		Instructions:  body.Instructions,
		ToolsAllowed:  body.ToolsAllowed,
		ForceUpdate:   body.ForceUpdate,
	}
	agent.SendMessage(msg)

	writeJSON(w, map[string]string{"status": "pushed", "skill_id": body.SkillID})
}

func (r *Relay) handleSkillList(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID string `json:"agent_id"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgSkillList)
	agent.SendMessage(msg)

	writeJSON(w, map[string]string{"status": "requested", "agent_id": body.AgentID})
}

func (r *Relay) handleSkillDelete(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID string `json:"agent_id"`
		SkillID string `json:"skill_id"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, "агент не подключён")
		return
	}

	msg := protocol.NewMessage(protocol.MsgSkillDelete)
	msg.Payload = map[string]string{"skill_id": body.SkillID}
	agent.SendMessage(msg)

	writeJSON(w, map[string]string{"status": "delete_requested", "skill_id": body.SkillID})
}

// === Вспомогательные функции ===

func writeJSON(w http.ResponseWriter, data any) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(data)
}

func writeError(w http.ResponseWriter, code int, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	json.NewEncoder(w).Encode(map[string]string{
		"error": message,
		"code":  fmt.Sprintf("%d", code),
	})
}

func jsonMarshal(v any) []byte {
	data, _ := json.Marshal(v)
	return data
}

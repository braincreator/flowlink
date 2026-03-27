// Package relay — реле-сервер flowlink.
// Принимает WSS-подключения от агентов, предоставляет HTTP API для OpenClaw.
package relay

import (
	"crypto/tls"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/dashboard"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/google/uuid"
	"github.com/gorilla/websocket"
)

// Relay — реле-сервер, связывающий агенты и OpenClaw.
type Relay struct {
	cfg         *config.RelayConfig
	logger      *slog.Logger
	pool        *AgentPool
	llmProxy    *LLMProxy
	auth        *AuthManager
	rateLimit   *RateLimiter
	audit       *AuditLogger
	registry    *Registry            // реестр клиентов и агентов (multi-tenancy)
	planStore   *billing.PlanStore   // тарифные планы
	usage       *billing.UsageTracker
	invoices    *billing.InvoiceStore
	eventBus   *EventBus   // шина событий для SSE-уведомлений
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

// SetCallback — устанавливает callback для request_id.
func (a *AgentConn) SetCallback(requestID string, callback func(any)) {
	a.mu.Lock()
	defer a.mu.Unlock()

	if a.callbacks == nil {
		a.callbacks = make(map[string]func(any))
	}
	a.callbacks[requestID] = callback
}

// NewRelay — создаёт новый реле-сервер.
func NewRelay(cfg *config.RelayConfig) *Relay {
	logger := slog.Default()

	// Инициализируем audit logger
	audit, err := NewAuditLogger("")
	if err != nil {
		logger.Error("ошибка инициализации audit logger", "err", err)
	}
	
	// Инициализируем реестр (multi-tenancy)
	registryDir := filepath.Join(os.Getenv("HOME"), ".flowlink", "registry")
	registry := NewRegistry(registryDir, logger)

	// Инициализируем биллинг
	billingDir := filepath.Join(os.Getenv("HOME"), ".flowlink", "billing")
	planStore := billing.NewPlanStore()
	usageTracker := billing.NewUsageTracker(billingDir, planStore, logger)
	invoiceStore := billing.NewInvoiceStore(billingDir, planStore, logger)

	return &Relay{
		cfg:       cfg,
		logger:    logger,
		pool:      NewAgentPool(),
		auth:      NewAuthManager(logger),
		rateLimit: NewRateLimiter(30, 200, logger), // 30/min, 200/hour
		audit:     audit,
		registry:  registry,
		eventBus:  NewEventBus(logger),
		planStore: planStore,
		usage:     usageTracker,
		invoices:  invoiceStore,
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

	// SSE events endpoint (должен быть ДО audit, т.к. middleware chain применяется к apiMux)
	apiMux.HandleFunc("/api/v1/events", r.handleSSE)

	// Audit log endpoints
	apiMux.HandleFunc("/api/v1/audit", r.handleAuditQuery)
	apiMux.HandleFunc("/api/v1/audit/export", r.handleAuditExport)
	apiMux.HandleFunc("/api/v1/audit/stats", r.handleAuditStats)

	// Registry endpoints (multi-tenancy)
	apiMux.HandleFunc("/api/v1/clients", r.handleClients)                  // POST — создать, GET — список
	apiMux.HandleFunc("/api/v1/clients/", r.handleClientByID)              // GET /api/v1/clients/{id}, POST /api/v1/clients/{id}/agents
	apiMux.HandleFunc("/api/v1/agents/register", r.handleAgentRegister)    // POST — зарегистрировать агента
	apiMux.HandleFunc("/api/v1/agents/delete/", r.handleAgentDelete)       // DELETE /api/v1/agents/delete/{id}

	// Billing endpoints
	apiMux.HandleFunc("/api/v1/billing/usage", r.handleBillingUsage)
	apiMux.HandleFunc("/api/v1/billing/plan", r.handleBillingPlan)
	apiMux.HandleFunc("/api/v1/billing/plan/change", r.handleBillingPlanChange)
	apiMux.HandleFunc("/api/v1/billing/invoices", r.handleBillingInvoices)
	apiMux.HandleFunc("/api/v1/billing/invoices/", r.handleBillingInvoicePay)
	apiMux.HandleFunc("/api/v1/billing/payment-methods", r.handleBillingPaymentMethods)

	// Middleware chain
	authCfg := AuthMiddlewareConfig{
		AuthManager: r.auth,
		StaticToken: r.cfg.APIToken,
		Logger:      r.logger,
	}

	handler := Chain(
		RecoveryMiddleware(r.logger),
		RequestLoggerMiddleware(r.logger),
		CORSMiddleware(nil, r.logger), // nil = разрешаем все origins
		RateLimitMiddleware(r.rateLimit, r.logger),
		AuthMiddleware(authCfg),
	)(apiMux)

	// Dashboard
	dashProvider := &dashboardProvider{r: r}
	http.Handle("/dashboard/", http.StripPrefix("/dashboard", dashboard.NewHandler(dashProvider)))

	// Инициализируем TLS если нужно
	var tlsConfig *tls.Config
	if r.cfg.TLSMode != "" {
		mode := TLSMode(r.cfg.TLSMode)
		certManager := NewCertManager(
			mode,
			r.cfg.TLSCert,
			r.cfg.TLSKey,
			r.cfg.TLSDomain,
			r.cfg.TLSCache,
		)

		var err error
		tlsConfig, err = certManager.GetTLSConfig()
		if err != nil {
			r.logger.Error("ошибка конфигурации TLS", "err", err)
			return err
		}
		r.logger.Info("TLS настроен", "mode", mode)
	}

	// Запуск
	r.logger.Info("запуск реле-сервера",
		"wss", r.cfg.WSSAddr,
		"api", r.cfg.APIAddr,
		"tls_mode", r.cfg.TLSMode,
	)

	// WSS сервер
	go func() {
		var wssServer *http.Server
		if tlsConfig != nil {
			wssServer = &http.Server{
				Addr:      r.cfg.WSSAddr,
				TLSConfig: tlsConfig,
			}
			r.logger.Info("WSS сервер запущен (TLS)", "addr", r.cfg.WSSAddr)
		} else {
			wssServer = &http.Server{
				Addr: r.cfg.WSSAddr,
			}
			r.logger.Info("WSS сервер запущен (без TLS)", "addr", r.cfg.WSSAddr)
		}

		if err := wssServer.ListenAndServe(); err != nil {
			r.logger.Error("WSS сервер ошибка", "err", err)
		}
	}()

	// HTTP API сервер
	r.logger.Info("HTTP API запущен", "addr", r.cfg.APIAddr)
	if tlsConfig != nil {
		return http.ListenAndServeTLS(r.cfg.APIAddr, "", "", handler)
	}
	return http.ListenAndServe(r.cfg.APIAddr, handler)
}

// HandleAgentWSForTest — экспортированная версия handleAgentWS для тестов.
func (r *Relay) HandleAgentWSForTest(w http.ResponseWriter, req *http.Request) {
	r.handleAgentWS(w, req)
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

	// Публикуем событие подключения
	r.eventBus.Publish(Event{
		Type:    EventAgentConnected,
		AgentID: payload.AgentID,
		Data: map[string]interface{}{
			"hostname": payload.Hostname,
			"os":       payload.OS,
			"arch":     payload.Arch,
			"version":  payload.ClientVer,
		},
	})

	// Обновляем статус в реестре
	if r.registry != nil {
		r.registry.UpdateAgentOnlineStatus(payload.AgentID, true)
	}

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
		// Обновляем статус в реестре
		if r.registry != nil {
			r.registry.UpdateAgentOnlineStatus(payload.AgentID, false)
		}
		r.logger.Info("агент отключён", "agent", payload.AgentID)

		// Публикуем событие отключения
		r.eventBus.Publish(Event{
			Type:    EventAgentDisconnected,
			AgentID: payload.AgentID,
		})
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
// Сначала проверяет реестр (multi-tenancy), затем fallback на старые методы.
func (r *Relay) authenticateAgent(agentID, token string) bool {
	// Вариант 0: Проверка через реестр (multi-tenancy)
	if r.registry != nil {
		if agent, ok := r.registry.GetAgentByToken(token); ok && agent.ClientID != "" {
			// Проверяем что клиент активен
			if client, ok := r.registry.GetClient(agent.ClientID); ok && client.IsActive {
				return true
			}
		}
	}

	// Вариант 1: Проверка через AuthManager (динамические токены)
	if r.auth != nil {
		valid, err := r.auth.ValidateAgentToken(agentID, token)
		if err == nil && valid {
			return true
		}
	}

	// Вариант 2: Проверка через whitelist в конфиге (статические токены)
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
	startTime := time.Now()

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
		// Логируем ошибку
		if r.audit != nil {
			r.audit.Log(AuditEntry{
				Timestamp:  startTime,
				AgentID:   body.AgentID,
				ClientID:  getClientID(req),
				Action:     "exec",
				Command:   body.Command,
				RiskLevel:  "medium",
				Result:     "error",
				DurationMs: time.Since(startTime).Milliseconds(),
				Error:      err.Error(),
				ClientIP:   getClientIP(req),
			})
		}

		r.eventBus.Publish(Event{
			Type:    EventError,
			AgentID: body.AgentID,
			ClientID: getClientID(req),
			Data: map[string]interface{}{
				"action": "exec",
				"error":  err.Error(),
			},
		})

		writeError(w, http.StatusBadGateway, "ошибка отправки команды: "+err.Error())
		return
	}

	// Логируем успешную отправку
	if r.audit != nil {
		r.audit.Log(AuditEntry{
			Timestamp:  startTime,
			AgentID:   body.AgentID,
			ClientID:  getClientID(req),
			Action:     "exec",
			Command:   body.Command,
			RiskLevel:  "medium",
			Result:     "success",
			DurationMs: time.Since(startTime).Milliseconds(),
			ClientIP:   getClientIP(req),
		})
	}

	// Публикуем событие старта выполнения
	r.eventBus.Publish(Event{
		Type:    EventExecStart,
		AgentID: body.AgentID,
		ClientID: getClientID(req),
		Data: map[string]interface{}{
			"command":    body.Command,
			"request_id": msg.Payload.(protocol.ExecRequestPayload).RequestID,
		},
	})

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

// GenerateAgentToken — генерирует токен для агента (публичный метод).
func (r *Relay) GenerateAgentToken(agentID string, expiresInSeconds int64) (string, error) {
	return r.auth.GenerateAgentToken(agentID, expiresInSeconds)
}

// GenerateAPIToken — генерирует API токен (публичный метод).
func (r *Relay) GenerateAPIToken(clientID string, expiresInSeconds int64) (string, error) {
	return r.auth.GenerateAPIToken(clientID, expiresInSeconds)
}

// RotateAgentTokens — ротация токена агента (публичный метод).
func (r *Relay) RotateAgentTokens(agentID string, expiresInSeconds int64) (string, error) {
	return r.auth.RotateTokens(agentID, expiresInSeconds)
}

// RevokeToken — отзыв токена (публичный метод).
func (r *Relay) RevokeToken(token string) error {
	return r.auth.RevokeToken(token)
}

// === Audit Log Handlers ===

func (r *Relay) handleAuditQuery(w http.ResponseWriter, req *http.Request) {
	if r.audit == nil {
		writeError(w, http.StatusServiceUnavailable, "audit logger не инициализирован")
		return
	}

	// Парсим query parameters
	query := AuditQuery{
		AgentID:   req.URL.Query().Get("agent_id"),
		ClientID:  req.URL.Query().Get("client_id"),
		Action:    req.URL.Query().Get("action"),
		RiskLevel: req.URL.Query().Get("risk_level"),
		Result:    req.URL.Query().Get("result"),
	}

	// Limit и offset
	if limit := req.URL.Query().Get("limit"); limit != "" {
		if l, err := parseInt(limit); err == nil {
			query.Limit = l
		}
	}
	if offset := req.URL.Query().Get("offset"); offset != "" {
		if o, err := parseInt(offset); err == nil {
			query.Offset = o
		}
	}

	// Date range
	if from := req.URL.Query().Get("from"); from != "" {
		if t, err := time.Parse(time.RFC3339, from); err == nil {
			query.From = &t
		}
	}
	if to := req.URL.Query().Get("to"); to != "" {
		if t, err := time.Parse(time.RFC3339, to); err == nil {
			query.To = &t
		}
	}

	// Запрашиваем
	entries, err := r.audit.Query(query)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "ошибка запроса: "+err.Error())
		return
	}

	writeJSON(w, map[string]any{
		"entries": entries,
		"count":   len(entries),
	})
}

func (r *Relay) handleAuditExport(w http.ResponseWriter, req *http.Request) {
	if r.audit == nil {
		writeError(w, http.StatusServiceUnavailable, "audit logger не инициализирован")
		return
	}

	format := req.URL.Query().Get("format")
	if format == "" {
		format = "json"
	}

	query := AuditQuery{
		AgentID:   req.URL.Query().Get("agent_id"),
		ClientID:  req.URL.Query().Get("client_id"),
		Action:    req.URL.Query().Get("action"),
		RiskLevel: req.URL.Query().Get("risk_level"),
		Result:    req.URL.Query().Get("result"),
	}

	// Date range
	if from := req.URL.Query().Get("from"); from != "" {
		if t, err := time.Parse(time.RFC3339, from); err == nil {
			query.From = &t
		}
	}
	if to := req.URL.Query().Get("to"); to != "" {
		if t, err := time.Parse(time.RFC3339, to); err == nil {
			query.To = &t
		}
	}

	// Экспортируем
	data, err := r.audit.Export(format, query)
	if err != nil {
		writeError(w, http.StatusBadRequest, "ошибка экспорта: "+err.Error())
		return
	}

	// Устанавливаем content type и filename
	switch format {
	case "csv":
		w.Header().Set("Content-Type", "text/csv")
		w.Header().Set("Content-Disposition", "attachment; filename=audit-export.csv")
	default:
		w.Header().Set("Content-Type", "application/json")
		w.Header().Set("Content-Disposition", "attachment; filename=audit-export.json")
	}

	w.Write(data)
}

func (r *Relay) handleAuditStats(w http.ResponseWriter, req *http.Request) {
	if r.audit == nil {
		writeError(w, http.StatusServiceUnavailable, "audit logger не инициализирован")
		return
	}

	stats, err := r.audit.Stats()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "ошибка статистики: "+err.Error())
		return
	}

	writeJSON(w, stats)
}

// === Registry HTTP Handlers (Multi-tenancy) ===

// handleClients — POST: создать клиента, GET: список клиентов.
func (r *Relay) handleClients(w http.ResponseWriter, req *http.Request) {
	switch req.Method {
	case http.MethodPost:
		var body struct {
			Name  string `json:"name"`
			Email string `json:"email"`
			Plan  string `json:"plan"`
		}
		if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
			writeError(w, http.StatusBadRequest, "неверный JSON")
			return
		}
		if body.Name == "" {
			writeError(w, http.StatusBadRequest, "name обязателен")
			return
		}
		if body.Plan == "" {
			body.Plan = "starter"
		}

		client, err := r.registry.CreateClient(body.Name, body.Email, body.Plan)
		if err != nil {
			writeError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeJSON(w, client)

	case http.MethodGet:
		clients := r.registry.ListClients()
		writeJSON(w, map[string]any{
			"clients": clients,
			"count":   len(clients),
		})

	default:
		writeError(w, http.StatusMethodNotAllowed, "метод не поддерживается")
	}
}

// handleClientByID — GET: клиент по ID, POST: зарегистрировать агента для клиента.
// Маршрутизация по пути: /api/v1/clients/{id} или /api/v1/clients/{id}/agents
func (r *Relay) handleClientByID(w http.ResponseWriter, req *http.Request) {
	// Извлекаем clientID из пути: /api/v1/clients/{id}/... или /api/v1/clients/{id}
	path := strings.TrimPrefix(req.URL.Path, "/api/v1/clients/")
	parts := strings.SplitN(path, "/", 2)
	clientID := parts[0]
	if clientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}

	// /api/v1/clients/{id}/agents
	if len(parts) == 2 && parts[1] == "agents" {
		switch req.Method {
		case http.MethodGet:
			agents := r.registry.ListAgents(clientID)
			writeJSON(w, map[string]any{
				"agents": agents,
				"count":  len(agents),
			})
		case http.MethodPost:
			var body struct {
				Label string   `json:"label"`
				Tags  []string `json:"tags"`
				OS    string   `json:"os"`
				Arch  string   `json:"arch"`
			}
			if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
				writeError(w, http.StatusBadRequest, "неверный JSON")
				return
			}
			if body.Label == "" {
				writeError(w, http.StatusBadRequest, "label обязателен")
				return
			}
			if body.Tags == nil {
				body.Tags = []string{}
			}

			agent, err := r.registry.RegisterAgent(clientID, body.Label, body.Tags, body.OS, body.Arch)
			if err != nil {
				writeError(w, http.StatusBadRequest, err.Error())
				return
			}
			writeJSON(w, agent)
		default:
			writeError(w, http.StatusMethodNotAllowed, "метод не поддерживается")
		}
		return
	}

	// /api/v1/clients/{id}
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "метод не поддерживается")
		return
	}

	client, ok := r.registry.GetClient(clientID)
	if !ok {
		writeError(w, http.StatusNotFound, "клиент не найден")
		return
	}
	writeJSON(w, client)
}

// handleAgentRegister — POST: зарегистрировать агента (альтернативный endpoint).
func (r *Relay) handleAgentRegister(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, "метод не поддерживается")
		return
	}

	var body struct {
		ClientID string   `json:"client_id"`
		Label    string   `json:"label"`
		Tags     []string `json:"tags"`
		OS       string   `json:"os"`
		Arch     string   `json:"arch"`
	}
	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}
	if body.ClientID == "" || body.Label == "" {
		writeError(w, http.StatusBadRequest, "client_id и label обязательны")
		return
	}
	if body.Tags == nil {
		body.Tags = []string{}
	}

	agent, err := r.registry.RegisterAgent(body.ClientID, body.Label, body.Tags, body.OS, body.Arch)
	if err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, agent)
}

// handleAgentDelete — DELETE: удалить агента по ID.
func (r *Relay) handleAgentDelete(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodDelete {
		writeError(w, http.StatusMethodNotAllowed, "метод не поддерживается")
		return
	}

	agentID := strings.TrimPrefix(req.URL.Path, "/api/v1/agents/delete/")
	if agentID == "" {
		writeError(w, http.StatusBadRequest, "agent_id обязателен")
		return
	}

	if err := r.registry.UnregisterAgent(agentID); err != nil {
		writeError(w, http.StatusNotFound, err.Error())
		return
	}
	writeJSON(w, map[string]string{"status": "deleted", "agent_id": agentID})
}

// === Billing HTTP Handlers ===

// handleBillingUsage — GET /api/v1/billing/usage?client_id=X
func (r *Relay) handleBillingUsage(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "только GET")
		return
	}
	clientID := req.URL.Query().Get("client_id")
	if clientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}
	// Определяем план клиента
	planID := "free"
	if client, ok := r.registry.GetClient(clientID); ok {
		planID = client.Plan
	}
	usage := r.usage.GetUsage(clientID, currentBillingMonth())
	checks := map[string]billing.LimitCheck{
		"commands": r.usage.CheckLimit(clientID, billing.ResourceCommands, planID),
		"agents":   r.usage.CheckLimit(clientID, billing.ResourceAgents, planID),
		"backups":  r.usage.CheckLimit(clientID, billing.ResourceBackups, planID),
		"storage":  r.usage.CheckLimit(clientID, billing.ResourceStorage, planID),
	}
	writeJSON(w, map[string]any{
		"usage":  usage,
		"limits": checks,
	})
}

// handleBillingPlan — GET /api/v1/billing/plan?client_id=X
func (r *Relay) handleBillingPlan(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "только GET")
		return
	}
	clientID := req.URL.Query().Get("client_id")
	if clientID == "" {
		// Возвращаем все планы
		writeJSON(w, map[string]any{
			"plans": r.planStore.ListPlans(),
		})
		return
	}
	planID := "free"
	if client, ok := r.registry.GetClient(clientID); ok {
		planID = client.Plan
	}
	plan, ok := r.planStore.GetPlan(planID)
	if !ok {
		writeError(w, http.StatusNotFound, "план не найден")
		return
	}
	writeJSON(w, plan)
}

// handleBillingPlanChange — POST /api/v1/billing/plan/change
func (r *Relay) handleBillingPlanChange(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, "только POST")
		return
	}
	var body struct {
		ClientID string `json:"client_id"`
		PlanID   string `json:"plan_id"`
	}
	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}
	if body.ClientID == "" || body.PlanID == "" {
		writeError(w, http.StatusBadRequest, "client_id и plan_id обязательны")
		return
	}
	if _, ok := r.planStore.GetPlan(body.PlanID); !ok {
		writeError(w, http.StatusBadRequest, "план не найден")
		return
	}
	client, ok := r.registry.GetClient(body.ClientID)
	if !ok {
		writeError(w, http.StatusNotFound, "клиент не найден")
		return
	}
	client.Plan = body.PlanID
	writeJSON(w, map[string]any{
		"status":   "changed",
		"plan_id":  body.PlanID,
		"client_id": body.ClientID,
	})
}

// handleBillingInvoices — GET /api/v1/billing/invoices?client_id=X
func (r *Relay) handleBillingInvoices(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "только GET")
		return
	}
	clientID := req.URL.Query().Get("client_id")
	if clientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}
	invoices := r.invoices.ListInvoices(clientID)
	writeJSON(w, map[string]any{
		"invoices": invoices,
		"count":    len(invoices),
	})
}

// handleBillingInvoicePay — POST /api/v1/billing/invoices/{id}/pay
func (r *Relay) handleBillingInvoicePay(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, "только POST")
		return
	}
	invoiceID := strings.TrimPrefix(req.URL.Path, "/api/v1/billing/invoices/")
	invoiceID = strings.TrimSuffix(invoiceID, "/pay")
	if invoiceID == "" {
		writeError(w, http.StatusBadRequest, "invoice_id обязателен")
		return
	}
	if err := r.invoices.MarkPaid(invoiceID); err != nil {
		writeError(w, http.StatusNotFound, err.Error())
		return
	}
	writeJSON(w, map[string]string{"status": "paid", "invoice_id": invoiceID})
}

// handleBillingPaymentMethods — GET /api/v1/billing/payment-methods?client_id=X
func (r *Relay) handleBillingPaymentMethods(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "только GET")
		return
	}
	clientID := req.URL.Query().Get("client_id")
	if clientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}
	methods := r.invoices.ListPaymentMethods(clientID)
	writeJSON(w, map[string]any{
		"payment_methods": methods,
		"count":           len(methods),
	})
}

func currentBillingMonth() string {
	return time.Now().Format("2006-01")
}

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

func parseInt(s string) (int, error) {
	var result int
	_, err := fmt.Sscanf(s, "%d", &result)
	return result, err
}

func getClientID(req *http.Request) string {
	// Из JWT токена или Authorization header
	auth := req.Header.Get("Authorization")
	if auth != "" {
		// Убираем "Bearer " prefix
		if len(auth) > 7 && strings.ToLower(auth[:7]) == "bearer " {
			return auth[7:]
		}
		return auth
	}
	return ""
}

func getClientIP(req *http.Request) string {
	if xff := req.Header.Get("X-Forwarded-For"); xff != "" {
		parts := strings.Split(xff, ",")
		return strings.TrimSpace(parts[0])
	}
	if xri := req.Header.Get("X-Real-IP"); xri != "" {
		return xri
	}
	// Убираем порт
	addr := req.RemoteAddr
	if idx := strings.LastIndex(addr, ":"); idx != -1 {
		return addr[:idx]
	}
	return addr
}

// dashboardProvider — реализует dashboard.DataProvider для избежания import cycle.
type dashboardProvider struct {
	r *Relay
}

func (dp *dashboardProvider) DashboardAgents() []dashboard.AgentInfo {
	agents := dp.r.registry.ListAllAgents()
	connected := dp.r.pool.List()
	onlineSet := make(map[string]bool)
	for _, c := range connected {
		onlineSet[c.ID] = true
	}
	result := make([]dashboard.AgentInfo, len(agents))
	for i, a := range agents {
		result[i] = dashboard.AgentInfo{
			ID: a.ID, ClientID: a.ClientID, Label: a.Label,
			Tags: a.Tags, OS: a.OS, Arch: a.Arch, Version: a.Version,
			IsOnline: onlineSet[a.ID],
			LastSeenAt: a.LastSeenAt.Format("2006-01-02T15:04:05Z07:00"),
		}
	}
	return result
}

func (dp *dashboardProvider) DashboardClients() []dashboard.ClientInfo {
	clients := dp.r.registry.ListClients()
	result := make([]dashboard.ClientInfo, len(clients))
	for i, c := range clients {
		result[i] = dashboard.ClientInfo{
			ID: c.ID, Name: c.Name, Email: c.Email,
			Plan: c.Plan, APIToken: c.APIToken,
			MaxAgents: c.MaxAgents, IsActive: c.IsActive,
		}
	}
	return result
}

func (dp *dashboardProvider) DashboardAuditStats() *dashboard.AuditStatsInfo {
	stats, err := dp.r.audit.Stats()
	if err != nil {
		return &dashboard.AuditStatsInfo{}
	}
	recent, _ := dp.r.audit.Recent(100)
	entries := make([]dashboard.AuditEntryInfo, len(recent))
	for i, e := range recent {
		entries[i] = dashboard.AuditEntryInfo{
			Timestamp:  e.Timestamp.Format("2006-01-02T15:04:05Z07:00"),
			AgentID:    e.AgentID,
			Action:     e.Action,
			Command:    e.Command,
			Result:     e.Result,
			DurationMs: e.DurationMs,
		}
	}
	return &dashboard.AuditStatsInfo{
		TotalEntries: stats.TotalEntries, ByAction: stats.ByAction,
		Last24hCount: stats.Last24hCount, Entries: entries,
	}
}

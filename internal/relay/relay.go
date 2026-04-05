// Package relay — реле-сервер flowlink.
// Принимает WSS-подключения от агентов, предоставляет HTTP API для OpenClaw.
package relay

import (
	"context"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/dashboard"
	"github.com/braincreator/flowlink/internal/health"
	"github.com/braincreator/flowlink/internal/nginx"
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
	eventBus      *EventBus        // шина событий для SSE-уведомлений
	approvalQueue *ApprovalQueue  // очередь запросов на подтверждение
	healthChecker  *health.HealthChecker // мониторинг здоровья компонентов
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
	if a.conn == nil {
		return fmt.Errorf("websocket connection is nil")
	}
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
		logger.Error("audit logger init failed", "err", err)
	}
	
	// Инициализируем реестр (multi-tenancy)
	registryDir := filepath.Join(os.Getenv("HOME"), ".flowlink", "registry")
	registry := NewRegistry(registryDir, logger)

	// Инициализируем биллинг
	billingDir := filepath.Join(os.Getenv("HOME"), ".flowlink", "billing")
	planStore := billing.NewPlanStore()
	usageTracker := billing.NewUsageTracker(billingDir, planStore, logger)
	invoiceStore := billing.NewInvoiceStore(billingDir, planStore, logger)

	// Инициализируем health checker
	hc := health.NewHealthChecker("0.1.0")
	hc.SetWSSAddr(cfg.WSSAddr)
	hc.SetAPIAddr(cfg.APIAddr)

	r := &Relay{
		cfg:       cfg,
		logger:    logger,
		pool:      NewAgentPool(),
		auth:      NewAuthManager(logger),
		rateLimit: NewRateLimiter(cfg.RateLimitPerMin, cfg.RateLimitPerHour, logger),
		audit:     audit,
		registry:  registry,
		eventBus:      NewEventBus(logger),
		approvalQueue: NewApprovalQueue(NewEventBus(logger), logger, filepath.Join(os.Getenv("HOME"), ".flowlink")),
		planStore:     planStore,
		usage:     usageTracker,
		invoices:  invoiceStore,
		healthChecker:  hc,
	}

	// Подключаем адаптеры health checker к реальным компонентам relay
	hc.SetAgentPool(poolForHealth{pool: r.pool})
	hc.SetAuthManager(authForHealth{auth: r.auth})
	hc.SetAuditLogger(auditForHealth{audit: r.audit})
	hc.SetRegistry(registryForHealth{registry: r.registry})

	return r
}

// SetLLMProxy — устанавливает LLM proxy.
func (r *Relay) SetLLMProxy(proxy *LLMProxy) {
	r.llmProxy = proxy
}

// CreateFirstClient — создаёт первого клиента (для setup wizard).
func (r *Relay) CreateFirstClient(name, email string) (*Client, error) {
	return r.registry.CreateClient(name, email, "starter")
}

// CreateFirstAgent — создаёт первого агента (для setup wizard).
func (r *Relay) CreateFirstAgent(clientID, label string) (*AgentRegistration, error) {
	return r.registry.RegisterAgent(clientID, label, []string{"default"}, runtime.GOOS, runtime.GOARCH)
}

// PoolList — возвращает список подключённых агентов (для тестов).
func (r *Relay) PoolList() []*AgentConn {
	return r.pool.List()
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
	apiMux.HandleFunc("/api/v1/agents/config", r.handleAgentConfigUpdate) // PUT — обновить конфиг агента
	apiMux.HandleFunc("/api/v1/agents/backup", r.handleBackupCreate)           // POST — trigger backup
	apiMux.HandleFunc("/api/v1/agents/backup/list", r.handleBackupList)      // GET — list snapshots
	apiMux.HandleFunc("/api/v1/agents/backup/", r.handleBackupOperations)    // POST /restore, DELETE /{id}
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
	apiMux.HandleFunc("/api/v1/approvals", r.handleApprovalsList)
	apiMux.HandleFunc("/api/v1/approvals/", r.handleApprovalAction)

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

	// Nginx config generator endpoint
	apiMux.HandleFunc("/api/v1/nginx-config", r.handleNginxConfig)

	// Rate Limit endpoints
	apiMux.HandleFunc("/api/v1/rate-limits", r.handleRateLimits)           // GET — список, POST — сброс статистики
	apiMux.HandleFunc("/api/v1/rate-limits/", r.handleRateLimitByClient)  // GET/PUT/DELETE для конкретного клиента
	apiMux.HandleFunc("/api/v1/rate-limits/stats", r.handleRateLimitStats) // GET — общая статистика

	// Health check endpoints
	apiMux.HandleFunc("/api/v1/health", r.handleHealth)         // GET — полный отчёт
	apiMux.HandleFunc("/api/v1/health/ready", r.handleHealthReady) // GET — 200/503
	apiMux.HandleFunc("/api/v1/health/live", r.handleHealthLive)  // GET — 200/503

	// Dashboard (с авторизацией через API token, регистрируем ДО middleware chain)
	dashProvider := &dashboardProvider{r: r}
	apiMux.Handle("/dashboard/", http.StripPrefix("/dashboard", dashboard.NewHandler(dashProvider, r.cfg.APIToken)))

	// Middleware chain (dashboard имеет собственную авторизацию)
	authCfg := AuthMiddlewareConfig{
		AuthManager: r.auth,
		StaticToken: r.cfg.APIToken,
		Logger:      r.logger,
		SkipPaths:   []string{"/dashboard/", "/dashboard"},
	}

	handler := Chain(
		RecoveryMiddleware(r.logger),
		RequestLoggerMiddleware(r.logger),
		CORSMiddleware(nil, r.logger), // nil = разрешаем все origins
		RateLimitMiddleware(r.rateLimit, r.logger),
		AuthMiddleware(authCfg),
	)(apiMux)

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
			r.logger.Error("TLS configuration error", "err", err)
			return err
		}
		r.logger.Info("TLS настроен", "mode", mode)
	}

	// Запуск
	r.logger.Info("relay server starting",
		"wss", r.cfg.WSSAddr,
		"api", r.cfg.APIAddr,
		"tls_mode", r.cfg.TLSMode,
	)

	// Контекст с graceful shutdown
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	// WSS сервер
	wssServer := &http.Server{Addr: r.cfg.WSSAddr}
	if tlsConfig != nil {
		wssServer.TLSConfig = tlsConfig
		r.logger.Info("WSS server started (TLS)", "addr", r.cfg.WSSAddr)
	} else {
		r.logger.Info("WSS server started (no TLS)", "addr", r.cfg.WSSAddr)
	}
	wssErr := make(chan error, 1)
	go func() {
		if tlsConfig != nil {
			wssErr <- wssServer.ListenAndServeTLS("", "")
		} else {
			wssErr <- wssServer.ListenAndServe()
		}
	}()

	// HTTP API сервер
	apiServer := &http.Server{Addr: r.cfg.APIAddr, Handler: handler}
	apiErr := make(chan error, 1)
	go func() {
		r.logger.Info("HTTP API started", "addr", r.cfg.APIAddr)
		if tlsConfig != nil {
			apiErr <- apiServer.ListenAndServeTLS("", "")
		} else {
			apiErr <- apiServer.ListenAndServe()
		}
	}()

	// Ждём shutdown signal или ошибку
	var serverErr error
	select {
	case <-ctx.Done():
		r.logger.Info("shutdown signal received, starting graceful shutdown...")
	case err := <-wssErr:
		if err != nil && err != http.ErrServerClosed {
			serverErr = fmt.Errorf("WSS: %w", err)
		}
	case err := <-apiErr:
		if err != nil && err != http.ErrServerClosed {
			serverErr = fmt.Errorf("API: %w", err)
		}
	}

	// Graceful shutdown: 30 секунд на завершение
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer shutdownCancel()

	// 1. Compacting registry
	r.logger.Info("saving registry...")
	if err := r.registry.Save(); err != nil {
		r.logger.Error("registry save error", "err", err)
	}

	// 2. Shutdown servers
	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		defer wg.Done()
		if err := wssServer.Shutdown(shutdownCtx); err != nil {
			r.logger.Error("WSS shutdown error", "err", err)
		}
	}()
	go func() {
		defer wg.Done()
		if err := apiServer.Shutdown(shutdownCtx); err != nil {
			r.logger.Error("API shutdown error", "err", err)
		}
	}()
	wg.Wait()

	r.logger.Info("graceful shutdown завершён")
	return serverErr
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
		r.logger.Error("WSS upgrade error", "err", err)
		return
	}

	// Читаем первое сообщение (connect)
	var connectMsg protocol.Message
	if err := conn.ReadJSON(&connectMsg); err != nil {
		r.logger.Error("connect read error", "err", err)
		conn.Close()
		return
	}

	if connectMsg.Type != protocol.MsgConnect {
		r.logger.Error("first message is not connect", "type", connectMsg.Type)
		conn.Close()
		return
	}

	// Парсим payload
	var payload protocol.ConnectPayload
	if err := json.Unmarshal(jsonMarshal(connectMsg.Payload), &payload); err != nil {
		r.logger.Error("connect payload parse error", "err", err)
		conn.Close()
		return
	}

	// Проверяем токен
	if !r.authenticateAgent(payload.AgentID, payload.Token) {
		r.logger.Warn("agent not authorized", "agent", payload.AgentID)
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
	r.logger.Info("agent connected", "agent", payload.AgentID,
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
		r.logger.Info("agent disconnected", "agent", payload.AgentID)

		// Публикуем событие отключения
		r.eventBus.Publish(Event{
			Type:    EventAgentDisconnected,
			AgentID: payload.AgentID,
		})
	}()

	for {
		var msg protocol.Message
		if err := conn.ReadJSON(&msg); err != nil {
			r.logger.Error("agent read error", "agent", payload.AgentID, "err", err)
			return
		}

		msg.AgentID = payload.AgentID
		agent.LastSeen = time.Now()

		r.logger.Debug("message from agent", "agent", msg.AgentID, "type", msg.Type)

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

		// Обработка сообщений от агента
		switch msg.Type {
		case protocol.MsgHeartbeat:
			ack := protocol.NewMessage(protocol.MsgHeartbeatAck)
			ack.AgentID = msg.AgentID
			conn.WriteJSON(ack)

		case protocol.MsgNeedsApproval, protocol.MsgApprovalRequest:
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			// Store in approval queue
			cmd, _ := payloadData["command"].(string)
			risk, _ := payloadData["risk_level"].(string)
			mode, _ := payloadData["approval_mode"].(string)
			if cmd == "" {
				cmd, _ = payloadData["description"].(string)
			}
			r.approvalQueue.Add(msg.AgentID, cmd, risk, mode)

			r.eventBus.Publish(Event{
				Type:    EventApprovalRequired,
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgBackupResponse:
			// Ответ на запрос создания бэкапа
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    EventBackupCreated,
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgBackupListResp:
			// Ответ на запрос списка снапшотов
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    EventBackupList,
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgBackupProgress:
			// Прогресс создания бэкапа (SSE event)
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    EventBackupProgress,
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgExecDone:
			// Выполнение команды завершено
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    EventExecComplete,
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgExecOutput:
			// Вывод выполнения команды
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    "exec.output",
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgSysInfoResp:
			// Ответ на запрос системной информации
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    "sysinfo.response",
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgFileResponse:
			// Ответ на запрос файла
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    "file.response",
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgConfigAck:
			// Подтверждение обновления конфигурации
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    EventAgentConfigUpdated,
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgTaskProgress:
			// Прогресс выполнения задачи
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    "task.progress",
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgTaskDone:
			// Задача завершена
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    "task.complete",
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		case protocol.MsgSkillList:
			// Ответ на запрос списка навыков
			// Сначала проверяем callback (для request/response паттерна)
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					if reqID, ok := m["request_id"].(string); ok {
						if agent.TriggerCallback(reqID, msg.Payload) {
							continue
						}
					}
				}
			}
			var payloadData map[string]any
			if msg.Payload != nil {
				if m, ok := msg.Payload.(map[string]any); ok {
					payloadData = m
				}
			}
			r.eventBus.Publish(Event{
				Type:    "skill.list",
				AgentID: msg.AgentID,
				Data:    payloadData,
			})

		default:
			// Для неизвестных сообщений — логируем
			r.logger.Debug("unknown message type from agent", "agent", msg.AgentID, "type", msg.Type)
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.AgentID == "" || body.Command == "" {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidPayload)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
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

		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeAgentWriteFailed, err.Error())
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	msg := protocol.NewMessage(protocol.MsgFileRead)
	msg.Payload = protocol.FileReadPayload{
		Path:     body.Path,
		Encoding: body.Encoding,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeInternalError, err.Error())
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
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
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeInternalError, err.Error())
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	msg := protocol.NewMessage(protocol.MsgFileList)
	msg.Payload = protocol.FileListPayload{
		Path:  body.Path,
		Depth: body.Depth,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeInternalError, err.Error())
		return
	}

	writeJSON(w, map[string]string{"status": "sent", "agent_id": body.AgentID})
}

func (r *Relay) handleSysInfo(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID string `json:"agent_id"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	msg := protocol.NewMessage(protocol.MsgSysInfo)
	if err := agent.SendMessage(msg); err != nil {
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeInternalError, err.Error())
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.AgentID == "" || body.Description == "" {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidPayload)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
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
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeInternalError, err.Error())
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.AgentID == "" || body.SkillID == "" || body.Instructions == "" {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidPayload)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	msg := protocol.NewMessage(protocol.MsgSkillDelete)
	msg.Payload = map[string]string{"skill_id": body.SkillID}
	agent.SendMessage(msg)

	writeJSON(w, map[string]string{"status": "delete_requested", "skill_id": body.SkillID})
}

// handleAgentConfigUpdate — PUT /api/v1/agents/config: обновить конфигурацию агента.
// Отправляет MsgConfigUpdate агенту через WS.
func (r *Relay) handleAgentConfigUpdate(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPut {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	var body struct {
		AgentID    string         `json:"agent_id"`
		ReadOnly   *bool          `json:"read_only"`
		Label      *string        `json:"label"`
		WorkDir    *string        `json:"work_dir"`
		KillSwitch map[string]any `json:"kill_switch"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}
	if body.AgentID == "" {
		writeError(w, http.StatusBadRequest, "agent_id обязателен")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	// Собираем payload — только переданные поля
	payload := map[string]any{}
	if body.ReadOnly != nil {
		payload["read_only"] = *body.ReadOnly
	}
	if body.Label != nil {
		payload["label"] = *body.Label
	}
	if body.WorkDir != nil {
		payload["work_dir"] = *body.WorkDir
	}
	if body.KillSwitch != nil {
		payload["kill_switch"] = body.KillSwitch
	}

	if len(payload) == 0 {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidPayload)
		return
	}

	// Отправляем MsgConfigUpdate агенту
	msg := protocol.NewMessage(protocol.MsgConfigUpdate)
	msg.Payload = payload
	if err := agent.SendMessage(msg); err != nil {
		r.logger.Error("config update send failed", "err", err, "agent", body.AgentID)
		writeError(w, http.StatusBadGateway, protocol.CodeConfigFailed)
		return
	}

	// Publish SSE event
	r.eventBus.Publish(Event{
		Type:    EventAgentConfigUpdated,
		AgentID: body.AgentID,
		Data:    payload,
	})

	writeJSON(w, map[string]any{
		"status":   "config_update_sent",
		"agent_id": body.AgentID,
		"fields":   payload,
	})
}

// === Backup API Handlers ===

// handleBackupCreate — POST /api/v1/agents/backup: trigger backup на агенте.
func (r *Relay) handleBackupCreate(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	var body struct {
		AgentID     string   `json:"agent_id"`
		Description string   `json:"description,omitempty"`
		Paths       []string `json:"paths,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.AgentID == "" {
		writeError(w, http.StatusBadRequest, "agent_id обязателен")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	requestID := uuid.New().String()
	msg := protocol.NewMessage(protocol.MsgBackupRequest)
	msg.Payload = protocol.BackupRequestPayload{
		RequestID:   requestID,
		Description: body.Description,
		Paths:       body.Paths,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeAgentWriteFailed, err.Error())
		return
	}

	writeJSON(w, map[string]string{
		"status":    "sent",
		"request_id": requestID,
		"agent_id":  body.AgentID,
	})
}

// handleBackupList — GET /api/v1/agents/backup/list: list snapshots.
func (r *Relay) handleBackupList(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	agentID := req.URL.Query().Get("agent_id")
	if agentID == "" {
		writeError(w, http.StatusBadRequest, "agent_id обязателен")
		return
	}

	agent, ok := r.pool.Get(agentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	requestID := uuid.New().String()
	msg := protocol.NewMessage(protocol.MsgBackupList)
	msg.Payload = protocol.BackupListPayload{
		RequestID: requestID,
	}

	if err := agent.SendMessage(msg); err != nil {
		writeErrorCustom(w, http.StatusBadGateway, protocol.CodeAgentWriteFailed, err.Error())
		return
	}

	writeJSON(w, map[string]string{
		"status":    "sent",
		"request_id": requestID,
		"agent_id":  agentID,
	})
}

// handleBackupOperations — POST /api/v1/agents/backup/{id}/restore, DELETE /api/v1/agents/backup/{id}.
func (r *Relay) handleBackupOperations(w http.ResponseWriter, req *http.Request) {
	// Парсим путь: /api/v1/agents/backup/{id}/restore или /api/v1/agents/backup/{id}
	path := strings.TrimPrefix(req.URL.Path, "/api/v1/agents/backup/")
	parts := strings.SplitN(path, "/", 2)

	snapshotID := parts[0]
	if snapshotID == "" {
		writeError(w, http.StatusBadRequest, "snapshot_id обязателен")
		return
	}

	// Определяем операцию
	var operation string
	if len(parts) == 2 && parts[1] == "restore" {
		operation = "restore"
	} else if req.Method == http.MethodDelete {
		operation = "delete"
	} else {
		writeError(w, http.StatusBadRequest, protocol.CodeUnknownError)
		return
	}

	// Читаем тело для agent_id
	var body struct {
		AgentID string `json:"agent_id"`
	}
	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.AgentID == "" {
		writeError(w, http.StatusBadRequest, "agent_id обязателен")
		return
	}

	agent, ok := r.pool.Get(body.AgentID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeAgentNotConnected)
		return
	}

	requestID := uuid.New().String()

	switch operation {
	case "restore":
		if req.Method != http.MethodPost {
			writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
			return
		}
		msg := protocol.NewMessage(protocol.MsgBackupRestore)
		msg.Payload = protocol.BackupRestorePayload{
			RequestID:  requestID,
			SnapshotID: snapshotID,
		}
		if err := agent.SendMessage(msg); err != nil {
			writeErrorCustom(w, http.StatusBadGateway, protocol.CodeAgentWriteFailed, err.Error())
			return
		}
		writeJSON(w, map[string]string{
			"status":     "sent",
			"request_id":  requestID,
			"snapshot_id": snapshotID,
			"agent_id":   body.AgentID,
		})

	case "delete":
		if req.Method != http.MethodDelete {
			writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
			return
		}
		msg := protocol.NewMessage(protocol.MsgBackupDelete)
		msg.Payload = protocol.BackupDeletePayload{
			RequestID:  requestID,
			SnapshotID: snapshotID,
		}
		if err := agent.SendMessage(msg); err != nil {
			writeErrorCustom(w, http.StatusBadGateway, protocol.CodeAgentWriteFailed, err.Error())
			return
		}
		writeJSON(w, map[string]string{
			"status":     "sent",
			"request_id":  requestID,
			"snapshot_id": snapshotID,
			"agent_id":   body.AgentID,
		})
	}
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
		writeErrorCustom(w, http.StatusInternalServerError, protocol.CodeInternalError, err.Error())
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
		writeErrorCustom(w, http.StatusBadRequest, protocol.CodeInternalError, err.Error())
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
		writeErrorCustom(w, http.StatusInternalServerError, protocol.CodeInternalError, err.Error())
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
			writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
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
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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
				writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
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
			writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		}
		return
	}

	// /api/v1/clients/{id}
	switch req.Method {
	case http.MethodGet:
		client, ok := r.registry.GetClient(clientID)
		if !ok {
			writeError(w, http.StatusNotFound, protocol.CodeClientNotFound)
			return
		}
		writeJSON(w, client)

	case http.MethodDelete:
		err := r.registry.DeactivateClient(clientID)
		if err != nil {
			writeError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeJSON(w, map[string]string{"status": "deactivated", "client_id": clientID})

	default:
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
	}
}

// handleAgentRegister — POST: зарегистрировать агента (альтернативный endpoint).
func (r *Relay) handleAgentRegister(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
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
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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

// handleApprovalsList — GET /api/v1/approvals?agent_id=X&status=pending&limit=50
func (r *Relay) handleApprovalsList(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	agentID := req.URL.Query().Get("agent_id")
	status := req.URL.Query().Get("status")
	limit := 50
	if l := req.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 {
			limit = n
		}
	}
	reqs := r.approvalQueue.List(agentID, ApprovalStatus(status), limit)
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"approvals":     reqs,
		"pending_count": r.approvalQueue.PendingCount(),
	})
}

// handleApprovalAction — POST /api/v1/approvals/{id}/approve or /reject
func (r *Relay) handleApprovalAction(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	path := strings.TrimPrefix(req.URL.Path, "/api/v1/approvals/")
	parts := strings.SplitN(path, "/", 2)
	if len(parts) < 2 {
		http.Error(w, "use /api/v1/approvals/{id}/approve or /reject", http.StatusBadRequest)
		return
	}
	id := parts[0]
	action := parts[1]

	var body struct {
		Comment string `json:"comment"`
	}
	json.NewDecoder(req.Body).Decode(&body)

	// Get actor from token or default
	actor := "dashboard"
	if auth := req.Header.Get("Authorization"); strings.HasPrefix(auth, "Bearer ") {
		actor = "token:" + auth[len("Bearer "):min(len(auth), 20)]
	}

	var req2 *ApprovalRequest
	var err error
	switch action {
	case "approve":
		req2, err = r.approvalQueue.Approve(id, actor, body.Comment)
	case "reject":
		req2, err = r.approvalQueue.Reject(id, actor, body.Comment)
	default:
		http.Error(w, "action must be 'approve' or 'reject'", http.StatusBadRequest)
		return
	}

	if err != nil {
		http.Error(w, err.Error(), http.StatusNotFound)
		return
	}

	// Отправляем MsgApprovalResponse агенту через WS
	if agent, ok := r.pool.Get(req2.AgentID); ok {
		respMsg := protocol.NewMessage(protocol.MsgApprovalResponse)
		respMsg.AgentID = req2.AgentID
		respMsg.Payload = map[string]any{
			"request_id": req2.ID,
			"decision":   action, // "approve" или "reject"
			"comment":    body.Comment,
		}
		if err := agent.SendMessage(respMsg); err != nil {
			r.logger.Error("approval response send failed", "err", err, "agent", req2.AgentID)
		}
	} else {
		r.logger.Warn("agent not connected for approval response", "agent", req2.AgentID)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(req2)
}

// handleBillingUsage — GET /api/v1/billing/usage?client_id=X
func (r *Relay) handleBillingUsage(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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
		writeError(w, http.StatusNotFound, protocol.CodeClientNotFound)
		return
	}
	writeJSON(w, plan)
}

// handleBillingPlanChange — POST /api/v1/billing/plan/change
func (r *Relay) handleBillingPlanChange(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}
	var body struct {
		ClientID string `json:"client_id"`
		PlanID   string `json:"plan_id"`
	}
	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}
	if body.ClientID == "" || body.PlanID == "" {
		writeError(w, http.StatusBadRequest, "client_id и plan_id обязательны")
		return
	}
	if _, ok := r.planStore.GetPlan(body.PlanID); !ok {
		writeError(w, http.StatusBadRequest, protocol.CodeClientNotFound)
		return
	}
	client, ok := r.registry.GetClient(body.ClientID)
	if !ok {
		writeError(w, http.StatusNotFound, protocol.CodeClientNotFound)
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
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
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

// === Auth HTTP Handlers (JWT Token Rotation) ===

// handleAuthToken — POST /api/v1/auth/token — генерация пары токенов.
// Body: {"client_id": "...", "client_secret": "..."} (опционально)
// Response: {"access_token": "...", "refresh_token": "...", "expires_at": 1234567890, "token_type": "Bearer"}
func (r *Relay) handleAuthToken(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	var body struct {
		ClientID     string `json:"client_id"`
		ClientSecret string `json:"client_secret"` // для будущей проверки
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.ClientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}

	// Проверяем что клиент существует (если есть registry)
	if r.registry != nil {
		if _, ok := r.registry.GetClient(body.ClientID); !ok {
			writeError(w, http.StatusNotFound, protocol.CodeClientNotFound)
			return
		}
	}

	// Генерируем пару токенов
	pair, err := r.auth.GenerateTokenPair(body.ClientID)
	if err != nil {
		r.logger.Error("token generation error", "err", err, "client_id", body.ClientID)
		writeError(w, http.StatusInternalServerError, protocol.CodeTokenGenerateError)
		return
	}

	writeJSON(w, pair)
}

// handleAuthRefresh — POST /api/v1/auth/refresh — refresh токенов.
// Body: {"refresh_token": "..."}
// Response: {"access_token": "...", "refresh_token": "...", "expires_at": 1234567890, "token_type": "Bearer"}
func (r *Relay) handleAuthRefresh(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	var body struct {
		RefreshToken string `json:"refresh_token"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.RefreshToken == "" {
		writeError(w, http.StatusBadRequest, "refresh_token обязателен")
		return
	}

	// Refresh токены
	pair, err := r.auth.RefreshToken(body.RefreshToken)
	if err != nil {
		r.logger.Warn("token refresh error", "err", err)
		writeError(w, http.StatusUnauthorized, err.Error())
		return
	}

	writeJSON(w, pair)
}

// handleAuthLogout — POST /api/v1/auth/logout — logout (blacklist токена).
// Требует Authorization header с access token.
// Response: {"status": "logged_out"}
func (r *Relay) handleAuthLogout(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	// Получаем токен из заголовка
	authHeader := req.Header.Get("Authorization")
	if authHeader == "" {
		writeError(w, http.StatusUnauthorized, "Authorization header обязателен")
		return
	}

	token := authHeader
	if strings.HasPrefix(token, "Bearer ") {
		token = strings.TrimPrefix(token, "Bearer ")
	}

	// Добавляем в blacklist
	if err := r.auth.Logout(token); err != nil {
		r.logger.Warn("logout error", "err", err)
		writeError(w, http.StatusBadRequest, protocol.CodeTokenInvalid)
		return
	}

	writeJSON(w, map[string]string{"status": "logged_out"})
}

// handleAuthRevoke — POST /api/v1/auth/revoke — admin revoke по client_id.
// Требует Authorization header с admin токеном (пока проверяем static token).
// Body: {"client_id": "..."}
// Response: {"status": "revoked", "count": 5}
func (r *Relay) handleAuthRevoke(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	// Проверяем admin доступ (static token из конфига)
	authHeader := req.Header.Get("Authorization")
	if authHeader == "" {
		writeError(w, http.StatusUnauthorized, "Authorization header обязателен")
		return
	}

	token := authHeader
	if strings.HasPrefix(token, "Bearer ") {
		token = strings.TrimPrefix(token, "Bearer ")
	}

	// Только static token может revoke
	if r.cfg.APIToken == "" || token != r.cfg.APIToken {
		writeError(w, http.StatusForbidden, protocol.CodeForbidden)
		return
	}

	var body struct {
		ClientID string `json:"client_id"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
		return
	}

	if body.ClientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}

	// Отзываем все токены клиента
	count := r.auth.RevokeByClientID(body.ClientID)

	writeJSON(w, map[string]any{
		"status":    "revoked",
		"client_id": body.ClientID,
		"count":     count,
	})
}

// === Rate Limit HTTP Handlers ===

// handleRateLimits — GET: список лимитов для всех клиентов, POST: сброс статистики.
func (r *Relay) handleRateLimits(w http.ResponseWriter, req *http.Request) {
	switch req.Method {
	case http.MethodGet:
		clientStats := r.rateLimit.GetAllClientStats()
		writeJSON(w, map[string]any{
			"clients": clientStats,
			"count":   len(clientStats),
		})

	case http.MethodPost:
		// POST /api/v1/rate-limits/reset — сбросить всю статистику
		r.rateLimit.ResetStats()
		writeJSON(w, map[string]string{"status": "stats_reset"})

	default:
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
	}
}

// handleRateLimitByClient — GET/PUT/DELETE для конкретного клиента.
func (r *Relay) handleRateLimitByClient(w http.ResponseWriter, req *http.Request) {
	// Извлекаем client_id из пути: /api/v1/rate-limits/{client_id}...
	path := strings.TrimPrefix(req.URL.Path, "/api/v1/rate-limits/")
	parts := strings.SplitN(path, "/", 2)
	clientID := parts[0]

	if clientID == "" {
		writeError(w, http.StatusBadRequest, "client_id обязателен")
		return
	}

	switch req.Method {
	case http.MethodGet:
		// GET — статистика для конкретного клиента
		stats := r.rateLimit.GetClientStats(clientID)
		writeJSON(w, stats)

	case http.MethodPut:
		// PUT — обновить лимиты для клиента
		var body struct {
			MaxPerMin  *int `json:"max_per_min"`
			MaxPerHour *int `json:"max_per_hour"`
		}
		if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
			writeError(w, http.StatusBadRequest, protocol.CodeInvalidJSON)
			return
		}

		if body.MaxPerMin == nil || body.MaxPerHour == nil {
			writeError(w, http.StatusBadRequest, "max_per_min и max_per_hour обязательны")
			return
		}

		r.rateLimit.SetClientLimits(clientID, *body.MaxPerMin, *body.MaxPerHour)
		stats := r.rateLimit.GetClientStats(clientID)
		writeJSON(w, map[string]any{
			"status":    "updated",
			"client_id": clientID,
			"limits":    stats,
		})

	case http.MethodDelete:
		// DELETE — сбросить кастомные лимиты (return to defaults)
		r.rateLimit.ResetClientLimits(clientID)
		writeJSON(w, map[string]string{
			"status":    "reset",
			"client_id": clientID,
		})

	case http.MethodPost:
		// POST /api/v1/rate-limits/{client_id}/reset — сбросить счётчики клиента
		if len(parts) == 2 && parts[1] == "reset" {
			r.rateLimit.ResetClientCounters(clientID)
			writeJSON(w, map[string]string{
				"status":    "counters_reset",
				"client_id": clientID,
			})
			return
		}
		writeError(w, http.StatusBadRequest, protocol.CodeInvalidPayload)

	default:
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
	}
}

// handleRateLimitStats — GET /api/v1/rate-limits/stats — общая статистика.
func (r *Relay) handleRateLimitStats(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, protocol.CodeUnknownError)
		return
	}

	stats := r.rateLimit.GetStats()
	writeJSON(w, stats)
}

func writeJSON(w http.ResponseWriter, data any) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(data)
}

func writeError(w http.ResponseWriter, httpCode int, protoCode string, args ...any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(httpCode)
	json.NewEncoder(w).Encode(map[string]string{
		"error": protocol.T(protoCode),
		"code":  protoCode,
	})
}

func writeErrorCustom(w http.ResponseWriter, httpCode int, protoCode string, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(httpCode)
	json.NewEncoder(w).Encode(map[string]string{
		"error": message,
		"code":  protoCode,
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

// handleNginxConfig обрабатывает GET /api/v1/nginx-config?domain=x&tls=true
func (r *Relay) handleNginxConfig(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}

	// Проверяем авторизацию (API token)
	token := req.Header.Get("Authorization")
	if token == "" {
		token = req.URL.Query().Get("token")
	}
	if token == "" {
		writeError(w, http.StatusUnauthorized, "authorization required")
		return
	}
	if r.cfg.APIToken != "" && token != "Bearer "+r.cfg.APIToken && token != r.cfg.APIToken {
		writeError(w, http.StatusUnauthorized, "invalid token")
		return
	}

	// Парсим параметры запроса
	domain := req.URL.Query().Get("domain")
	if domain == "" {
		writeError(w, http.StatusBadRequest, "domain parameter required")
		return
	}

	tlsParam := req.URL.Query().Get("tls")
	tls := tlsParam == "true" || tlsParam == "1"

	wsPath := req.URL.Query().Get("ws_path")
	if wsPath == "" {
		wsPath = "/ws"
	}

	apiPrefix := req.URL.Query().Get("api_prefix")
	if apiPrefix == "" {
		apiPrefix = "/api/v1"
	}

	certPath := req.URL.Query().Get("cert_path")
	keyPath := req.URL.Query().Get("key_path")

	rateLimitStr := req.URL.Query().Get("rate_limit")
	rateLimit := 100
	if rateLimitStr != "" {
		var err error
		rateLimit, err = strconv.Atoi(rateLimitStr)
		if err != nil {
			writeError(w, http.StatusBadRequest, "invalid rate_limit parameter")
			return
		}
	}

	fullConfig := req.URL.Query().Get("full") == "true"

	// Создаём конфигурацию
	config := nginx.Config{
		Domain:         domain,
		WSSPath:        wsPath,
		APIPrefix:      apiPrefix,
		TLS:            tls,
		CertPath:       certPath,
		KeyPath:        keyPath,
		BackendAPIPort: 8080,
		BackendWSSPort: 8443,
		RateLimit:      rateLimit,
		EnableGzip:     true,
	}

	if tls {
		config.Port = 443
		if certPath == "" {
			config.CertPath = fmt.Sprintf("/etc/letsencrypt/live/%s/fullchain.pem", domain)
		}
		if keyPath == "" {
			config.KeyPath = fmt.Sprintf("/etc/letsencrypt/live/%s/privkey.pem", domain)
		}
	} else {
		config.Port = 80
	}

	// Валидация
	if err := config.Validate(); err != nil {
		writeError(w, http.StatusBadRequest, fmt.Sprintf("invalid config: %v", err))
		return
	}

	// Генерируем конфиг
	gen := nginx.NewGenerator(config)
	var output string
	var err error

	if fullConfig {
		output, err = gen.GenerateFullConfig()
	} else {
		output, err = gen.Generate()
	}

	if err != nil {
		writeError(w, http.StatusInternalServerError, fmt.Sprintf("failed to generate config: %v", err))
		return
	}

	// Возвращаем конфиг
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	w.Write([]byte(output))
}

// --- Health Check Adapters ---

// poolForHealth адаптирует AgentPool для health checker.
type poolForHealth struct {
	pool *AgentPool
}

func (p poolForHealth) Count() int {
	return p.pool.Count()
}

// authForHealth адаптирует AuthManager для health checker.
type authForHealth struct {
	auth *AuthManager
}

func (a authForHealth) TokenCount() (int, int, int) {
	total, blacklisted := 0, 0
	if a.auth != nil {
		total = a.auth.TokenCount()
		blacklisted = a.auth.BlacklistCount()
	}
	return total, total - blacklisted, blacklisted
}

// auditForHealth адаптирует AuditLogger для health checker.
type auditForHealth struct {
	audit *AuditLogger
}

func (a auditForHealth) IsWritable() bool {
	if a.audit != nil {
		return a.audit.IsWritable()
	}
	return false
}

// registryForHealth адаптирует Registry для health checker.
type registryForHealth struct {
	registry *Registry
}

func (r registryForHealth) IsReadable() bool {
	return true // registry всегда читаем (in-memory)
}

func (r registryForHealth) IsWritable() bool {
	return true // registry с авто-сохранением
}

// --- Health Check Handlers ---

// handleHealth — GET /api/v1/health — полный отчёт о здоровье.
func (r *Relay) handleHealth(w http.ResponseWriter, req *http.Request) {
	report := r.healthChecker.Check()
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(report)
}

// handleHealthReady — GET /api/v1/health/ready — 200 если готов, 503 если нет.
func (r *Relay) handleHealthReady(w http.ResponseWriter, req *http.Request) {
	if r.healthChecker.IsReady() {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	} else {
		w.WriteHeader(http.StatusServiceUnavailable)
		w.Write([]byte("NOT READY"))
	}
}

// handleHealthLive — GET /api/v1/health/live — 200 если процесс жив.
func (r *Relay) handleHealthLive(w http.ResponseWriter, req *http.Request) {
	if r.healthChecker.IsLive() {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	} else {
		w.WriteHeader(http.StatusServiceUnavailable)
		w.Write([]byte("NOT LIVE"))
	}
}

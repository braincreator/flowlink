// Package health — health checks для FlowLink relay.
package health

import (
	"fmt"
	"net/http"
	"os"
	"sync"
	"time"
)

// Status — статус компонента.
type Status string

const (
	StatusHealthy  Status = "healthy"  // Всё ок
	StatusDegraded Status = "degraded" // Есть проблемы, но работает
	StatusDown     Status = "down"     // Критическая проблема
)

// ComponentHealth — здоровье одного компонента.
type ComponentHealth struct {
	Name      string `json:"name"`
	Status    Status `json:"status"`
	Message   string `json:"message"`
	LatencyMs int64  `json:"latency_ms"`
	Timestamp int64  `json:"timestamp"`
}

// HealthReport — полный отчёт о здоровье системы.
type HealthReport struct {
	Status      Status                     `json:"status"`
	Timestamp   int64                      `json:"timestamp"`
	Components  map[string]ComponentHealth `json:"components"`
	UptimeSec   int64                      `json:"uptime_sec"`
	Version     string                     `json:"version"`
}

// HealthChecker — проверяет здоровье компонентов relay.
type HealthChecker struct {
	mu          sync.RWMutex
	startTime   time.Time
	version     string
	
	// Components для проверки
	wssAddr     string
	apiAddr     string
	agentPool   AgentPoolChecker
	authManager AuthManagerChecker
	auditLogger AuditLoggerChecker
	registry    RegistryChecker
}

// AgentPoolChecker — интерфейс для проверки agent pool.
type AgentPoolChecker interface {
	Count() int
}

// AuthManagerChecker — интерфейс для проверки auth manager.
type AuthManagerChecker interface {
	TokenCount() (total int, active int, blacklisted int)
}

// AuditLoggerChecker — интерфейс для проверки audit logger.
type AuditLoggerChecker interface {
	IsWritable() bool
}

// RegistryChecker — интерфейс для проверки registry.
type RegistryChecker interface {
	IsReadable() bool
	IsWritable() bool
}

// NewHealthChecker — создаёт новый health checker.
func NewHealthChecker(version string) *HealthChecker {
	return &HealthChecker{
		startTime: time.Now(),
		version:   version,
	}
}

// SetWSSAddr — устанавливает адрес WSS сервера.
func (h *HealthChecker) SetWSSAddr(addr string) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.wssAddr = addr
}

// SetAPIAddr — устанавливает адрес API сервера.
func (h *HealthChecker) SetAPIAddr(addr string) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.apiAddr = addr
}

// SetAgentPool — устанавливает agent pool checker.
func (h *HealthChecker) SetAgentPool(pool AgentPoolChecker) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.agentPool = pool
}

// SetAuthManager — устанавливает auth manager checker.
func (h *HealthChecker) SetAuthManager(auth AuthManagerChecker) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.authManager = auth
}

// SetAuditLogger — устанавливает audit logger checker.
func (h *HealthChecker) SetAuditLogger(audit AuditLoggerChecker) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.auditLogger = audit
}

// SetRegistry — устанавливает registry checker.
func (h *HealthChecker) SetRegistry(registry RegistryChecker) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.registry = registry
}

// Check — выполняет все проверки и возвращает отчёт.
func (h *HealthChecker) Check() *HealthReport {
	h.mu.RLock()
	defer h.mu.RUnlock()

	now := time.Now()
	components := make(map[string]ComponentHealth)

	// 1. WSS Listener
	components["wss_listener"] = h.checkWSSListener(now)

	// 2. API Listener
	components["api_listener"] = h.checkAPIListener(now)

	// 3. Agent Pool
	components["agent_pool"] = h.checkAgentPool(now)

	// 4. Auth Manager
	components["auth_manager"] = h.checkAuthManager(now)

	// 5. Audit Logger
	components["audit_logger"] = h.checkAuditLogger(now)

	// 6. Registry (File System)
	components["registry"] = h.checkRegistry(now)

	// Определяем aggregate status
	aggregateStatus := h.calculateAggregateStatus(components)

	return &HealthReport{
		Status:     aggregateStatus,
		Timestamp:  now.Unix(),
		Components: components,
		UptimeSec:  int64(now.Sub(h.startTime).Seconds()),
		Version:    h.version,
	}
}

// checkWSSListener — проверяет WSS listener.
func (h *HealthChecker) checkWSSListener(now time.Time) ComponentHealth {
	start := now
	name := "wss_listener"

	if h.wssAddr == "" {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   "WSS address not configured",
			LatencyMs: 0,
			Timestamp: now.Unix(),
		}
	}

	// Проверяем что порт слушается (TCP dial)
	client := http.Client{
		Timeout: 2 * time.Second,
	}
	
	// Пытаемся подключиться к WSS endpoint
	url := fmt.Sprintf("http://%s/health", h.wssAddr)
	resp, err := client.Get(url)
	
	latency := time.Since(start).Milliseconds()

	if err != nil {
		// Попробуем просто TCP dial
		// Для WSS это норм, если HTTP не отвечает
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   fmt.Sprintf("WSS health check failed: %v", err),
			LatencyMs: latency,
			Timestamp: now.Unix(),
		}
	}
	resp.Body.Close()

	return ComponentHealth{
		Name:      name,
		Status:    StatusHealthy,
		Message:   "WSS listener is alive",
		LatencyMs: latency,
		Timestamp: now.Unix(),
	}
}

// checkAPIListener — проверяет API listener.
func (h *HealthChecker) checkAPIListener(now time.Time) ComponentHealth {
	start := now
	name := "api_listener"

	if h.apiAddr == "" {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   "API address not configured",
			LatencyMs: 0,
			Timestamp: now.Unix(),
		}
	}

	// Проверяем /api/v1/health/live endpoint
	client := http.Client{
		Timeout: 2 * time.Second,
	}

	url := fmt.Sprintf("http://%s/api/v1/health/live", h.apiAddr)
	resp, err := client.Get(url)
	
	latency := time.Since(start).Milliseconds()

	if err != nil {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDown,
			Message:   fmt.Sprintf("API listener unreachable: %v", err),
			LatencyMs: latency,
			Timestamp: now.Unix(),
		}
	}
	resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   fmt.Sprintf("API returned status %d", resp.StatusCode),
			LatencyMs: latency,
			Timestamp: now.Unix(),
		}
	}

	return ComponentHealth{
		Name:      name,
		Status:    StatusHealthy,
		Message:   "API listener is alive",
		LatencyMs: latency,
		Timestamp: now.Unix(),
	}
}

// checkAgentPool — проверяет agent pool.
func (h *HealthChecker) checkAgentPool(now time.Time) ComponentHealth {
	start := now
	name := "agent_pool"

	if h.agentPool == nil {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   "Agent pool not initialized",
			LatencyMs: 0,
			Timestamp: now.Unix(),
		}
	}

	count := h.agentPool.Count()
	latency := time.Since(start).Milliseconds()

	var status Status
	var message string

	if count == 0 {
		status = StatusDegraded
		message = "No agents connected"
	} else {
		status = StatusHealthy
		message = fmt.Sprintf("%d agent(s) connected", count)
	}

	return ComponentHealth{
		Name:      name,
		Status:    status,
		Message:   message,
		LatencyMs: latency,
		Timestamp: now.Unix(),
	}
}

// checkAuthManager — проверяет auth manager.
func (h *HealthChecker) checkAuthManager(now time.Time) ComponentHealth {
	start := now
	name := "auth_manager"

	if h.authManager == nil {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   "Auth manager not initialized",
			LatencyMs: 0,
			Timestamp: now.Unix(),
		}
	}

	total, active, blacklisted := h.authManager.TokenCount()
	latency := time.Since(start).Milliseconds()

	var status Status
	message := fmt.Sprintf("Tokens: %d total, %d active, %d blacklisted", total, active, blacklisted)

	// Критично если все токены в blacklist
	if total > 0 && active == 0 {
		status = StatusDown
		message += " (CRITICAL: all tokens blacklisted)"
	} else if blacklisted > active {
		status = StatusDegraded
		message += " (WARNING: more blacklisted than active)"
	} else {
		status = StatusHealthy
	}

	return ComponentHealth{
		Name:      name,
		Status:    status,
		Message:   message,
		LatencyMs: latency,
		Timestamp: now.Unix(),
	}
}

// checkAuditLogger — проверяет audit logger.
func (h *HealthChecker) checkAuditLogger(now time.Time) ComponentHealth {
	start := now
	name := "audit_logger"

	if h.auditLogger == nil {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   "Audit logger not initialized",
			LatencyMs: 0,
			Timestamp: now.Unix(),
		}
	}

	writable := h.auditLogger.IsWritable()
	latency := time.Since(start).Milliseconds()

	var status Status
	var message string

	if writable {
		status = StatusHealthy
		message = "Audit log is writable"
	} else {
		status = StatusDown
		message = "Audit log is NOT writable (CRITICAL)"
	}

	return ComponentHealth{
		Name:      name,
		Status:    status,
		Message:   message,
		LatencyMs: latency,
		Timestamp: now.Unix(),
	}
}

// checkRegistry — проверяет registry (file system).
func (h *HealthChecker) checkRegistry(now time.Time) ComponentHealth {
	start := now
	name := "registry"

	if h.registry == nil {
		return ComponentHealth{
			Name:      name,
			Status:    StatusDegraded,
			Message:   "Registry not initialized",
			LatencyMs: 0,
			Timestamp: now.Unix(),
		}
	}

	readable := h.registry.IsReadable()
	writable := h.registry.IsWritable()
	latency := time.Since(start).Milliseconds()

	var status Status
	var message string

	if readable && writable {
		status = StatusHealthy
		message = "Registry is readable and writable"
	} else if readable {
		status = StatusDegraded
		message = "Registry is read-only"
	} else {
		status = StatusDown
		message = "Registry is NOT readable (CRITICAL)"
	}

	return ComponentHealth{
		Name:      name,
		Status:    status,
		Message:   message,
		LatencyMs: latency,
		Timestamp: now.Unix(),
	}
}

// calculateAggregateStatus — вычисляет общий статус системы.
func (h *HealthChecker) calculateAggregateStatus(components map[string]ComponentHealth) Status {
	hasDown := false
	hasDegraded := false

	for _, comp := range components {
		switch comp.Status {
		case StatusDown:
			hasDown = true
		case StatusDegraded:
			hasDegraded = true
		}
	}

	// Критичные компоненты: API listener, Audit logger, Registry
	criticalComponents := []string{"api_listener", "audit_logger", "registry"}
	for _, name := range criticalComponents {
		if comp, ok := components[name]; ok && comp.Status == StatusDown {
			return StatusDown
		}
	}

	if hasDown {
		return StatusDown
	}
	if hasDegraded {
		return StatusDegraded
	}
	return StatusHealthy
}

// IsReady — проверяет готовность системы (для load balancers).
func (h *HealthChecker) IsReady() bool {
	report := h.Check()
	// Ready если не down (degraded допустим)
	return report.Status != StatusDown
}

// IsLive — проверяет что процесс жив (для orchestrators).
func (h *HealthChecker) IsLive() bool {
	// Процесс жив если health checker существует
	return h != nil
}

// === Mock implementations for testing ===

// MockAgentPool — mock для тестов.
type MockAgentPool struct {
	CountFunc func() int
}

func (m *MockAgentPool) Count() int {
	if m.CountFunc != nil {
		return m.CountFunc()
	}
	return 0
}

// MockAuthManager — mock для тестов.
type MockAuthManager struct {
	TokenCountFunc func() (int, int, int)
}

func (m *MockAuthManager) TokenCount() (int, int, int) {
	if m.TokenCountFunc != nil {
		return m.TokenCountFunc()
	}
	return 0, 0, 0
}

// MockAuditLogger — mock для тестов.
type MockAuditLogger struct {
	IsWritableFunc func() bool
}

func (m *MockAuditLogger) IsWritable() bool {
	if m.IsWritableFunc != nil {
		return m.IsWritableFunc()
	}
	return false
}

// MockRegistry — mock для тестов.
type MockRegistry struct {
	IsReadableFunc func() bool
	IsWritableFunc func() bool
}

func (m *MockRegistry) IsReadable() bool {
	if m.IsReadableFunc != nil {
		return m.IsReadableFunc()
	}
	return false
}

func (m *MockRegistry) IsWritable() bool {
	if m.IsWritableFunc != nil {
		return m.IsWritableFunc()
	}
	return false
}

// FileCheck — простая проверка файловой системы.
func FileCheck(path string) (readable, writable bool) {
	// Check readable
	_, err := os.Stat(path)
	if err != nil {
		return false, false
	}

	// Check writable (try to create temp file)
	tmpFile := path + "/.health_check_tmp"
	f, err := os.Create(tmpFile)
	if err != nil {
		return true, false
	}
	f.Close()
	os.Remove(tmpFile)

	return true, true
}

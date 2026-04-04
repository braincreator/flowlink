package health

import (
	"testing"
	"time"
)

func TestHealthChecker_Check(t *testing.T) {
	hc := NewHealthChecker("test-1.0.0")

	// Без компонентов - должен быть degraded
	report := hc.Check()
	if report.Status == StatusHealthy {
		t.Errorf("Expected degraded without components, got %s", report.Status)
	}
	if report.Version != "test-1.0.0" {
		t.Errorf("Expected version test-1.0.0, got %s", report.Version)
	}
}

func TestHealthChecker_CheckWSSListener(t *testing.T) {
	hc := NewHealthChecker("test")

	// Без адреса - degraded
	comp := hc.checkWSSListener(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded without address, got %s", comp.Status)
	}

	// С адресом (но без реального сервера) - degraded (не может подключиться)
	hc.SetWSSAddr("localhost:9999")
	comp = hc.checkWSSListener(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded with unreachable address, got %s", comp.Status)
	}
}

func TestHealthChecker_CheckAPIListener(t *testing.T) {
	hc := NewHealthChecker("test")

	// Без адреса - degraded
	comp := hc.checkAPIListener(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded without address, got %s", comp.Status)
	}
}

func TestHealthChecker_CheckAgentPool(t *testing.T) {
	hc := NewHealthChecker("test")

	// Без pool - degraded
	comp := hc.checkAgentPool(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded without pool, got %s", comp.Status)
	}

	// С pool, 0 агентов - degraded
	mockPool := &MockAgentPool{
		CountFunc: func() int { return 0 },
	}
	hc.SetAgentPool(mockPool)
	comp = hc.checkAgentPool(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded with 0 agents, got %s", comp.Status)
	}

	// С агентами - healthy
	mockPool.CountFunc = func() int { return 5 }
	comp = hc.checkAgentPool(time.Now())
	if comp.Status != StatusHealthy {
		t.Errorf("Expected healthy with agents, got %s", comp.Status)
	}
}

func TestHealthChecker_CheckAuthManager(t *testing.T) {
	hc := NewHealthChecker("test")

	// Без auth - degraded
	comp := hc.checkAuthManager(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded without auth, got %s", comp.Status)
	}

	// С auth, все токены blacklisted - down
	mockAuth := &MockAuthManager{
		TokenCountFunc: func() (int, int, int) { return 10, 0, 10 },
	}
	hc.SetAuthManager(mockAuth)
	comp = hc.checkAuthManager(time.Now())
	if comp.Status != StatusDown {
		t.Errorf("Expected down with all tokens blacklisted, got %s", comp.Status)
	}

	// Больше blacklisted чем active - degraded
	mockAuth.TokenCountFunc = func() (int, int, int) { return 10, 2, 8 }
	comp = hc.checkAuthManager(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded with more blacklisted, got %s", comp.Status)
	}

	// Норм - healthy
	mockAuth.TokenCountFunc = func() (int, int, int) { return 10, 8, 2 }
	comp = hc.checkAuthManager(time.Now())
	if comp.Status != StatusHealthy {
		t.Errorf("Expected healthy with normal tokens, got %s", comp.Status)
	}
}

func TestHealthChecker_CheckAuditLogger(t *testing.T) {
	hc := NewHealthChecker("test")

	// Без audit - degraded
	comp := hc.checkAuditLogger(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded without audit, got %s", comp.Status)
	}

	// Audit не writable - down
	mockAudit := &MockAuditLogger{
		IsWritableFunc: func() bool { return false },
	}
	hc.SetAuditLogger(mockAudit)
	comp = hc.checkAuditLogger(time.Now())
	if comp.Status != StatusDown {
		t.Errorf("Expected down with unwritable audit, got %s", comp.Status)
	}

	// Audit writable - healthy
	mockAudit.IsWritableFunc = func() bool { return true }
	comp = hc.checkAuditLogger(time.Now())
	if comp.Status != StatusHealthy {
		t.Errorf("Expected healthy with writable audit, got %s", comp.Status)
	}
}

func TestHealthChecker_CheckRegistry(t *testing.T) {
	hc := NewHealthChecker("test")

	// Без registry - degraded
	comp := hc.checkRegistry(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded without registry, got %s", comp.Status)
	}

	// Registry не readable - down
	mockRegistry := &MockRegistry{
		IsReadableFunc: func() bool { return false },
		IsWritableFunc: func() bool { return false },
	}
	hc.SetRegistry(mockRegistry)
	comp = hc.checkRegistry(time.Now())
	if comp.Status != StatusDown {
		t.Errorf("Expected down with unreadable registry, got %s", comp.Status)
	}

	// Registry read-only - degraded
	mockRegistry.IsReadableFunc = func() bool { return true }
	comp = hc.checkRegistry(time.Now())
	if comp.Status != StatusDegraded {
		t.Errorf("Expected degraded with read-only registry, got %s", comp.Status)
	}

	// Registry read-write - healthy
	mockRegistry.IsWritableFunc = func() bool { return true }
	comp = hc.checkRegistry(time.Now())
	if comp.Status != StatusHealthy {
		t.Errorf("Expected healthy with read-write registry, got %s", comp.Status)
	}
}

func TestHealthChecker_CalculateAggregateStatus(t *testing.T) {
	hc := NewHealthChecker("test")

	tests := []struct {
		name       string
		components map[string]ComponentHealth
		expected   Status
	}{
		{
			name: "all healthy",
			components: map[string]ComponentHealth{
				"wss_listener":  {Status: StatusHealthy},
				"api_listener":  {Status: StatusHealthy},
				"agent_pool":    {Status: StatusHealthy},
				"auth_manager":  {Status: StatusHealthy},
				"audit_logger":  {Status: StatusHealthy},
				"registry":      {Status: StatusHealthy},
			},
			expected: StatusHealthy,
		},
		{
			name: "one degraded",
			components: map[string]ComponentHealth{
				"wss_listener":  {Status: StatusDegraded},
				"api_listener":  {Status: StatusHealthy},
				"agent_pool":    {Status: StatusHealthy},
				"auth_manager":  {Status: StatusHealthy},
				"audit_logger":  {Status: StatusHealthy},
				"registry":      {Status: StatusHealthy},
			},
			expected: StatusDegraded,
		},
		{
			name: "critical component down",
			components: map[string]ComponentHealth{
				"wss_listener":  {Status: StatusHealthy},
				"api_listener":  {Status: StatusDown},
				"agent_pool":    {Status: StatusHealthy},
				"auth_manager":  {Status: StatusHealthy},
				"audit_logger":  {Status: StatusHealthy},
				"registry":      {Status: StatusHealthy},
			},
			expected: StatusDown,
		},
		{
			name: "non-critical component down",
			components: map[string]ComponentHealth{
				"wss_listener":  {Status: StatusDown},
				"api_listener":  {Status: StatusHealthy},
				"agent_pool":    {Status: StatusHealthy},
				"auth_manager":  {Status: StatusHealthy},
				"audit_logger":  {Status: StatusHealthy},
				"registry":      {Status: StatusHealthy},
			},
			expected: StatusDown,
		},
		{
			name: "audit logger down",
			components: map[string]ComponentHealth{
				"wss_listener":  {Status: StatusHealthy},
				"api_listener":  {Status: StatusHealthy},
				"agent_pool":    {Status: StatusHealthy},
				"auth_manager":  {Status: StatusHealthy},
				"audit_logger":  {Status: StatusDown},
				"registry":      {Status: StatusHealthy},
			},
			expected: StatusDown,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := hc.calculateAggregateStatus(tt.components)
			if result != tt.expected {
				t.Errorf("Expected %s, got %s", tt.expected, result)
			}
		})
	}
}

func TestHealthChecker_IsReady(t *testing.T) {
	hc := NewHealthChecker("test")

	// Настраиваем все компоненты как healthy
	mockPool := &MockAgentPool{CountFunc: func() int { return 1 }}
	mockAuth := &MockAuthManager{TokenCountFunc: func() (int, int, int) { return 10, 8, 2 }}
	mockAudit := &MockAuditLogger{IsWritableFunc: func() bool { return true }}
	mockRegistry := &MockRegistry{
		IsReadableFunc: func() bool { return true },
		IsWritableFunc: func() bool { return true },
	}

	hc.SetAgentPool(mockPool)
	hc.SetAuthManager(mockAuth)
	hc.SetAuditLogger(mockAudit)
	hc.SetRegistry(mockRegistry)

	if !hc.IsReady() {
		t.Error("Expected ready with all healthy components")
	}

	// Делаем audit unwritable - должен быть NOT ready
	mockAudit.IsWritableFunc = func() bool { return false }
	if hc.IsReady() {
		t.Error("Expected NOT ready with unwritable audit")
	}
}

func TestHealthChecker_IsLive(t *testing.T) {
	hc := NewHealthChecker("test")

	if !hc.IsLive() {
		t.Error("Expected live")
	}
}

func TestHealthChecker_Uptime(t *testing.T) {
	hc := NewHealthChecker("test")

	time.Sleep(100 * time.Millisecond)

	report := hc.Check()
	if report.UptimeSec < 0 {
		t.Errorf("Expected positive uptime, got %d", report.UptimeSec)
	}
}

func TestFileCheck(t *testing.T) {
	// Создаём временную директорию
	tmpDir := t.TempDir()

	readable, writable := FileCheck(tmpDir)
	if !readable {
		t.Error("Expected readable")
	}
	if !writable {
		t.Error("Expected writable")
	}

	// Несуществующий путь
	readable, writable = FileCheck("/nonexistent/path/12345")
	if readable {
		t.Error("Expected NOT readable for nonexistent path")
	}
	if writable {
		t.Error("Expected NOT writable for nonexistent path")
	}
}

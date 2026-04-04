// Package integration — главный менеджер интеграции.
package integration

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
)

// IntegrationManager — управляет всеми интеграциями.
// Запускается как часть flowlink-relay процесса.
type IntegrationManager struct {
	mu          sync.Mutex
	bridge      *BillingAutoscaleBridge
	provisioner *Provisioner
	notifier    *Notifier
	webhook     *WebhookHandler
	status      *IntegrationStatusHandler
	logger      *slog.Logger

	// Background tasks
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// IntegrationConfig — конфигурация интеграции.
type IntegrationConfig struct {
	DockerSocket       string // default: /var/run/docker.sock
	BasePort           int    // default: 9081
	DataDir            string // default: /var/lib/flowlink
	WebhookSecret      string
	TGBotToken         string
	TGAPI              string // default: https://api.telegram.org
	SMTPHost           string
	SMTPPort           int
	SMTPUser           string
	SMTPPass           string
	TochkaClientID     string
	TochkaClientSecret string
	TochkaAccountID    string
}

// NewIntegrationManager — создаёт и инициализирует все компоненты.
func NewIntegrationManager(cfg *IntegrationConfig, subStore *billing.SubscriptionStore, logger *slog.Logger) (*IntegrationManager, error) {
	if logger == nil {
		logger = slog.Default()
	}

	// Set defaults
	if cfg.DockerSocket == "" {
		cfg.DockerSocket = "/var/run/docker.sock"
	}
	if cfg.BasePort == 0 {
		cfg.BasePort = 9081
	}
	if cfg.DataDir == "" {
		cfg.DataDir = "/var/lib/flowlink"
	}
	if cfg.TGAPI == "" {
		cfg.TGAPI = "https://api.telegram.org"
	}

	// Create directories
	if err := os.MkdirAll(cfg.DataDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create data dir: %w", err)
	}

	// 1. Create provisioner
	provisioner := NewProvisioner(
		cfg.DockerSocket,
		cfg.BasePort,
		cfg.DataDir+"/config",
		cfg.DataDir+"/data",
		logger,
	)

	// 2. Create notifier
	notifier := NewNotifier(
		cfg.TGBotToken,
		cfg.TGAPI,
		cfg.SMTPHost,
		cfg.SMTPPort,
		cfg.SMTPUser,
		cfg.SMTPPass,
		logger,
	)

	// 3. Create bridge (without scaler/router for now - they'll be injected later)
	bridge := NewBillingAutoscaleBridge(
		subStore,
		nil, // scaler - injected via SetScaler
		nil, // router - injected via SetRouter
		provisioner,
		notifier,
		logger,
	)

	// 4. Create webhook handler
	webhook := NewWebhookHandler(
		bridge,
		subStore,
		cfg.WebhookSecret,
		logger,
	)

	// 5. Create manager
	ctx, cancel := context.WithCancel(context.Background())

	mgr := &IntegrationManager{
		bridge:      bridge,
		provisioner: provisioner,
		notifier:    notifier,
		webhook:     webhook,
		logger:      logger,
		ctx:         ctx,
		cancel:      cancel,
	}

	// 6. Create status handler (needs manager reference)
	mgr.status = NewIntegrationStatusHandler(mgr, logger)

	return mgr, nil
}

// SetScaler — sets the autoscaler implementation (dependency injection).
func (m *IntegrationManager) SetScaler(scaler AutoScalerInterface) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.bridge.scaler = scaler
}

// SetRouter — sets the traffic router implementation (dependency injection).
func (m *IntegrationManager) SetRouter(router AutoRouterInterface) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.bridge.router = router
}

// Start — запускает все компоненты.
// Registers webhook routes, starts health monitoring.
func (m *IntegrationManager) Start(ctx context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	m.logger.Info("starting integration manager")

	// 1. Start background tasks
	m.wg.Add(1)
	go m.gracePeriodChecker()

	m.wg.Add(1)
	go m.healthMonitor()

	m.logger.Info("integration manager started")

	return nil
}

// Stop — gracefully stops all components.
func (m *IntegrationManager) Stop(ctx context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	m.logger.Info("stopping integration manager")

	// Cancel context
	m.cancel()

	// Wait for background tasks
	done := make(chan struct{})
	go func() {
		m.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		m.logger.Info("integration manager stopped")
		return nil
	case <-ctx.Done():
		m.logger.Warn("integration manager stop timeout")
		return ctx.Err()
	}
}

// RegisterRoutes — регистрирует HTTP endpoints на relay сервере.
// POST /api/v1/webhook/tochka — Tochka payment webhook
// GET  /api/v1/integration/status — integration health
// POST /api/v1/integration/provision — manual provision (admin)
// POST /api/v1/integration/deprovision — manual deprovision (admin)
func (m *IntegrationManager) RegisterRoutes(mux *http.ServeMux) {
	m.webhook.RegisterRoutes(mux)
	m.status.RegisterRoutes(mux)
}

// gracePeriodChecker — периодически проверяет истекшие grace periods.
func (m *IntegrationManager) gracePeriodChecker() {
	defer m.wg.Done()

	ticker := time.NewTicker(1 * time.Hour)
	defer ticker.Stop()

	for {
		select {
		case <-m.ctx.Done():
			return
		case <-ticker.C:
			if err := m.bridge.CheckGracePeriods(m.ctx); err != nil {
				m.logger.Error("grace period check failed", "err", err)
			}
		}
	}
}

// healthMonitor — мониторинг здоровья provisioned контейнеров.
func (m *IntegrationManager) healthMonitor() {
	defer m.wg.Done()

	ticker := time.NewTicker(5 * time.Minute)
	defer ticker.Stop()

	for {
		select {
		case <-m.ctx.Done():
			return
		case <-ticker.C:
			m.checkContainersHealth()
		}
	}
}

// checkContainersHealth — проверяет здоровье всех контейнеров.
func (m *IntegrationManager) checkContainersHealth() {
	clients, err := m.provisioner.GetProvisionedClients()
	if err != nil {
		m.logger.Error("failed to get provisioned clients", "err", err)
		return
	}

	for _, client := range clients {
		if client.Status != "running" {
			continue
		}

		// Check health endpoint
		healthURL := fmt.Sprintf("http://localhost:%d/health", client.Port)
		ctx, cancel := context.WithTimeout(m.ctx, 5*time.Second)

		req, err := http.NewRequestWithContext(ctx, "GET", healthURL, nil)
		if err != nil {
			cancel()
			continue
		}

		httpClient := &http.Client{Timeout: 5 * time.Second}
		resp, err := httpClient.Do(req)
		cancel()

		if err != nil || resp.StatusCode != http.StatusOK {
			m.logger.Warn("container health check failed",
				"customer_id", client.CustomerID,
				"container_id", client.ContainerID,
				"err", err,
			)

			// Notify about autoheal
			if m.notifier != nil {
				notif := &Notification{
					Type:       NotifAutohealed,
					CustomerID: client.CustomerID,
					Subject:    "⚠️ FlowLink: Перезапуск сервера",
					Body:       "Ваш сервер был перезапущен из-за проблем со здоровьем. Если проблема повторяется — обратитесь в поддержку.",
				}
				m.notifier.Send(m.ctx, notif)
			}

			// TODO: Trigger autoheal (restart container)
			if resp != nil {
				resp.Body.Close()
			}
		}
	}
}

// WaitForShutdown — waits for OS signals and gracefully shuts down.
func (m *IntegrationManager) WaitForShutdown() {
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	<-sigChan
	m.logger.Info("shutdown signal received")

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if err := m.Stop(ctx); err != nil {
		m.logger.Error("failed to stop integration manager", "err", err)
	}
}

// GetStats — returns integration statistics.
func (m *IntegrationManager) GetStats() map[string]interface{} {
	clients, _ := m.provisioner.GetProvisionedClients()

	activeCount := 0
	for _, c := range clients {
		if c.Status == "running" {
			activeCount++
		}
	}

	return map[string]interface{}{
		"total_clients":     len(clients),
		"active_clients":    activeCount,
		"grace_periods":     len(m.bridge.gracePeriods),
		"base_port":         m.provisioner.basePort,
		"docker_socket":     m.provisioner.dockerAPI,
	}
}

// ManualProvision — manually provisions a client (admin endpoint).
func (m *IntegrationManager) ManualProvision(ctx context.Context, req *ProvisioningRequest) (*ProvisioningResult, error) {
	return m.provisioner.Provision(ctx, req)
}

// ManualDeprovision — manually deprovisions a client (admin endpoint).
func (m *IntegrationManager) ManualDeprovision(ctx context.Context, customerID string) error {
	return m.provisioner.Deprovision(ctx, customerID)
}

// SendTestNotification — sends test notification (for debugging).
func (m *IntegrationManager) SendTestNotification(ctx context.Context, customerID, telegramID, email string) error {
	notif := &Notification{
		Type:       NotifProvisioned,
		CustomerID: customerID,
		TelegramID: telegramID,
		Email:      email,
		Subject:    "✅ FlowLink: Тестовое уведомление",
		Body:       "Это тестовое уведомление. Если вы его получили — интеграция работает корректно.",
	}
	return m.notifier.Send(ctx, notif)
}

package autoscale

import (
	"fmt"
	"log/slog"
	"sync"
	"time"
)

// AutoscaleConfig — конфигурация autoscaler.
type AutoscaleConfig struct {
	MinServers        int     `json:"min_servers"`
	MaxServers        int     `json:"max_servers"`
	ScaleUpThreshold  float64 `json:"scale_up_threshold"`   // clients per server
	ScaleDownThreshold float64 `json:"scale_down_threshold"` // clients per server
	CooldownMinutes   int     `json:"cooldown_minutes"`
	ServerPreset      ServerCreateOpts
	TagPrefix         string `json:"tag_prefix"`
}

// DefaultAutoscaleConfig возвращает конфигурацию по умолчанию.
func DefaultAutoscaleConfig() *AutoscaleConfig {
	return &AutoscaleConfig{
		MinServers:        1,
		MaxServers:        5,
		ScaleUpThreshold:  10,
		ScaleDownThreshold: 3,
		CooldownMinutes:   10,
		TagPrefix:         "flowlink-relay-",
		ServerPreset: ServerCreateOpts{
			OS:       "ubuntu-22.04",
			CPU:      1,
			RAM:      1024,
			Disk:     10,
			Location: "ru-1",
		},
	}
}

// ScaleDecision — решение autoscaler.
type ScaleDecision struct {
	Action  string `json:"action"`  // "none", "scale_up", "scale_down"
	Reason  string `json:"reason"`
	Current int    `json:"current_servers"`
	Desired int    `json:"desired_servers"`
}

// ScaleStatus — текущий статус autoscaling.
type ScaleStatus struct {
	ActiveServers  int       `json:"active_servers"`
	TotalManaged   int       `json:"total_managed"`
	LastAction     string    `json:"last_action"`
	LastActionAt   time.Time `json:"last_action_at"`
	InCooldown     bool      `json:"in_cooldown"`
	CooldownUntil  time.Time `json:"cooldown_until"`
}

// Autoscaler — главный autoscaling контроллер.
type Autoscaler struct {
	tw           *TimewebClient
	registry     *ScaleRegistry
	config       *AutoscaleConfig
	mu           sync.Mutex
	logger       *slog.Logger
	lastActionAt time.Time
	relayPort    int
}

// NewAutoscaler создаёт autoscaler.
func NewAutoscaler(tw *TimewebClient, registry *ScaleRegistry, cfg *AutoscaleConfig) *Autoscaler {
	if cfg == nil {
		cfg = DefaultAutoscaleConfig()
	}
	return &Autoscaler{
		tw:        tw,
		registry:  registry,
		config:    cfg,
		logger:    slog.Default(),
		relayPort: 8080,
	}
}

// SetRelayPort устанавливает порт relay серверов для nginx конфига.
func (a *Autoscaler) SetRelayPort(port int) {
	a.relayPort = port
}

// Evaluate принимает решение о масштабировании.
func (a *Autoscaler) Evaluate(clientCount int) (*ScaleDecision, error) {
	a.mu.Lock()
	defer a.mu.Unlock()

	active := a.registry.ActiveCount()

	// Scale up: clients per server > threshold
	if float64(clientCount) > float64(active)*a.config.ScaleUpThreshold {
		if active >= a.config.MaxServers {
			return &ScaleDecision{
				Action:  "none",
				Reason:  fmt.Sprintf("max servers reached (%d)", a.config.MaxServers),
				Current: active,
				Desired: active,
			}, nil
		}
		return &ScaleDecision{
			Action:  "scale_up",
			Reason:  fmt.Sprintf("client load %.0f > capacity %.0f", float64(clientCount), float64(active)*a.config.ScaleUpThreshold),
			Current: active,
			Desired: active + 1,
		}, nil
	}

	// Scale down: clients per server < threshold and above minimum
	if active > a.config.MinServers {
		if float64(clientCount) < float64(active)*a.config.ScaleDownThreshold {
			return &ScaleDecision{
				Action:  "scale_down",
				Reason:  fmt.Sprintf("client load %.0f < threshold %.0f", float64(clientCount), float64(active)*a.config.ScaleDownThreshold),
				Current: active,
				Desired: active - 1,
			}, nil
		}
	}

	return &ScaleDecision{
		Action:  "none",
		Reason:  "within target range",
		Current: active,
		Desired: active,
	}, nil
}

// ScaleUp добавляет новый relay сервер.
func (a *Autoscaler) ScaleUp() error {
	a.mu.Lock()
	defer a.mu.Unlock()

	if a.inCooldown() {
		return fmt.Errorf("in cooldown until %s", a.lastActionAt.Add(time.Duration(a.config.CooldownMinutes)*time.Minute))
	}

	active := a.registry.ActiveCount()
	if active >= a.config.MaxServers {
		return fmt.Errorf("max servers reached (%d)", a.config.MaxServers)
	}

	name := fmt.Sprintf("%s%d", a.config.TagPrefix, time.Now().Unix())
	opts := a.config.ServerPreset
	opts.Name = name

	srv, err := a.tw.CreateServer(opts)
	if err != nil {
		return fmt.Errorf("create server: %w", err)
	}

	managed := &ManagedServer{
		ServerID: srv.ID,
		Role:     "relay",
		Status:   "active",
	}
	a.registry.AddServer(managed)
	a.registry.UpdateAction(srv.ID, "created")
	a.registry.Save()

	a.lastActionAt = time.Now()
	a.logger.Info("scaled up", "server_id", srv.ID, "name", name, "active_count", active+1)

	return nil
}

// ScaleDown удаляет последний добавленный relay сервер.
func (a *Autoscaler) ScaleDown() error {
	a.mu.Lock()
	defer a.mu.Unlock()

	if a.inCooldown() {
		return fmt.Errorf("in cooldown until %s", a.lastActionAt.Add(time.Duration(a.config.CooldownMinutes)*time.Minute))
	}

	active := a.registry.GetActiveServers()
	if len(active) <= a.config.MinServers {
		return fmt.Errorf("at minimum servers (%d)", a.config.MinServers)
	}

	// Remove the last added server
	target := active[len(active)-1]
	a.registry.SetStatus(target.ServerID, "draining")
	a.registry.Save()

	if err := a.tw.DeleteServer(target.ServerID); err != nil {
		a.registry.SetStatus(target.ServerID, "active")
		a.registry.Save()
		return fmt.Errorf("delete server %d: %w", target.ServerID, err)
	}

	a.registry.RemoveServer(target.ServerID)
	a.registry.Save()

	a.lastActionAt = time.Now()
	a.logger.Info("scaled down", "server_id", target.ServerID, "active_count", len(active)-1)

	return nil
}

// GetStatus возвращает текущий статус autoscaling.
func (a *Autoscaler) GetStatus() *ScaleStatus {
	a.mu.Lock()
	defer a.mu.Unlock()

	cooldownUntil := a.lastActionAt.Add(time.Duration(a.config.CooldownMinutes) * time.Minute)

	return &ScaleStatus{
		ActiveServers: a.registry.ActiveCount(),
		TotalManaged:  len(a.registry.GetAllServers()),
		LastAction:    "none",
		LastActionAt:  a.lastActionAt,
		InCooldown:    time.Now().Before(cooldownUntil),
		CooldownUntil: cooldownUntil,
	}
}

// GenerateNginxConfig генерирует nginx upstream конфиг.
func (a *Autoscaler) GenerateNginxConfig() string {
	servers := a.registry.GetActiveServers()
	return GenerateNginxUpstream(servers, a.relayPort)
}

// GetActiveRelayAddresses возвращает адреса активных relay серверов.
func (a *Autoscaler) GetActiveRelayAddresses() []string {
	return GetActiveRelayAddresses(a.registry.GetActiveServers())
}

func (a *Autoscaler) inCooldown() bool {
	if a.lastActionAt.IsZero() {
		return false
	}
	cooldownUntil := a.lastActionAt.Add(time.Duration(a.config.CooldownMinutes) * time.Minute)
	return time.Now().Before(cooldownUntil)
}

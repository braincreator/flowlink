// Package agent — Kill Switch и Circuit Breaker для flowlink.
// Экстренная остановка и автоматическая защита от перегрузок.


package agent

import (
	"fmt"
	"log/slog"
	"os"
	"runtime"
	"sync"
	"time"
)

// KillSwitchMode — режим работы kill switch.
type KillSwitchMode string

const (
	ModeRunning   KillSwitchMode = "running"   // нормальная работа
	ModePaused    KillSwitchMode = "paused"    // пауза (новые команды не выполняются)
	ModeReadonly  KillSwitchMode = "readonly"  // только чтение (write команды отклоняются)
	ModeEmergency KillSwitchMode = "emergency" // экстренная остановка (все задачи убиты)
)

// KillSwitch — управляет режимами работы агента.
type KillSwitch struct {
	mu            sync.RWMutex
	mode          KillSwitchMode
	pauseReason   string
	pauseUntil    time.Time

	// Circuit Breaker
	consecutiveErrors int
	lastErrorTime     time.Time
	errorWindow       time.Duration // окно для подсчёта ошибок (default: 5min)

	// Мониторинг ресурсов
	cpuHighSince  time.Time
	diskUsage     float64 // процент использования диска

	// Настройки
	cpuThreshold      float64       // порог CPU (default: 95%)
	cpuThresholdDur   time.Duration // длительность превышения (default: 5min)
	diskThreshold     float64       // порог диска (default: 90%)

	// Уведомления
	notifyFn func(event string, details map[string]any)

	logger *slog.Logger
}

// NewKillSwitch — создаёт новый kill switch.
func NewKillSwitch() *KillSwitch {
	ks := &KillSwitch{
		mode:            ModeRunning,
		errorWindow:     5 * time.Minute,
		cpuThreshold:    95.0,
		cpuThresholdDur: 5 * time.Minute,
		diskThreshold:   90.0,
		logger:          slog.Default(),
	}

	// Обработка сигналов
	go ks.handleSignals()

	// Периодический мониторинг ресурсов
	go ks.monitorResources()

	return ks
}

// handleSignals — обрабатывает системные сигналы.
func (k *KillSwitch) handleSignals() {
	sigChan := make(chan os.Signal, 1)
	notifyPlatformSignals(sigChan)

	for sig := range sigChan {
		k.logger.Info("получен сигнал", "signal", sig)
		k.EmergencyStop()
		os.Exit(0)
	}
}

// monitorResources — периодически проверяет использование ресурсов.
func (k *KillSwitch) monitorResources() {
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		k.checkResources()
	}
}

// checkResources — проверяет CPU и диск.
func (k *KillSwitch) checkResources() {
	// Проверка CPU (упрощённо через load average)
	cpuUsage := k.getCPUUsage()

	k.mu.Lock()
	defer k.mu.Unlock()

	// CPU monitoring
	if cpuUsage > k.cpuThreshold {
		if k.cpuHighSince.IsZero() {
			k.cpuHighSince = time.Now()
		} else if time.Since(k.cpuHighSince) > k.cpuThresholdDur {
			// CPU высокий более 5 минут → пауза
			k.mode = ModePaused
			k.pauseReason = fmt.Sprintf("CPU высокая загрузка: %.1f%%", cpuUsage)
			k.logger.Warn("авто-пауза: высокая загрузка CPU", "cpu", cpuUsage)
			k.notify("cpu_pause", map[string]any{"cpu": cpuUsage})
		}
	} else {
		k.cpuHighSince = time.Time{}
	}

	// Disk monitoring
	diskUsage := k.getDiskUsage()
	k.diskUsage = diskUsage

	if diskUsage > k.diskThreshold {
		// Диск почти полный → только чтение
		if k.mode == ModeRunning {
			k.mode = ModeReadonly
			k.pauseReason = fmt.Sprintf("Диск почти полон: %.1f%%", diskUsage)
			k.logger.Warn("авто-readonly: диск почти полон", "disk", diskUsage)
			k.notify("disk_readonly", map[string]any{"disk": diskUsage})
		}
	}
}

// getCPUUsage — возвращает использование CPU (платформо-зависимо).
// Реализация в killswitch_darwin.go и killswitch_linux.go.
func (k *KillSwitch) getCPUUsage() float64 {
	return k.getPlatformCPUUsage()
}

// getDiskUsage — возвращает процент использования диска (платформо-зависимо).
func (k *KillSwitch) getDiskUsage() float64 {
	return k.getPlatformDiskUsage()
}

// IsPaused — проверяет, находится ли агент на паузе.
func (k *KillSwitch) IsPaused() bool {
	k.mu.RLock()
	defer k.mu.RUnlock()

	// Проверяем автоматическую паузу по таймеру
	if k.mode == ModePaused && !k.pauseUntil.IsZero() && time.Now().After(k.pauseUntil) {
		k.mu.RUnlock()
		k.mu.Lock()
		k.mode = ModeRunning
		k.pauseReason = ""
		k.pauseUntil = time.Time{}
		k.mu.Unlock()
		k.mu.RLock()
	}

	return k.mode == ModePaused || k.mode == ModeEmergency
}

// IsReadonly — проверяет, находится ли агент в режиме только чтения.
func (k *KillSwitch) IsReadonly() bool {
	k.mu.RLock()
	defer k.mu.RUnlock()
	return k.mode == ModeReadonly
}

// EmergencyStop — экстренная остановка всех задач.
func (k *KillSwitch) EmergencyStop() {
	k.mu.Lock()
	defer k.mu.Unlock()

	k.mode = ModeEmergency
	k.pauseReason = "Экстренная остановка"

	k.logger.Warn("EMERGENCY STOP активирован")
	k.notify("emergency_stop", nil)
}

// Pause — ставит агент на паузу.
func (k *KillSwitch) Pause(reason string) {
	k.mu.Lock()
	defer k.mu.Unlock()

	k.mode = ModePaused
	k.pauseReason = reason

	k.logger.Info("агент на паузе", "reason", reason)
	k.notify("paused", map[string]any{"reason": reason})
}

// PauseFor — ставит агент на паузу на определённое время.
func (k *KillSwitch) PauseFor(reason string, duration time.Duration) {
	k.mu.Lock()
	defer k.mu.Unlock()

	k.mode = ModePaused
	k.pauseReason = reason
	k.pauseUntil = time.Now().Add(duration)

	k.logger.Info("агент на паузе", "reason", reason, "duration", duration)
	k.notify("paused", map[string]any{"reason": reason, "duration": duration.Seconds()})
}

// Resume — возобновляет работу агента.
func (k *KillSwitch) Resume() {
	k.mu.Lock()
	defer k.mu.Unlock()

	k.mode = ModeRunning
	k.pauseReason = ""
	k.pauseUntil = time.Time{}
	k.consecutiveErrors = 0

	k.logger.Info("агент возобновил работу")
	k.notify("resumed", nil)
}

// CheckCommand — проверяет, можно ли выполнить команду.
func (k *KillSwitch) CheckCommand(cmd string) error {
	k.mu.RLock()
	defer k.mu.RUnlock()

	switch k.mode {
	case ModeEmergency:
		return fmt.Errorf("экстренная остановка: команды не выполняются")
	case ModePaused:
		return fmt.Errorf("агент на паузе: %s", k.pauseReason)
	case ModeReadonly:
		if IsWriteCommand(cmd) {
			return fmt.Errorf("режим только чтения: write-команды отклонены")
		}
	case ModeRunning:
		// Всё ок
	}

	return nil
}

// IsWriteCommand — определяет, является ли командой записью.
func IsWriteCommand(cmd string) bool {
	writePatterns := []string{
		"rm ", "rmdir", "mv ", "cp ",
		"chmod ", "chown ",
		"apt install", "apt remove", "apt upgrade",
		"yum install", "yum remove",
		"docker rm", "docker rmi", "docker run",
		"systemctl stop", "systemctl restart",
		"iptables ",
		"crontab ",
		"echo >", "cat >",
	}

	for _, pattern := range writePatterns {
		if containsPattern(cmd, pattern) {
			return true
		}
	}

	return false
}

// RecordError — записывает ошибку для circuit breaker.
func (k *KillSwitch) RecordError(err error) {
	k.mu.Lock()
	defer k.mu.Unlock()

	now := time.Now()

	// Сбрасываем счётчик если окно прошло
	if now.Sub(k.lastErrorTime) > k.errorWindow {
		k.consecutiveErrors = 0
	}

	k.consecutiveErrors++
	k.lastErrorTime = now

	// 3 consecutive errors → pause 60s
	if k.consecutiveErrors >= 3 {
		k.mode = ModePaused
		k.pauseReason = fmt.Sprintf("Circuit breaker: %d последовательных ошибок", k.consecutiveErrors)
		k.pauseUntil = now.Add(60 * time.Second)

		k.logger.Warn("circuit breaker активирован",
			"errors", k.consecutiveErrors,
			"last_error", err,
		)
		k.notify("circuit_breaker", map[string]any{
			"errors":     k.consecutiveErrors,
			"pause_sec":  60,
			"last_error": err.Error(),
		})
	}
}

// RecordSuccess — записывает успешное выполнение.
func (k *KillSwitch) RecordSuccess() {
	k.mu.Lock()
	defer k.mu.Unlock()
	k.consecutiveErrors = 0
}

// GetMode — возвращает текущий режим.
// Mode — alias для GetMode (используется Policy Layer).
func (k *KillSwitch) Mode() KillSwitchMode {
	return k.GetMode()
}

func (k *KillSwitch) GetMode() KillSwitchMode {
	k.mu.RLock()
	defer k.mu.RUnlock()
	return k.mode
}

// GetStatus — возвращает детальный статус.
func (k *KillSwitch) GetStatus() map[string]any {
	k.mu.RLock()
	defer k.mu.RUnlock()

	return map[string]any{
		"mode":              string(k.mode),
		"pause_reason":      k.pauseReason,
		"pause_until":       k.pauseUntil,
		"consecutive_errors": k.consecutiveErrors,
		"disk_usage":        k.diskUsage,
	}
}

// SetNotifyFn — устанавливает функцию уведомлений.
func (k *KillSwitch) SetNotifyFn(fn func(event string, details map[string]any)) {
	k.mu.Lock()
	defer k.mu.Unlock()
	k.notifyFn = fn
}

// notify — отправляет уведомление.
func (k *KillSwitch) notify(event string, details map[string]any) {
	if k.notifyFn != nil {
		go k.notifyFn(event, details)
	}
}

// containsPattern — проверяет наличие паттерна в команде.
func containsPattern(cmd, pattern string) bool {
	return len(cmd) >= len(pattern) && 
		(cmd == pattern || 
		 (len(cmd) > len(pattern) && cmd[:len(pattern)] == pattern))
}

// GetCPUCount — возвращает количество CPU ядер.
func GetCPUCount() int {
	return runtime.NumCPU()
}

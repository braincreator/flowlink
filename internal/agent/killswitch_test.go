package agent

import (
	"fmt"
	"testing"
	"time"
)

func TestKillSwitch_InitialMode(t *testing.T) {
	ks := NewKillSwitch()
	if ks.GetMode() != ModeRunning {
		t.Errorf("initial mode = %v, want %v", ks.GetMode(), ModeRunning)
	}
}

func TestKillSwitch_EmergencyStop(t *testing.T) {
	ks := NewKillSwitch()
	ks.EmergencyStop()
	if ks.GetMode() != ModeEmergency {
		t.Errorf("after EmergencyStop mode = %v, want %v", ks.GetMode(), ModeEmergency)
	}
	// EmergencyStop устанавливает ModeEmergency, а не ModeReadonly
	// IsPaused() возвращает true для ModePaused И ModeEmergency
	if !ks.IsPaused() {
		t.Error("EmergencyStop should set paused (IsPaused returns true for emergency)")
	}
	if ks.IsReadonly() {
		t.Error("EmergencyStop sets ModeEmergency, not ModeReadonly")
	}
}

func TestKillSwitch_PauseResume(t *testing.T) {
	ks := NewKillSwitch()

	ks.Pause("test")
	if ks.GetMode() != ModePaused {
		t.Errorf("after Pause mode = %v, want %v", ks.GetMode(), ModePaused)
	}
	if !ks.IsPaused() {
		t.Error("should be paused")
	}

	ks.Resume()
	if ks.GetMode() != ModeRunning {
		t.Errorf("after Resume mode = %v, want %v", ks.GetMode(), ModeRunning)
	}
}

func TestKillSwitch_CheckCommand(t *testing.T) {
	ks := NewKillSwitch()

	// Running — всё разрешено
	if err := ks.CheckCommand("ls -la"); err != nil {
		t.Errorf("running: unexpected error: %v", err)
	}

	// Paused — ничего нельзя
	ks.Pause("test")
	if err := ks.CheckCommand("ls -la"); err == nil {
		t.Error("paused: expected error for any command")
	}

	// Emergency — ничего нельзя
	ks.EmergencyStop()
	if err := ks.CheckCommand("echo hello"); err == nil {
		t.Error("emergency: expected error")
	}
}

func TestKillSwitch_CircuitBreaker(t *testing.T) {
	ks := NewKillSwitch()

	// Record errors
	ks.RecordError(fmt.Errorf("err1"))
	ks.RecordError(fmt.Errorf("err2"))
	ks.RecordError(fmt.Errorf("err3"))

	// После 3 ошибок — должен быть paused
	if !ks.IsPaused() {
		t.Error("expected pause after 3 consecutive errors")
	}

	// Resume и success
	ks.Resume()
	ks.RecordSuccess()
	if ks.IsPaused() {
		t.Error("should not be paused after success")
	}
}

func TestKillSwitch_PauseFor(t *testing.T) {
	ks := NewKillSwitch()
	ks.PauseFor("test", 100*time.Millisecond)

	if !ks.IsPaused() {
		t.Error("should be paused immediately")
	}

	time.Sleep(150 * time.Millisecond)

	if ks.IsPaused() {
		t.Error("should auto-resume after duration")
	}
}

func TestIsWriteCommand(t *testing.T) {
	tests := []struct {
		cmd  string
		want bool
	}{
		{"rm -rf /tmp/test", true},
		{"echo hello", false},
		{"touch /tmp/file", false}, // touch нет в writePatterns production кода
		{"cat /etc/passwd", false},
		{"mv a b", true},
		{"ls -la", false},
		{"chmod 755 file", true},
		{"apt remove nginx", true},
		{"docker rm container", true},
		{"systemctl stop nginx", true},
		{"", false},
	}

	for _, tt := range tests {
		got := IsWriteCommand(tt.cmd)
		if got != tt.want {
			t.Errorf("IsWriteCommand(%q) = %v, want %v", tt.cmd, got, tt.want)
		}
	}
}

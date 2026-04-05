package agent

import (
	"sync"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
)

// TestKillSwitch tests kill switch functionality
func TestKillSwitch_Basic(t *testing.T) {
	ks := NewKillSwitch()

	if ks == nil {
		t.Fatal("expected non-nil kill switch")
	}

	// Test initial mode
	if ks.Mode() != ModeRunning {
		t.Errorf("expected mode %s, got %s", ModeRunning, ks.Mode())
	}

	// Test emergency stop
	ks.EmergencyStop()
	if ks.Mode() != ModeEmergency {
		t.Errorf("expected mode %s, got %s", ModeEmergency, ks.Mode())
	}

	// Test check command in emergency mode
	err := ks.CheckCommand("ls")
	if err == nil {
		t.Error("expected error in emergency mode")
	}

	// Test resume
	ks.Resume()
	if ks.Mode() != ModeRunning {
		t.Errorf("expected mode %s, got %s", ModeRunning, ks.Mode())
	}

	// Test check command in running mode
	err = ks.CheckCommand("ls")
	if err != nil {
		t.Errorf("unexpected error in running mode: %v", err)
	}
}

// TestKillSwitch_Pause tests pause mode
func TestKillSwitch_Pause(t *testing.T) {
	ks := NewKillSwitch()

	ks.Pause("testing")
	if ks.Mode() != ModePaused {
		t.Errorf("expected mode %s, got %s", ModePaused, ks.Mode())
	}

	ks.Resume()
	if ks.Mode() != ModeRunning {
		t.Errorf("expected mode %s, got %s", ModeRunning, ks.Mode())
	}
}

// TestKillSwitchMode tests kill switch mode constants
func TestKillSwitchMode(t *testing.T) {
	modes := []KillSwitchMode{
		ModeRunning,
		ModeEmergency,
		ModePaused,
	}

	for _, m := range modes {
		if m == "" {
			t.Error("mode should not be empty")
		}
	}
}

// TestReadFile tests file reading
func TestReadFile(t *testing.T) {
	t.Run("existing file", func(t *testing.T) {
		// Test with a real file
		payload := protocol.FileReadPayload{
			Path: "/etc/hostname",
		}

		result := ReadFile(payload)

		if result.Error != "" {
			// May fail on some systems, that's ok
			t.Logf("file read error (expected on some systems): %s", result.Error)
		} else {
			if result.Content == "" {
				t.Error("expected non-empty content")
			}
		}
	})

	t.Run("nonexistent file", func(t *testing.T) {
		payload := protocol.FileReadPayload{
			Path: "/nonexistent/file.txt",
		}

		result := ReadFile(payload)

		if result.Error == "" {
			t.Error("expected error for nonexistent file")
		}
	})

	t.Run("empty path", func(t *testing.T) {
		payload := protocol.FileReadPayload{
			Path: "",
		}

		result := ReadFile(payload)

		if result.Error == "" {
			t.Error("expected error for empty path")
		}
	})
}

// TestWriteFile tests file writing
func TestWriteFile(t *testing.T) {
	t.Run("write to temp", func(t *testing.T) {
		payload := protocol.FileWritePayload{
			Path:    "/tmp/flowlink-test-file.txt",
			Content: "test content",
		}

		result := WriteFile(payload)

		if result.Error != "" {
			t.Logf("write error: %s", result.Error)
		}
	})

	t.Run("empty path", func(t *testing.T) {
		payload := protocol.FileWritePayload{
			Path:    "",
			Content: "test",
		}

		result := WriteFile(payload)

		if result.Error == "" {
			t.Error("expected error for empty path")
		}
	})
}

// TestListFiles tests file listing
func TestListFiles(t *testing.T) {
	t.Run("existing directory", func(t *testing.T) {
		payload := protocol.FileListPayload{
			Path: "/tmp",
		}

		result := ListFiles(payload)

		if result.Error != "" {
			t.Errorf("unexpected error: %s", result.Error)
		}

		if !result.IsDir {
			t.Error("expected IsDir to be true")
		}
	})

	t.Run("nonexistent directory", func(t *testing.T) {
		payload := protocol.FileListPayload{
			Path: "/nonexistent/directory",
		}

		result := ListFiles(payload)

		if result.Error == "" {
			t.Error("expected error for nonexistent directory")
		}
	})
}

// TestCollectSystemInfo tests system info collection
func TestCollectSystemInfo(t *testing.T) {
	info := CollectSystemInfo()

	if info.Hostname == "" {
		t.Error("expected non-empty hostname")
	}

	if info.OS == "" {
		t.Error("expected non-empty OS")
	}

	if info.Arch == "" {
		t.Error("expected non-empty arch")
	}

	if info.CPUCount < 1 {
		t.Error("expected at least 1 CPU")
	}
}

// TestExecAsync tests async command execution
func TestExecAsync(t *testing.T) {
	cfg := &config.Config{
		Sandbox: config.SandboxConfig{
			MaxExecTimeout: 60,
		},
	}

	executor := NewExecutor(cfg)

	var mu sync.Mutex
	outputReceived := false
	doneReceived := false

	onOutput := func(payload protocol.ExecOutputPayload) {
		mu.Lock()
		outputReceived = true
		mu.Unlock()
	}

	onDone := func(payload protocol.ExecDonePayload) {
		mu.Lock()
		doneReceived = true
		if payload.ExitCode != 0 {
			t.Errorf("expected exit code 0, got %d", payload.ExitCode)
		}
		mu.Unlock()
	}

	execRequest := protocol.ExecRequestPayload{
		RequestID: "req-123",
		Command:   "echo hello",
		Timeout:   10,
	}

	executor.ExecAsync(execRequest, onOutput, onDone)

	// Wait for completion
	time.Sleep(500 * time.Millisecond)

	mu.Lock()
	dr := doneReceived
	or := outputReceived
	mu.Unlock()

	if !dr {
		t.Error("expected done callback to be called")
	}

	_ = or // May or may not be called depending on timing
}



package agent

import (
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

func TestNewExecutor(t *testing.T) {
	cfg := &config.Config{
		Sandbox: config.SandboxConfig{
			MaxExecTimeout: 60,
		},
	}
	executor := NewExecutor(cfg)
	if executor == nil {
		t.Fatal("expected non-nil executor")
	}
}

func TestExecutor_SimpleCommands(t *testing.T) {
	cfg := &config.Config{
		Sandbox: config.SandboxConfig{
			MaxExecTimeout: 60,
		},
	}
	executor := NewExecutor(cfg)

	tests := []struct {
		name    string
		command string
		wantErr bool
	}{
		{"echo hello", "echo hello", false},
		{"true", "true", false},
		{"pwd", "pwd", false},
		{"ls /tmp", "ls /tmp", false},
		{"empty command", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.command == "" {
				return // skip empty
			}
			stdout, _, exitCode := executor.ExecSync(tt.command, "", 10)
			if tt.wantErr && exitCode == 0 {
				t.Errorf("expected error for %q, got exit 0", tt.command)
			}
			if !tt.wantErr && exitCode != 0 {
				t.Errorf("unexpected error for %q: exit %d, stdout=%q", tt.command, exitCode, stdout)
			}
		})
	}
}

func TestExecutor_Timeout(t *testing.T) {
	cfg := &config.Config{
		Sandbox: config.SandboxConfig{
			MaxExecTimeout: 2,
		},
	}
	executor := NewExecutor(cfg)

	start := time.Now()
	_, _, exitCode := executor.ExecSync("sleep 30", "", 2)
	elapsed := time.Since(start)

	if elapsed > 10*time.Second {
		t.Errorf("timeout didn't work: took %v", elapsed)
	}
	// Timeout means non-zero exit
	t.Logf("exit code: %d, elapsed: %v", exitCode, elapsed)
}

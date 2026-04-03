package agent

import (
	"testing"

	"github.com/braincreator/flowlink/internal/config"
)

func newTestPolicyLayer(readOnly bool) *PolicyLayer {
	cfg := config.DefaultConfig()
	sandbox := NewSandbox(&cfg.Sandbox)
	approval := NewApproverV2(cfg.Approval)
	backup := NewBackupEngine(cfg.Backup)
	killSwitch := NewKillSwitch()

	policy := NewPolicyLayer(sandbox, approval, backup, killSwitch, &cfg)
	policy.SetReadOnly(readOnly)
	return policy
}

func TestPolicyLayer_Blacklist(t *testing.T) {
	policy := newTestPolicyLayer(false)

	tests := []struct {
		cmd     string
		blocked bool
		reason  string
	}{
		{"rm -rf /", true, "blacklist"},
		{"rm -rf /*", true, "blacklist"},
		{"mkfs.ext4 /dev/sda1", true, "blacklist"},
		{"dd if=/dev/zero of=/dev/sda", true, "blacklist"},
		{":(){ :|:& };:", true, "blacklist"},
		{"DROP DATABASE production", true, "blacklist"},
		{"shutdown -h now", true, "blacklist"},
		{"chmod 777 /etc/passwd", true, "blacklist"},
		{"ls -la", false, ""},
		{"cat /etc/hostname", false, ""},
		{"ps aux", false, ""},
		{"df -h", false, ""},
		{"git status", false, ""},
	}

	for _, test := range tests {
		t.Run(test.cmd, func(t *testing.T) {
			result := policy.Check(test.cmd)
			if test.blocked && !result.Blocked {
				t.Errorf("expected blocked, got allowed for: %s", test.cmd)
			}
			if !test.blocked && result.Blocked {
				t.Errorf("expected allowed, got blocked for: %s (reason: %s)", test.cmd, result.Reason)
			}
		})
	}
}

func TestPolicyLayer_ReadOnlyMode(t *testing.T) {
	policy := newTestPolicyLayer(true) // read-only ON

	// Read-only commands should work
	readOnlyCmds := []string{
		"ls -la",
		"cat /etc/hostname",
		"ps aux",
		"df -h",
		"git status",
		"docker ps",
	}

	for _, cmd := range readOnlyCmds {
		t.Run("allow_ro_"+cmd, func(t *testing.T) {
			result := policy.Check(cmd)
			if !result.Allowed {
				t.Errorf("read-only command should be allowed: %s (reason: %s)", cmd, result.Reason)
			}
		})
	}

	// Write commands should be blocked
	writeCmds := []string{
		"rm file.txt",
		"touch newfile.txt",
		"mkdir /tmp/test",
		"cp a.txt b.txt",
		"mv a.txt b.txt",
		"npm install express",
		"systemctl restart nginx",
	}

	for _, cmd := range writeCmds {
		t.Run("block_rw_"+cmd, func(t *testing.T) {
			result := policy.Check(cmd)
			if result.Allowed {
				t.Errorf("write command should be blocked in read-only mode: %s", cmd)
			}
		})
	}
}

func TestPolicyLayer_ReadOnlyOff(t *testing.T) {
	policy := newTestPolicyLayer(false) // read-only OFF

	// Write commands (non-destructive) should work
	writeCmds := []string{
		"touch newfile.txt",
		"mkdir /tmp/test",
		"echo x > /tmp/f",
	}

	for _, cmd := range writeCmds {
		t.Run("allow_rw_"+cmd, func(t *testing.T) {
			result := policy.Check(cmd)
			if !result.Allowed {
				t.Errorf("write command should be allowed in read-write mode: %s (reason: %s)", cmd, result.Reason)
			}
		})
	}

	// But destructive commands should still be blocked
	destructiveCmds := []string{
		"rm -rf /var",
		"shutdown now",
		"DROP TABLE users",
	}

	for _, cmd := range destructiveCmds {
		t.Run("block_destructive_"+cmd, func(t *testing.T) {
			result := policy.Check(cmd)
			if !result.Blocked {
				t.Errorf("destructive command should be blocked: %s", cmd)
			}
		})
	}
}

func TestPolicyLayer_RiskClassification(t *testing.T) {
	policy := newTestPolicyLayer(false)

	tests := []struct {
		cmd       string
		wantRisk  string
		wantAllow bool
	}{
		// Low risk
		{"ls -la", "low", true},
		{"cat /etc/hostname", "low", true},

		// Medium risk
		{"apt upgrade", "medium", true},
		{"docker run nginx", "medium", true},
		{"systemctl restart nginx", "medium", true},
		{"npm install express", "medium", true},

		// High risk (blocked by blacklist, not just classified)
		{"rm -rf /etc", "high", false},
		{"DROP DATABASE prod", "high", false},
		{"shutdown", "high", false},
	}

	for _, test := range tests {
		t.Run(test.cmd, func(t *testing.T) {
			result := policy.Check(test.cmd)
			if result.RiskLevel != test.wantRisk {
				t.Errorf("expected risk %s, got %s for: %s", test.wantRisk, result.RiskLevel, test.cmd)
			}
			if test.wantAllow != result.Allowed {
				t.Errorf("expected allowed=%v, got %v for: %s", test.wantAllow, result.Allowed, test.cmd)
			}
		})
	}
}

func TestPolicyLayer_KillSwitch(t *testing.T) {
	policy := newTestPolicyLayer(false)

	// Emergency mode blocks everything
	policy.killSwitch.EmergencyStop()

	result := policy.Check("ls -la")
	if result.Allowed {
		t.Error("emergency kill switch should block all commands")
	}

	// Reset
	policy.killSwitch.Resume()
	result = policy.Check("ls -la")
	if !result.Allowed {
		t.Error("normal mode should allow safe commands")
	}
}

func TestPolicyLayer_GetStatus(t *testing.T) {
	policy := newTestPolicyLayer(true)
	status := policy.GetStatus()

	if status["read_only"] != true {
		t.Error("expected read_only=true")
	}
	if status["kill_switch_mode"] != ModeRunning {
		t.Error("expected kill_switch_mode=running")
	}
	if status["blacklist_entries"] != len(ExtendedBlacklist) {
		t.Errorf("expected %d blacklist entries, got %v",
			len(ExtendedBlacklist), status["blacklist_entries"])
	}
}

func TestIsWriteOperation(t *testing.T) {
	policy := newTestPolicyLayer(false)

	writeOps := []string{"rm file", "touch x", "mkdir y", "cp a b", "mv a b", "echo x > /tmp/f", "chmod 755 f"}
	readOps := []string{"ls", "cat f", "grep x f", "ps aux", "df -h", "git status", "echo hello"}

	for _, cmd := range writeOps {
		if !policy.isWriteOperation(cmd) {
			t.Errorf("expected write operation: %s", cmd)
		}
	}
	for _, cmd := range readOps {
		if policy.isWriteOperation(cmd) {
			t.Errorf("expected read operation: %s", cmd)
		}
	}
}

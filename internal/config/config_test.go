package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

// --- DefaultConfig ---

func TestDefaultConfig_NonNil(t *testing.T) {
	_ = DefaultConfig() // should not panic
}

func TestDefaultConfig_Values(t *testing.T) {
	cfg := DefaultConfig()

	tests := []struct {
		name  string
		got   any
		want  any
	}{
		{"HeartbeatSec", cfg.HeartbeatSec, 30},
		{"RelayURL", cfg.RelayURL, "wss://relay.flowmasters.ru/ws"},
		{"ReadOnly", *cfg.ReadOnly, true},
		{"Sandbox.AllowSudo", cfg.Sandbox.AllowSudo, false},
		{"Sandbox.MaxFileSize", cfg.Sandbox.MaxFileSize, int64(100 * 1024 * 1024)},
		{"Sandbox.MaxExecTimeout", cfg.Sandbox.MaxExecTimeout, 300},
		{"Approval.Mode", cfg.Approval.Mode, "auto"},
		{"Approval.SoftAskNotify", cfg.Approval.SoftAskNotify, true},
		{"Approval.HardAskTimeout", cfg.Approval.HardAskTimeout, 3600},
		{"Approval.MaxRetries", cfg.Approval.MaxRetries, 3},
		{"Backup.MaxSnapshots", cfg.Backup.MaxSnapshots, 50},
		{"Backup.RetentionDays", cfg.Backup.RetentionDays, 7},
		{"Backup.Enabled", cfg.Backup.Enabled, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("got %v, want %v", tt.got, tt.want)
			}
		})
	}
}

func TestDefaultConfig_BlockedPatterns(t *testing.T) {
	cfg := DefaultConfig()
	patterns := []string{"rm -rf /*", "mkfs*", "dd if=*", ":(){ :|:& };:"}
	for _, p := range patterns {
		found := false
		for _, bp := range cfg.Sandbox.BlockedPatterns {
			if bp == p {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("default blocked pattern %q not found", p)
		}
	}
}

// --- DefaultRelayConfig ---

func TestDefaultRelayConfig(t *testing.T) {
	cfg := DefaultRelayConfig()
	if cfg.WSSAddr != ":8443" {
		t.Errorf("WSSAddr = %q, want :8443", cfg.WSSAddr)
	}
	if cfg.APIAddr != ":8080" {
		t.Errorf("APIAddr = %q, want :8080", cfg.APIAddr)
	}
	if cfg.HeartbeatTimeout != 90 {
		t.Errorf("HeartbeatTimeout = %d, want 90", cfg.HeartbeatTimeout)
	}
	if cfg.MaxAgents != 100 {
		t.Errorf("MaxAgents = %d, want 100", cfg.MaxAgents)
	}
}

// --- DefaultKillSwitchConfig ---

func TestDefaultKillSwitchConfig(t *testing.T) {
	cfg := DefaultKillSwitchConfig()
	if cfg.DiskThreshold != 90.0 {
		t.Errorf("DiskThreshold = %f, want 90.0", cfg.DiskThreshold)
	}
	if cfg.CPUThreshold != 95.0 {
		t.Errorf("CPUThreshold = %f, want 95.0", cfg.CPUThreshold)
	}
	if cfg.CPUThresholdDur != 300 {
		t.Errorf("CPUThresholdDur = %d, want 300", cfg.CPUThresholdDur)
	}
}

// --- DefaultTaskConfig ---

func TestDefaultTaskConfig(t *testing.T) {
	cfg := DefaultTaskConfig()
	if cfg.MaxSteps != 20 {
		t.Errorf("MaxSteps = %d, want 20", cfg.MaxSteps)
	}
	if cfg.ApprovalMode != "auto" {
		t.Errorf("ApprovalMode = %q, want auto", cfg.ApprovalMode)
	}
}

// --- ConfigDir / ConfigPath ---

func TestConfigDir_Default(t *testing.T) {
	t.Setenv("FLOWLINK_CONFIG_DIR", "")
	dir, err := ConfigDir()
	if err != nil {
		t.Fatalf("ConfigDir() error: %v", err)
	}
	home, _ := os.UserHomeDir()
	want := filepath.Join(home, ".flowlink")
	if dir != want {
		t.Errorf("ConfigDir() = %q, want %q", dir, want)
	}
}

func TestConfigDir_Custom(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)
	dir, err := ConfigDir()
	if err != nil {
		t.Fatalf("ConfigDir() error: %v", err)
	}
	if dir != tmpDir {
		t.Errorf("ConfigDir() = %q, want %q", dir, tmpDir)
	}
}

func TestConfigPath(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)
	path, err := ConfigPath()
	if err != nil {
		t.Fatalf("ConfigPath() error: %v", err)
	}
	want := filepath.Join(tmpDir, "config.json")
	if path != want {
		t.Errorf("ConfigPath() = %q, want %q", path, want)
	}
}

// --- LoadConfig ---

func TestLoadConfig_MissingFile_ReturnsDefault(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)
	cfg, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() error: %v", err)
	}
	if cfg == nil {
		t.Fatal("LoadConfig() returned nil")
	}
	if cfg.HeartbeatSec != 30 {
		t.Errorf("HeartbeatSec = %d, want 30 (default)", cfg.HeartbeatSec)
	}
}

func TestLoadConfig_ValidJSON(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)

	input := Config{
		AgentID:       "test-agent",
		Token:         "test-token",
		RelayURL:      "wss://test.example.com/ws",
		HeartbeatSec:  60,
		Label:         "test-label",
		ReadOnly:      boolPtr(false),
		Approval:      ApprovalConfigV2{Mode: "hard_ask"},
	}
	data, _ := json.Marshal(input)
	os.WriteFile(filepath.Join(tmpDir, "config.json"), data, 0600)

	cfg, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() error: %v", err)
	}
	if cfg.AgentID != "test-agent" {
		t.Errorf("AgentID = %q, want test-agent", cfg.AgentID)
	}
	if cfg.HeartbeatSec != 60 {
		t.Errorf("HeartbeatSec = %d, want 60", cfg.HeartbeatSec)
	}
	if cfg.Approval.Mode != "hard_ask" {
		t.Errorf("Approval.Mode = %q, want hard_ask", cfg.Approval.Mode)
	}
	if *cfg.ReadOnly != false {
		t.Errorf("ReadOnly = %v, want false", *cfg.ReadOnly)
	}
}

func TestLoadConfig_InvalidJSON(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)
	os.WriteFile(filepath.Join(tmpDir, "config.json"), []byte("{invalid json}"), 0600)

	_, err := LoadConfig()
	if err == nil {
		t.Fatal("LoadConfig() expected error for invalid JSON, got nil")
	}
}

func TestLoadConfig_FillsDefaults(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)
	// Partial config — no HeartbeatSec, no Label
	os.WriteFile(filepath.Join(tmpDir, "config.json"), []byte(`{"agent_id":"a","token":"t"}`), 0600)

	cfg, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() error: %v", err)
	}
	if cfg.HeartbeatSec != 30 {
		t.Errorf("HeartbeatSec = %d, want 30 (default filled)", cfg.HeartbeatSec)
	}
	if cfg.Label == "" {
		t.Error("Label should be filled with hostname")
	}
}

// --- SaveConfig ---

func TestSaveConfig_RoundTrip(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("FLOWLINK_CONFIG_DIR", tmpDir)

	original := DefaultConfig()
	original.AgentID = "save-test"
	original.HeartbeatSec = 15

	err := SaveConfig(&original)
	if err != nil {
		t.Fatalf("SaveConfig() error: %v", err)
	}

	// Verify file exists and has correct permissions
	path := filepath.Join(tmpDir, "config.json")
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("config file not created: %v", err)
	}
	if info.Mode().Perm()&0o777 != 0o600 {
		t.Errorf("file permissions = %o, want 0600", info.Mode().Perm()&0o777)
	}

	// Load and compare
	loaded, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() after save error: %v", err)
	}
	if loaded.AgentID != "save-test" {
		t.Errorf("AgentID = %q, want save-test", loaded.AgentID)
	}
	if loaded.HeartbeatSec != 15 {
		t.Errorf("HeartbeatSec = %d, want 15", loaded.HeartbeatSec)
	}
}

// --- LoadRelayConfig ---

func TestLoadRelayConfig_Valid(t *testing.T) {
	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "relay.json")
	data := `{"wss_addr":":9443","api_addr":":9090","heartbeat_timeout_sec":120,"max_agents":50}`
	os.WriteFile(path, []byte(data), 0600)

	cfg, err := LoadRelayConfig(path)
	if err != nil {
		t.Fatalf("LoadRelayConfig() error: %v", err)
	}
	if cfg.WSSAddr != ":9443" {
		t.Errorf("WSSAddr = %q, want :9443", cfg.WSSAddr)
	}
	if cfg.HeartbeatTimeout != 120 {
		t.Errorf("HeartbeatTimeout = %d, want 120", cfg.HeartbeatTimeout)
	}
	if cfg.MaxAgents != 50 {
		t.Errorf("MaxAgents = %d, want 50", cfg.MaxAgents)
	}
}

func TestLoadRelayConfig_InvalidJSON(t *testing.T) {
	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "relay.json")
	os.WriteFile(path, []byte("not json"), 0600)

	_, err := LoadRelayConfig(path)
	if err == nil {
		t.Fatal("LoadRelayConfig() expected error for invalid JSON")
	}
}

func TestLoadRelayConfig_MissingFile(t *testing.T) {
	_, err := LoadRelayConfig("/nonexistent/path.json")
	if err == nil {
		t.Fatal("LoadRelayConfig() expected error for missing file")
	}
}

func TestLoadRelayConfig_FillsDefaults(t *testing.T) {
	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "relay.json")
	os.WriteFile(path, []byte(`{"wss_addr":":8443"}`), 0600)

	cfg, err := LoadRelayConfig(path)
	if err != nil {
		t.Fatalf("LoadRelayConfig() error: %v", err)
	}
	if cfg.HeartbeatTimeout != 90 {
		t.Errorf("HeartbeatTimeout = %d, want 90 (default)", cfg.HeartbeatTimeout)
	}
}

// --- OSInfo ---

func TestOSInfo(t *testing.T) {
	osName, arch := OSInfo()
	if osName == "" {
		t.Error("OSInfo() returned empty osName")
	}
	if arch == "" {
		t.Error("OSInfo() returned empty arch")
	}
}

// --- JSON marshalling round-trip for all config types ---

func TestConfigTypes_JSONRoundTrip(t *testing.T) {
	tests := []struct {
		name string
		obj  any
	}{
		{"SandboxConfig", SandboxConfig{AllowedDirs: []string{"/tmp"}, BlockedPatterns: []string{"rm -rf"}, MaxFileSize: 1024, MaxExecTimeout: 60, AllowSudo: false}},
		{"ApprovalConfigV2", ApprovalConfigV2{Mode: "soft_ask", SoftAskNotify: false, HardAskTimeout: 1800, MaxRetries: 5}},
		{"KillSwitchConfig", KillSwitchConfig{DiskThreshold: 80.0, CPUThreshold: 90.0, CPUThresholdDur: 600}},
		{"BackupConfig", BackupConfig{MaxSnapshots: 10, MaxTotalSize: 1024, RetentionDays: 3, BackupDir: "/backup", Enabled: false}},
		{"PaymentConfig", PaymentConfig{Provider: "tochka", TochkaClientID: "c1", WebhookURL: "https://example.com/hook"}},
		{"AutoscaleConfig", AutoscaleConfig{Enabled: true, Provider: "timeweb", MinServers: 2, MaxServers: 10, CooldownMinutes: 5}},
		{"TelegramBotConfig", TelegramBotConfig{Token: "bot:123", AllowedIDs: []int64{111, 222}, NotifyOn: []string{"exec", "error"}}},
		{"LLMBackendConfig", LLMBackendConfig{Name: "gpt4", URL: "https://api.openai.com", APIKey: "sk-xxx", Priority: 1, Provider: "openai_compatible"}},
		{"E2EEConfig", E2EEConfig{Enabled: true, AutoRotate: true}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			data, err := json.Marshal(tt.obj)
			if err != nil {
				t.Fatalf("Marshal error: %v", err)
			}
			if len(data) == 0 {
				t.Fatal("Marshal returned empty bytes")
			}
			// Verify it's valid JSON
			var raw map[string]any
			if err := json.Unmarshal(data, &raw); err != nil {
				t.Fatalf("Unmarshal back error: %v", err)
			}
		})
	}
}

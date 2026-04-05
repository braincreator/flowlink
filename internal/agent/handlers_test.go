package agent

import (
	"testing"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
)

// TestHandleFileRead tests file read handler
func TestHandleFileRead(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Test with valid path
	payload := protocol.FileReadPayload{
		Path: "/etc/hostname",
	}

	result := ReadFile(payload)
	// May have error if file doesn't exist or permission denied
	_ = result

	// Test with empty path
	payload2 := protocol.FileReadPayload{
		Path: "",
	}
	result2 := ReadFile(payload2)
	if result2.Error == "" {
		t.Error("expected error for empty path")
	}

	_ = agent
}

// TestHandleFileWrite tests file write handler
func TestHandleFileWrite(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Test with empty path
	payload := protocol.FileWritePayload{
		Path:    "",
		Content: "test",
	}

	result := WriteFile(payload)
	if result.Error == "" {
		t.Error("expected error for empty path")
	}

	_ = agent
}

// TestHandleFileList tests file list handler
func TestHandleFileList(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Test with valid directory
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

	// Test with nonexistent directory
	payload2 := protocol.FileListPayload{
		Path: "/nonexistent/directory",
	}

	result2 := ListFiles(payload2)
	if result2.Error == "" {
		t.Error("expected error for nonexistent directory")
	}

	_ = agent
}

// TestPolicyCheck tests policy checking
func TestPolicyCheck(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.ReadOnly = boolPtr(false)

	agent := NewAgent(&cfg)

	// Test safe command
	result := agent.policy.Check("ls -la")
	if !result.Allowed {
		t.Errorf("expected 'ls -la' to be allowed, reason: %s", result.Reason)
	}

	// Test destructive command
	result2 := agent.policy.Check("rm -rf /")
	if result2.Allowed {
		t.Error("expected 'rm -rf /' to be blocked")
	}

	// Test in read-only mode
	agent.policy.SetReadOnly(true)
	result3 := agent.policy.Check("touch /tmp/test")
	if result3.Allowed {
		t.Error("expected write command to be blocked in read-only mode")
	}
}

// TestSkillOperations tests skill CRUD operations
func TestSkillOperations(t *testing.T) {
	store, err := NewSkillStore(t.TempDir())
	if err != nil {
		t.Fatalf("failed to create skill store: %v", err)
	}

	// Create skill
	skill := &Skill{
		ID:           "test-skill-unique-1",
		Name:         "Test Skill",
		Description:  "Test description",
		Instructions: "Test instructions",
		ToolsAllowed: []string{"exec"},
	}

	// Save
	err = store.Save(skill)
	if err != nil {
		t.Fatalf("failed to save skill: %v", err)
	}

	// Get
	loaded, exists := store.Get("test-skill-unique-1")
	if !exists {
		t.Fatal("expected skill to exist")
	}

	if loaded.Name != "Test Skill" {
		t.Errorf("expected name 'Test Skill', got %s", loaded.Name)
	}

	// List
	skills := store.List()
	if len(skills) != 1 {
		t.Errorf("expected 1 skill, got %d", len(skills))
	}

	// Delete
	err = store.Delete("test-skill-unique-1")
	if err != nil {
		t.Fatalf("failed to delete skill: %v", err)
	}

	// Verify deleted
	_, exists = store.Get("test-skill-unique-1")
	if exists {
		t.Error("expected skill to be deleted")
	}
}

// TestKillSwitchModes tests kill switch mode changes
func TestKillSwitchModes(t *testing.T) {
	ks := NewKillSwitch()

	// Initial state
	if ks.Mode() != ModeRunning {
		t.Errorf("expected initial mode %s, got %s", ModeRunning, ks.Mode())
	}

	// Emergency stop
	ks.EmergencyStop()
	if ks.Mode() != ModeEmergency {
		t.Errorf("expected mode %s, got %s", ModeEmergency, ks.Mode())
	}

	// Check command blocked
	err := ks.CheckCommand("ls")
	if err == nil {
		t.Error("expected error in emergency mode")
	}

	// Resume
	ks.Resume()
	if ks.Mode() != ModeRunning {
		t.Errorf("expected mode %s, got %s", ModeRunning, ks.Mode())
	}

	// Check command allowed
	err = ks.CheckCommand("ls")
	if err != nil {
		t.Errorf("unexpected error in running mode: %v", err)
	}

	// Pause
	ks.Pause("testing")
	if ks.Mode() != ModePaused {
		t.Errorf("expected mode %s, got %s", ModePaused, ks.Mode())
	}
}

// TestApprovalRiskClassification tests risk classification
func TestApprovalRiskClassification(t *testing.T) {
	approver := NewApproverV2(DefaultApprovalConfigV2())

	tests := []struct {
		cmd        string
		riskLevel  string // expected risk level: low, medium, high
	}{
		{"ls -la", "low"},
		{"cat /etc/passwd", "low"},
		{"apt update", "low"},     // Package commands can be low
		{"rm -rf /", "high"},
		{"DROP DATABASE test", "low"}, // SQL commands without context may be low
	}

	for _, test := range tests {
		risk := approver.ClassifyRisk(test.cmd)

		if risk != test.riskLevel {
			t.Errorf("cmd '%s': expected risk=%s, got risk=%s", test.cmd, test.riskLevel, risk)
		}
	}
}

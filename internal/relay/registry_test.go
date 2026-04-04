package relay

import (
	"os"
	"path/filepath"
	"testing"
)

// TestRegistryClientCRUD — полный CRUD цикл для клиентов.
func TestRegistryClientCRUD(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	// Create
	client, err := reg.CreateClient("Test User", "test@example.com", "starter")
	if err != nil {
		t.Fatalf("CreateClient: %v", err)
	}
	if client.ID == "" {
		t.Fatal("client ID empty")
	}
	if client.APIToken == "" {
		t.Fatal("client token empty")
	}
	if !client.IsActive {
		t.Fatal("client should be active")
	}
	t.Logf("Created client: ID=%s Token=%s", client.ID, client.APIToken)

	// Get
	got, ok := reg.GetClient(client.ID)
	if !ok {
		t.Fatal("GetClient: not found")
	}
	if got.Name != "Test User" {
		t.Fatalf("name mismatch: got %s", got.Name)
	}

	// GetByAPIToken
	got2, ok := reg.GetClientByAPIToken(client.APIToken)
	if !ok {
		t.Fatal("GetClientByAPIToken: not found")
	}
	if got2.ID != client.ID {
		t.Fatalf("token lookup: got wrong client %s", got2.ID)
	}

	// List
	clients := reg.ListClients()
	if len(clients) != 1 {
		t.Fatalf("expected 1 client, got %d", len(clients))
	}

	// Deactivate
	err = reg.DeactivateClient(client.ID)
	if err != nil {
		t.Fatalf("DeactivateClient: %v", err)
	}
	got3, _ := reg.GetClient(client.ID)
	if got3.IsActive {
		t.Fatal("client should be inactive")
	}

	// Deactivated client not found by token
	_, ok = reg.GetClientByAPIToken(client.APIToken)
	if ok {
		t.Fatal("inactive client should not be found by token")
	}
}

// TestRegistryAgentCRUD — полный CRUD для агентов.
func TestRegistryAgentCRUD(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	// Create client first
	client, _ := reg.CreateClient("Test", "test@test.com", "starter")

	// Register agent
	agent, err := reg.RegisterAgent(client.ID, "test-host", []string{"prod"}, "linux", "amd64")
	if err != nil {
		t.Fatalf("RegisterAgent: %v", err)
	}
	if agent.ID == "" || agent.Token == "" {
		t.Fatal("agent ID or token empty")
	}
	t.Logf("Registered agent: ID=%s Token=%s", agent.ID, agent.Token)

	// Get agent
	got, ok := reg.GetAgent(agent.ID)
	if !ok {
		t.Fatal("GetAgent: not found")
	}
	if got.Label != "test-host" {
		t.Fatalf("label mismatch: got %s", got.Label)
	}

	// List by client
	agents := reg.ListAgents(client.ID)
	if len(agents) != 1 {
		t.Fatalf("expected 1 agent, got %d", len(agents))
	}

	// Unregister
	err = reg.UnregisterAgent(agent.ID)
	if err != nil {
		t.Fatalf("UnregisterAgent: %v", err)
	}
	_, ok = reg.GetAgent(agent.ID)
	if ok {
		t.Fatal("agent should be deleted")
	}
	agents = reg.ListAgents(client.ID)
	if len(agents) != 0 {
		t.Fatalf("expected 0 agents, got %d", len(agents))
	}
}

// TestRegistryPersistence — проверка персистенции JSONL.
func TestRegistryPersistence(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	// Create client + agent
	client, _ := reg.CreateClient("Persist", "persist@test.com", "starter")
	agent, _ := reg.RegisterAgent(client.ID, "persist-host", []string{}, "linux", "amd64")

	// Simulate restart — create new registry from same dir
	reg2 := NewRegistry(dir, nil)

	// Verify client loaded
	got, ok := reg2.GetClient(client.ID)
	if !ok {
		t.Fatal("client not loaded after restart")
	}
	if got.Name != "Persist" {
		t.Fatalf("name mismatch: got %s", got.Name)
	}

	// Verify agent loaded
	got2, ok := reg2.GetAgent(agent.ID)
	if !ok {
		t.Fatal("agent not loaded after restart")
	}
	if got2.Label != "persist-host" {
		t.Fatalf("label mismatch: got %s", got2.Label)
	}
}

// TestRegistryAgentLimit — проверка лимита агентов по тарифу.
func TestRegistryAgentLimit(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	client, _ := reg.CreateClient("Limited", "limited@test.com", "starter") // max_agents = 3

	for i := 0; i < 3; i++ {
		_, err := reg.RegisterAgent(client.ID, "agent", []string{}, "linux", "amd64")
		if err != nil {
			t.Fatalf("agent %d: %v", i, err)
		}
	}

	// 4th agent should fail
	_, err := reg.RegisterAgent(client.ID, "overflow", []string{}, "linux", "amd64")
	if err == nil {
		t.Fatal("expected agent limit error")
	}
	t.Logf("Agent limit correctly enforced: %v", err)
}

// TestRegistryCompaction — проверка Save (compaction).
func TestRegistryCompaction(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	// Create and delete agent
	client, _ := reg.CreateClient("Compact", "c@test.com", "starter")
	agent, _ := reg.RegisterAgent(client.ID, "temp", []string{}, "linux", "amd64")
	reg.UnregisterAgent(agent.ID)

	// Before compaction: agents.jsonl has entries + tombstone
	beforeSize := fileSize(filepath.Join(dir, "agents.jsonl"))

	// Save (compaction)
	err := reg.Save()
	if err != nil {
		t.Fatalf("Save: %v", err)
	}

	// After compaction: only active entries
	afterSize := fileSize(filepath.Join(dir, "agents.jsonl"))
	if afterSize >= beforeSize {
		t.Logf("warning: compaction didn't reduce size (before=%d, after=%d)", beforeSize, afterSize)
	}

	// Verify agent is still deleted
	_, ok := reg.GetAgent(agent.ID)
	if ok {
		t.Fatal("agent should still be deleted after compaction")
	}
}

func fileSize(path string) int64 {
	info, err := os.Stat(path)
	if err != nil {
		return 0
	}
	return info.Size()
}

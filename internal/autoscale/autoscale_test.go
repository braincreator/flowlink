package autoscale

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"sync/atomic"
	"testing"
	"time"
)

var serverIDCounter int64

func newTestServer() *httptest.Server {
	serverIDCounter = 100

	mux := http.NewServeMux()

	// POST /servers/cloud — create
	mux.HandleFunc("/servers/cloud", func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodPost {
			var body map[string]interface{}
			json.NewDecoder(r.Body).Decode(&body)
			id := atomic.AddInt64(&serverIDCounter, 1)
			resp := map[string]interface{}{
				"server": map[string]interface{}{
					"id": id,
					"name": body["name"],
					"status": map[string]interface{}{"status": "on"},
					"ips": []map[string]interface{}{
						{"type": "v4", "address": "1.2.3.4"},
					},
					"os":           map[string]interface{}{"name": "ubuntu-22.04"},
					"configuration": map[string]interface{}{"vcpu": 1, "memory_mb": 1024, "disk_gb": 10},
					"bandwidth":    100,
					"location":     "ru-1",
					"created_at":   time.Now().Format(time.RFC3339),
				},
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(resp)
			return
		}
		if r.Method == http.MethodGet {
			resp := map[string]interface{}{
				"servers": []interface{}{},
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(resp)
		}
	})

	// Per-server operations
	mux.HandleFunc("/servers/cloud/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodGet {
			id := r.URL.Path[len("/servers/cloud/"):]
			resp := map[string]interface{}{
				"server": map[string]interface{}{
					"id": id,
					"name": "test-server",
					"status": map[string]interface{}{"status": "on"},
					"ips": []map[string]interface{}{
						{"type": "v4", "address": "1.2.3.4"},
					},
					"os":           map[string]interface{}{"name": "ubuntu-22.04"},
					"configuration": map[string]interface{}{"vcpu": 1, "memory_mb": 1024, "disk_gb": 10},
					"bandwidth":    100,
					"location":     "ru-1",
					"created_at":   time.Now().Format(time.RFC3339),
				},
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(resp)
			return
		}
		if r.Method == http.MethodDelete {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`{"message":"deleted"}`))
			return
		}
		if r.Method == http.MethodPost {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`{"message":"ok"}`))
		}
	})

	return httptest.NewServer(mux)
}

func TestTimewebClient_CreateDelete(t *testing.T) {
	ts := newTestServer()
	defer ts.Close()

	client := NewTimewebClient("test-token")
	client.baseURL = ts.URL

	// Create
	srv, err := client.CreateServer(ServerCreateOpts{
		Name: "test-relay", OS: "ubuntu-22.04", CPU: 1, RAM: 1024, Disk: 10, Location: "ru-1",
	})
	if err != nil {
		t.Fatalf("CreateServer: %v", err)
	}
	if srv.IP != "1.2.3.4" {
		t.Errorf("expected IP 1.2.3.4, got %s", srv.IP)
	}
	if srv.CPU != 1 || srv.RAM != 1024 {
		t.Errorf("unexpected CPU/RAM: %d/%d", srv.CPU, srv.RAM)
	}

	// Get
	_, err = client.GetServer(12345)
	if err != nil {
		t.Fatalf("GetServer: %v", err)
	}

	// List
	servers, err := client.ListServers()
	if err != nil {
		t.Fatalf("ListServers: %v", err)
	}
	if len(servers) != 0 {
		t.Errorf("expected 0 servers in list, got %d", len(servers))
	}

	// Delete
	err = client.DeleteServer(int(srv.ID))
	if err != nil {
		t.Fatalf("DeleteServer: %v", err)
	}
}

func testAutoscaler(t *testing.T) (*Autoscaler, func()) {
	t.Helper()
	ts := newTestServer()
	dir := t.TempDir()
	reg, err := NewScaleRegistry(dir)
	if err != nil {
		t.Fatalf("registry: %v", err)
	}
	client := NewTimewebClient("test-token")
	client.baseURL = ts.URL
	cfg := DefaultAutoscaleConfig()
	cfg.MaxServers = 3
	a := NewAutoscaler(client, reg, cfg)
	return a, ts.Close
}

func TestAutoscaler_Evaluate(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()

	// With 0 servers, any clients > 0 triggers scale up (0 * 10 = 0)
	dec, _ := a.Evaluate(0)
	if dec.Action != "none" {
		t.Errorf("0 clients → none, got %s", dec.Action)
	}

	dec, _ = a.Evaluate(1)
	if dec.Action != "scale_up" {
		t.Errorf("1 client with 0 servers → scale_up, got %s", dec.Action)
	}

	// Add a server, test normal operation
	a.registry.AddServer(&ManagedServer{ServerID: 1, Role: "relay", Status: "active"})

	dec, _ = a.Evaluate(5)
	if dec.Action != "none" {
		t.Errorf("5 clients/1 server → none, got %s", dec.Action)
	}

	dec, _ = a.Evaluate(11)
	if dec.Action != "scale_up" {
		t.Errorf("11 clients/1 server → scale_up, got %s", dec.Action)
	}
}

func TestAutoscaler_ScaleUp(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()

	if err := a.ScaleUp(); err != nil {
		t.Fatalf("ScaleUp: %v", err)
	}
	if a.registry.ActiveCount() != 1 {
		t.Errorf("expected 1 active, got %d", a.registry.ActiveCount())
	}
}

func TestAutoscaler_ScaleDown(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()

	// Add two servers
	a.ScaleUp()
	a.lastActionAt = time.Time{}
	a.ScaleUp()
	a.lastActionAt = time.Time{}

	if a.registry.ActiveCount() != 2 {
		t.Fatalf("expected 2 active, got %d", a.registry.ActiveCount())
	}

	if err := a.ScaleDown(); err != nil {
		t.Fatalf("ScaleDown: %v", err)
	}
	if a.registry.ActiveCount() != 1 {
		t.Errorf("expected 1 active after scale down, got %d", a.registry.ActiveCount())
	}
}

func TestAutoscaler_Cooldown(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()
	a.config.CooldownMinutes = 10

	a.ScaleUp()
	err := a.ScaleUp()
	if err == nil {
		t.Error("expected cooldown error")
	}
}

func TestAutoscaler_MinMaxBounds(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()
	a.config.MinServers = 1
	a.config.MaxServers = 2

	// Can't scale down below min
	err := a.ScaleDown()
	if err == nil {
		t.Error("expected min servers error")
	}

	// Scale up to max
	a.ScaleUp()
	a.lastActionAt = time.Time{}
	a.ScaleUp()
	a.lastActionAt = time.Time{}

	// Can't exceed max
	err = a.ScaleUp()
	if err == nil {
		t.Error("expected max servers error")
	}
}

func TestNginxConfig_Generation(t *testing.T) {
	servers := []*ManagedServer{
		{ServerID: 1, Role: "relay", Status: "active"},
		{ServerID: 2, Role: "relay", Status: "active"},
		{ServerID: 3, Role: "relay", Status: "draining"},
	}

	config := GenerateNginxUpstream(servers, 8080)
	if len(config) == 0 {
		t.Error("empty config")
	}
	if !contains(config, "least_conn") {
		t.Error("missing least_conn")
	}
	if !contains(config, "proxy_pass http://flowlink_relays") {
		t.Error("missing proxy_pass")
	}
}

func TestGetActiveRelayAddresses(t *testing.T) {
	servers := []*ManagedServer{
		{ServerID: 1, Role: "relay", Status: "active"},
		{ServerID: 2, Role: "relay", Status: "draining"},
		{ServerID: 3, Role: "relay", Status: "active"},
	}
	addrs := GetActiveRelayAddresses(servers)
	if len(addrs) != 2 {
		t.Errorf("expected 2 active, got %d", len(addrs))
	}
}

func TestRegistry_Persistence(t *testing.T) {
	dir := t.TempDir()
	reg, _ := NewScaleRegistry(dir)
	reg.AddServer(&ManagedServer{ServerID: 42, Role: "relay", Status: "active"})
	reg.Save()

	reg2, _ := NewScaleRegistry(dir)
	if reg2.ActiveCount() != 1 {
		t.Errorf("expected 1 after reload, got %d", reg2.ActiveCount())
	}
	if reg2.GetServer(42) == nil {
		t.Error("server not persisted")
	}
}

func TestRegistry_EmptyDir(t *testing.T) {
	dir := t.TempDir()
	reg, _ := NewScaleRegistry(dir)
	if reg.ActiveCount() != 0 {
		t.Error("empty registry should have 0")
	}
}

func TestRegistry_FilePath(t *testing.T) {
	dir := t.TempDir()
	reg, _ := NewScaleRegistry(dir)
	expected := filepath.Join(dir, "autoscale.json")
	if reg.filePath() != expected {
		t.Errorf("expected %s", expected)
	}
}

func TestDefaultAutoscaleConfig(t *testing.T) {
	cfg := DefaultAutoscaleConfig()
	if cfg.MinServers != 1 || cfg.MaxServers != 5 {
		t.Error("unexpected defaults")
	}
	if cfg.ScaleUpThreshold != 10 || cfg.ScaleDownThreshold != 3 {
		t.Error("unexpected thresholds")
	}
}

func TestAutoscaler_EvaluateWithServers(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()
	a.registry.AddServer(&ManagedServer{ServerID: 1, Role: "relay", Status: "active"})

	dec, _ := a.Evaluate(5)
	if dec.Action != "none" {
		t.Errorf("5/1 → none, got %s", dec.Action)
	}

	dec, _ = a.Evaluate(15)
	if dec.Action != "scale_up" {
		t.Errorf("15/1 → scale_up, got %s", dec.Action)
	}
}

func TestAutoscaler_GetStatus(t *testing.T) {
	a, close := testAutoscaler(t)
	defer close()
	status := a.GetStatus()
	if status.ActiveServers != 0 || status.InCooldown {
		t.Error("unexpected initial status")
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && searchString(s, substr)
}

func searchString(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

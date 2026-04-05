// Package relay — tests for audit logger
package relay

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestNewAuditLogger(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })
	if al == nil {
		t.Fatal("expected non-nil audit logger")
	}
}

func TestNewAuditLogger_EmptyDir(t *testing.T) {
	// Should use default directory
	al, err := NewAuditLogger("")
	if err != nil {
		t.Fatalf("NewAuditLogger with empty dir failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })
	if al == nil {
		t.Fatal("expected non-nil audit logger")
	}
}

func TestAuditLogger_Log(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	entry := AuditEntry{
		ID:         "test-1",
		Timestamp:  time.Now(),
		AgentID:    "agent-1",
		ClientID:   "client-1",
		Action:     "exec",
		Command:    "ls -la",
		RiskLevel:  "low",
		Result:     "success",
		DurationMs: 100,
		ClientIP:   "127.0.0.1",
	}

	err = al.Log(entry)
	if err != nil {
		t.Fatalf("Log failed: %v", err)
	}
}

func TestAuditLogger_Query(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log some entries
	for i := 0; i < 5; i++ {
		entry := AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: time.Now().Add(-time.Duration(i) * time.Hour),
			AgentID:   "agent-1",
			Action:    "exec",
			RiskLevel: "low",
			Result:    "success",
		}
		al.Log(entry)
	}

	// Query all
	entries, err := al.Query(AuditQuery{})
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(entries) != 5 {
		t.Errorf("expected 5 entries, got %d", len(entries))
	}

	// Query by agent
	entries, err = al.Query(AuditQuery{AgentID: "agent-1"})
	if err != nil {
		t.Fatalf("Query with AgentID failed: %v", err)
	}
	if len(entries) != 5 {
		t.Errorf("expected 5 entries for agent-1, got %d", len(entries))
	}

	// Query with limit
	entries, err = al.Query(AuditQuery{Limit: 2})
	if err != nil {
		t.Fatalf("Query with Limit failed: %v", err)
	}
	if len(entries) != 2 {
		t.Errorf("expected 2 entries with limit, got %d", len(entries))
	}
}

func TestAuditLogger_QueryWithDateRange(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	now := time.Now()

	// Log entries at different times
	for i := 0; i < 3; i++ {
		entry := AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: now.Add(-time.Duration(i*24) * time.Hour),
			Action:    "exec",
		}
		al.Log(entry)
	}

	// Query last 24 hours
	from := now.Add(-25 * time.Hour)
	to := now.Add(1 * time.Hour)
	entries, err := al.Query(AuditQuery{From: &from, To: &to})
	if err != nil {
		t.Fatalf("Query with date range failed: %v", err)
	}
	if len(entries) < 1 {
		t.Error("expected at least 1 entry in date range")
	}
}

func TestAuditLogger_Stats(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log some entries
	for i := 0; i < 3; i++ {
		entry := AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: time.Now(),
			Action:    "exec",
			RiskLevel: "low",
			Result:    "success",
		}
		al.Log(entry)
	}

	stats, err := al.Stats()
	if err != nil {
		t.Fatalf("Stats failed: %v", err)
	}

	if stats.TotalEntries != 3 {
		t.Errorf("expected TotalEntries 3, got %d", stats.TotalEntries)
	}
}

func TestAuditLogger_Recent(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log some entries
	for i := 0; i < 5; i++ {
		entry := AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: time.Now(),
			Action:    "exec",
		}
		al.Log(entry)
	}

	recent, err := al.Recent(3)
	if err != nil {
		t.Fatalf("Recent failed: %v", err)
	}

	if len(recent) != 3 {
		t.Errorf("expected 3 recent entries, got %d", len(recent))
	}
}

func TestAuditLogger_Export_JSON(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log entry
	entry := AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		Action:    "exec",
	}
	al.Log(entry)

	data, err := al.Export("json", AuditQuery{})
	if err != nil {
		t.Fatalf("Export failed: %v", err)
	}

	// Verify it's valid JSON
	var entries []AuditEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		t.Errorf("exported data is not valid JSON: %v", err)
	}
}

func TestAuditLogger_Export_CSV(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log entry
	entry := AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		Action:    "exec",
	}
	al.Log(entry)

	data, err := al.Export("csv", AuditQuery{})
	if err != nil {
		t.Fatalf("Export CSV failed: %v", err)
	}

	if len(data) == 0 {
		t.Error("expected non-empty CSV export")
	}
}

func TestAuditLogger_IsWritable(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	if !al.IsWritable() {
		t.Error("expected IsWritable to be true")
	}
}

func TestAuditLogger_Prune(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Create old audit file
	oldDate := time.Now().Add(-100 * 24 * time.Hour).Format("2006-01-02")
	oldFile := filepath.Join(dir, "audit-"+oldDate+".jsonl")
	os.WriteFile(oldFile, []byte("{}\n"), 0644)

	// Run prune
	err = al.Prune(30) // delete files older than 30 days
	if err != nil {
		t.Fatalf("Prune failed: %v", err)
	}

	// Old file should be deleted
	if _, err := os.Stat(oldFile); !os.IsNotExist(err) {
		t.Error("expected old audit file to be deleted")
	}
}

func TestAuditLogger_Rotate(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log entry
	entry := AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		Action:    "exec",
	}
	al.Log(entry)

	// Force rotation by changing date
	al.mu.Lock()
	al.currentDate = "2000-01-01" // Old date
	al.mu.Unlock()

	// Log another entry (should trigger rotation)
	entry2 := AuditEntry{
		ID:        "test-2",
		Timestamp: time.Now(),
		Action:    "read",
	}
	al.Log(entry2)

	// Both files should exist
	today := time.Now().Format("2006-01-02")
	if _, err := os.Stat(filepath.Join(dir, "audit-"+today+".jsonl")); os.IsNotExist(err) {
		t.Error("expected today's audit file")
	}
}

func TestAuditEntry_HMAC(t *testing.T) {
	dir := t.TempDir()
	keyPath := filepath.Join(dir, "hmac.key")

	al, err := NewAuditLoggerWithHMAC(dir, keyPath)
	if err != nil {
		t.Fatalf("NewAuditLoggerWithHMAC failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	entry := AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		Action:    "exec",
		Command:   "ls -la",
	}

	// Log entry (should be signed)
	err = al.Log(entry)
	if err != nil {
		t.Fatalf("Log failed: %v", err)
	}

	// Verify HMAC was added
	entries, _ := al.Query(AuditQuery{})
	if len(entries) > 0 && entries[0].HMAC == "" {
		t.Error("expected HMAC to be set")
	}
}

func TestAuditLogger_Close(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log entry
	err = al.Log(AuditEntry{ID: "test", Action: "exec"})
	if err != nil {
		t.Fatalf("Log failed: %v", err)
	}

	// Close
	err = al.Close()
	if err != nil {
		t.Fatalf("Close failed: %v", err)
	}
}

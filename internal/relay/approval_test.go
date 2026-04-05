// Package relay — tests for approval queue
package relay

import (
	"log/slog"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestNewApprovalQueue(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	if aq == nil {
		t.Fatal("expected non-nil approval queue")
	}

	if aq.PendingCount() != 0 {
		t.Errorf("expected 0 pending, got %d", aq.PendingCount())
	}
}

func TestApprovalQueue_Add(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	// Subscribe to events
	ch := eventBus.Subscribe()

	req := aq.Add("agent-1", "rm -rf /", "high", "hard_ask")

	if req == nil {
		t.Fatal("expected non-nil request")
	}
	if req.ID == "" {
		t.Error("expected request ID")
	}
	if req.AgentID != "agent-1" {
		t.Errorf("expected agent-1, got %s", req.AgentID)
	}
	if req.Command != "rm -rf /" {
		t.Errorf("expected 'rm -rf /', got %s", req.Command)
	}
	if req.RiskLevel != "high" {
		t.Errorf("expected high risk, got %s", req.RiskLevel)
	}
	if req.Status != ApprovalPending {
		t.Errorf("expected pending status, got %s", req.Status)
	}

	// Check event was published
	select {
	case event := <-ch:
		if event.Type != EventApprovalRequired {
			t.Errorf("expected %s event, got %s", EventApprovalRequired, event.Type)
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for event")
	}

	// Check pending count
	if aq.PendingCount() != 1 {
		t.Errorf("expected 1 pending, got %d", aq.PendingCount())
	}
}

func TestApprovalQueue_Approve(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	// Subscribe to events
	ch := eventBus.Subscribe()

	req := aq.Add("agent-1", "ls -la", "low", "soft_ask")

	// Consume the add event
	<-ch

	// Approve
	approved, err := aq.Approve(req.ID, "admin", "looks good")
	if err != nil {
		t.Fatalf("approve failed: %v", err)
	}

	if approved.Status != ApprovalApproved {
		t.Errorf("expected approved status, got %s", approved.Status)
	}
	if approved.ResolvedBy != "admin" {
		t.Errorf("expected resolved by admin, got %s", approved.ResolvedBy)
	}
	if approved.Comment != "looks good" {
		t.Errorf("expected comment, got %s", approved.Comment)
	}
	if approved.ResolvedAt == nil {
		t.Error("expected resolved_at to be set")
	}

	// Check event
	select {
	case event := <-ch:
		if event.Type != EventApprovalGranted {
			t.Errorf("expected %s event, got %s", EventApprovalGranted, event.Type)
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for event")
	}

	// Check pending count decreased
	if aq.PendingCount() != 0 {
		t.Errorf("expected 0 pending, got %d", aq.PendingCount())
	}
}

func TestApprovalQueue_Reject(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	// Subscribe to events
	ch := eventBus.Subscribe()

	req := aq.Add("agent-1", "rm -rf /", "critical", "hard_ask")

	// Consume the add event
	<-ch

	// Reject
	rejected, err := aq.Reject(req.ID, "admin", "too dangerous")
	if err != nil {
		t.Fatalf("reject failed: %v", err)
	}

	if rejected.Status != ApprovalRejected {
		t.Errorf("expected rejected status, got %s", rejected.Status)
	}
	if rejected.ResolvedBy != "admin" {
		t.Errorf("expected resolved by admin, got %s", rejected.ResolvedBy)
	}

	// Check event
	select {
	case event := <-ch:
		if event.Type != EventApprovalRejected {
			t.Errorf("expected %s event, got %s", EventApprovalRejected, event.Type)
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for event")
	}
}

func TestApprovalQueue_List(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	// Add multiple requests
	aq.Add("agent-1", "cmd1", "low", "soft_ask")
	aq.Add("agent-1", "cmd2", "medium", "soft_ask")
	aq.Add("agent-2", "cmd3", "high", "hard_ask")

	// List all
	all := aq.List("", "", 0)
	if len(all) != 3 {
		t.Errorf("expected 3 requests, got %d", len(all))
	}

	// List by agent
	agent1Reqs := aq.List("agent-1", "", 0)
	if len(agent1Reqs) != 2 {
		t.Errorf("expected 2 requests for agent-1, got %d", len(agent1Reqs))
	}

	// List by status
	pending := aq.List("", ApprovalPending, 0)
	if len(pending) != 3 {
		t.Errorf("expected 3 pending, got %d", len(pending))
	}

	// Approve one
	aq.Approve(all[0].ID, "admin", "")

	// List pending again
	pending = aq.List("", ApprovalPending, 0)
	if len(pending) != 2 {
		t.Errorf("expected 2 pending after approve, got %d", len(pending))
	}

	// List approved
	approved := aq.List("", ApprovalApproved, 0)
	if len(approved) != 1 {
		t.Errorf("expected 1 approved, got %d", len(approved))
	}

	// Test limit
	limited := aq.List("", "", 2)
	if len(limited) != 2 {
		t.Errorf("expected 2 limited, got %d", len(limited))
	}
}

func TestApprovalQueue_Get(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	req := aq.Add("agent-1", "cmd", "low", "soft_ask")

	// Get existing
	got, ok := aq.Get(req.ID)
	if !ok {
		t.Fatal("expected to find request")
	}
	if got.ID != req.ID {
		t.Errorf("expected ID %s, got %s", req.ID, got.ID)
	}

	// Get non-existing
	_, ok = aq.Get("non-existing")
	if ok {
		t.Error("expected not to find non-existing request")
	}
}

func TestApprovalQueue_ExpireOld(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	// Add request
	req := aq.Add("agent-1", "cmd", "low", "soft_ask")

	// Manually set old timestamp
	aq.mu.Lock()
	req.CreatedAt = time.Now().Add(-2 * time.Hour)
	aq.mu.Unlock()

	// Expire requests older than 1 hour
	expired := aq.ExpireOld(time.Hour)
	if expired != 1 {
		t.Errorf("expected 1 expired, got %d", expired)
	}

	// Check status
	got, _ := aq.Get(req.ID)
	if got.Status != ApprovalExpired {
		t.Errorf("expected expired status, got %s", got.Status)
	}

	// No more to expire
	expired = aq.ExpireOld(time.Hour)
	if expired != 0 {
		t.Errorf("expected 0 expired, got %d", expired)
	}
}

func TestApprovalQueue_ApproveNotFound(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	_, err := aq.Approve("non-existing", "admin", "")
	if err == nil {
		t.Error("expected error for non-existing request")
	}
}

func TestApprovalQueue_ApproveAlreadyResolved(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	req := aq.Add("agent-1", "cmd", "low", "soft_ask")

	// Approve once
	_, err := aq.Approve(req.ID, "admin", "")
	if err != nil {
		t.Fatalf("first approve failed: %v", err)
	}

	// Try to approve again
	_, err = aq.Approve(req.ID, "admin2", "")
	if err == nil {
		t.Error("expected error for already approved request")
	}
}

func TestApprovalQueue_RejectNotFound(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	_, err := aq.Reject("non-existing", "admin", "")
	if err == nil {
		t.Error("expected error for non-existing request")
	}
}

func TestApprovalQueue_Persistence(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	storePath := filepath.Join(dir, "approvals.jsonl")

	// Create queue and add request
	aq := NewApprovalQueue(eventBus, logger, dir)
	req := aq.Add("agent-1", "cmd", "low", "soft_ask")

	// Check file was created
	if _, err := os.Stat(storePath); os.IsNotExist(err) {
		t.Error("expected approvals file to be created")
	}

	// Create new queue from same directory (simulates restart)
	eventBus2 := NewEventBus(logger)
	defer eventBus2.Close()
	aq2 := NewApprovalQueue(eventBus2, logger, dir)

	// Check request was loaded
	got, ok := aq2.Get(req.ID)
	if !ok {
		t.Fatal("expected request to be loaded from persistence")
	}
	if got.AgentID != "agent-1" {
		t.Errorf("expected agent-1, got %s", got.AgentID)
	}
}

func TestApprovalQueue_NilEventBus(t *testing.T) {
	logger := slog.Default()
	dir := t.TempDir()

	// Create queue with nil event bus
	aq := NewApprovalQueue(nil, logger, dir)

	// Should still work (events won't be published)
	req := aq.Add("agent-1", "cmd", "low", "soft_ask")
	if req == nil {
		t.Error("expected request to be created even with nil event bus")
	}

	// Approve should also work
	_, err := aq.Approve(req.ID, "admin", "")
	if err != nil {
		t.Errorf("approve failed: %v", err)
	}
}

func TestApprovalQueue_SortByCreatedAt(t *testing.T) {
	logger := slog.Default()
	eventBus := NewEventBus(logger)
	defer eventBus.Close()

	dir := t.TempDir()
	aq := NewApprovalQueue(eventBus, logger, dir)

	// Add requests with delay to ensure different timestamps
	req1 := aq.Add("agent-1", "cmd1", "low", "soft_ask")
	time.Sleep(10 * time.Millisecond)
	_ = aq.Add("agent-2", "cmd2", "low", "soft_ask") // req2 middle
	time.Sleep(10 * time.Millisecond)
	req3 := aq.Add("agent-3", "cmd3", "low", "soft_ask")

	// List should be sorted by created_at desc (newest first)
	list := aq.List("", "", 0)
	if len(list) != 3 {
		t.Fatalf("expected 3 requests, got %d", len(list))
	}

	// First should be req3 (newest)
	if list[0].ID != req3.ID {
		t.Errorf("expected first to be req3, got %s", list[0].ID)
	}
	// Last should be req1 (oldest)
	if list[2].ID != req1.ID {
		t.Errorf("expected last to be req1, got %s", list[2].ID)
	}
}

func TestGenerateApprovalID(t *testing.T) {
	id1 := generateApprovalID("agent-123")
	id2 := generateApprovalID("agent-123")

	// IDs should be different (due to timestamp)
	if id1 == id2 {
		t.Error("expected different IDs")
	}

	// Should start with "apr_"
	if len(id1) < 4 || id1[:4] != "apr_" {
		t.Errorf("expected ID to start with 'apr_', got %s", id1)
	}
}

func TestSortRequests(t *testing.T) {
	now := time.Now()
	reqs := []*ApprovalRequest{
		{ID: "oldest", CreatedAt: now.Add(-2 * time.Hour)},
		{ID: "middle", CreatedAt: now.Add(-1 * time.Hour)},
		{ID: "newest", CreatedAt: now},
	}

	sortRequests(reqs)

	// Should be sorted newest first
	if reqs[0].ID != "newest" {
		t.Errorf("expected newest first, got %s", reqs[0].ID)
	}
	if reqs[2].ID != "oldest" {
		t.Errorf("expected oldest last, got %s", reqs[2].ID)
	}
}

func TestSplitLines(t *testing.T) {
	tests := []struct {
		input    string
		expected int
	}{
		{"line1\nline2\nline3", 3},
		{"single", 1},
		{"", 0},
		{"line1\r\nline2", 2},
	}

	for _, tt := range tests {
		lines := splitLines(tt.input)
		if len(lines) != tt.expected {
			t.Errorf("input %q: expected %d lines, got %d", tt.input, tt.expected, len(lines))
		}
	}
}

func TestTrimSpace(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"  hello  ", "hello"},
		{"\t\nhello\n\t", "hello"},
		{"hello", "hello"},
		{"   ", ""},
	}

	for _, tt := range tests {
		result := trimSpace(tt.input)
		if result != tt.expected {
			t.Errorf("input %q: expected %q, got %q", tt.input, tt.expected, result)
		}
	}
}

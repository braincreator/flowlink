// Package relay — approval queue for agent command approval workflow.
package relay

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// ApprovalStatus represents the state of an approval request.
type ApprovalStatus string

const (
	ApprovalPending  ApprovalStatus = "pending"
	ApprovalApproved ApprovalStatus = "approved"
	ApprovalRejected ApprovalStatus = "rejected"
	ApprovalExpired  ApprovalStatus = "expired"
)

// ApprovalRequest represents a pending approval from an agent.
type ApprovalRequest struct {
	ID          string          `json:"id"`
	AgentID     string          `json:"agent_id"`
	Command     string          `json:"command"`
	RiskLevel   string          `json:"risk_level"`   // low, medium, high, critical
	ApprovalMode string         `json:"approval_mode"` // auto, soft_ask, hard_ask
	Status      ApprovalStatus  `json:"status"`
	CreatedAt   time.Time       `json:"created_at"`
	ResolvedAt  *time.Time      `json:"resolved_at,omitempty"`
	ResolvedBy  string          `json:"resolved_by,omitempty"`
	Comment     string          `json:"comment,omitempty"`
}

// ApprovalQueue manages pending approval requests.
type ApprovalQueue struct {
	mu        sync.RWMutex
	requests  map[string]*ApprovalRequest
	eventBus  *EventBus
	logger    *slog.Logger
	storePath string
}

// NewApprovalQueue creates a new approval queue.
func NewApprovalQueue(eventBus *EventBus, logger *slog.Logger, dataDir string) *ApprovalQueue {
	aq := &ApprovalQueue{
		requests:  make(map[string]*ApprovalRequest),
		eventBus:  eventBus,
		logger:    logger,
		storePath: filepath.Join(dataDir, "approvals.jsonl"),
	}
	aq.load()
	return aq
}

// Add creates a new approval request and notifies via SSE.
func (aq *ApprovalQueue) Add(agentID, command, riskLevel, approvalMode string) *ApprovalRequest {
	aq.mu.Lock()
	defer aq.mu.Unlock()

	id := generateApprovalID(agentID)
	req := &ApprovalRequest{
		ID:           id,
		AgentID:      agentID,
		Command:      command,
		RiskLevel:    riskLevel,
		ApprovalMode: approvalMode,
		Status:       ApprovalPending,
		CreatedAt:    time.Now(),
	}
	aq.requests[id] = req
	aq.persist(req)

	// Notify SSE subscribers
	if aq.eventBus != nil {
		aq.eventBus.Publish(Event{
			Type: EventApprovalRequired,
			Data: map[string]any{"id": req.ID, "agent_id": req.AgentID, "command": req.Command, "risk_level": req.RiskLevel},
		})
	}

	aq.logger.Info("approval request queued", "id", id, "agent", agentID, "risk", riskLevel)
	return req
}

// Approve approves a pending request and returns it.
func (aq *ApprovalQueue) Approve(id, approvedBy, comment string) (*ApprovalRequest, error) {
	aq.mu.Lock()
	defer aq.mu.Unlock()

	req, ok := aq.requests[id]
	if !ok {
		return nil, fmt.Errorf("approval %s not found", id)
	}
	if req.Status != ApprovalPending {
		return nil, fmt.Errorf("approval %s is %s (not pending)", id, req.Status)
	}

	now := time.Now()
	req.Status = ApprovalApproved
	req.ResolvedAt = &now
	req.ResolvedBy = approvedBy
	req.Comment = comment
	aq.persist(req)

	if aq.eventBus != nil {
		aq.eventBus.Publish(Event{
			Type: EventApprovalGranted,
			Data: map[string]any{"id": req.ID, "status": "approved"},
		})
	}

	aq.logger.Info("approval approved", "id", id, "by", approvedBy)
	return req, nil
}

// Reject rejects a pending request.
func (aq *ApprovalQueue) Reject(id, rejectedBy, comment string) (*ApprovalRequest, error) {
	aq.mu.Lock()
	defer aq.mu.Unlock()

	req, ok := aq.requests[id]
	if !ok {
		return nil, fmt.Errorf("approval %s not found", id)
	}
	if req.Status != ApprovalPending {
		return nil, fmt.Errorf("approval %s is %s (not pending)", id, req.Status)
	}

	now := time.Now()
	req.Status = ApprovalRejected
	req.ResolvedAt = &now
	req.ResolvedBy = rejectedBy
	req.Comment = comment
	aq.persist(req)

	if aq.eventBus != nil {
		aq.eventBus.Publish(Event{
			Type: EventApprovalRejected,
			Data: map[string]any{"id": req.ID, "status": "rejected"},
		})
	}

	aq.logger.Info("approval rejected", "id", id, "by", rejectedBy)
	return req, nil
}

// List returns all requests, optionally filtered by status and agent.
func (aq *ApprovalQueue) List(agentID string, status ApprovalStatus, limit int) []*ApprovalRequest {
	aq.mu.RLock()
	defer aq.mu.RUnlock()

	var result []*ApprovalRequest
	for _, req := range aq.requests {
		if agentID != "" && req.AgentID != agentID {
			continue
		}
		if status != "" && req.Status != status {
			continue
		}
		result = append(result, req)
	}

	// Sort by created_at desc
	sortRequests(result)

	if limit > 0 && len(result) > limit {
		result = result[:limit]
	}
	return result
}

// Get returns a single request by ID.
func (aq *ApprovalQueue) Get(id string) (*ApprovalRequest, bool) {
	aq.mu.RLock()
	defer aq.mu.RUnlock()
	req, ok := aq.requests[id]
	return req, ok
}

// PendingCount returns number of pending requests.
func (aq *ApprovalQueue) PendingCount() int {
	aq.mu.RLock()
	defer aq.mu.RUnlock()
	count := 0
	for _, req := range aq.requests {
		if req.Status == ApprovalPending {
			count++
		}
	}
	return count
}

// ExpireOld marks pending requests older than maxAge as expired.
func (aq *ApprovalQueue) ExpireOld(maxAge time.Duration) int {
	aq.mu.Lock()
	defer aq.mu.Unlock()

	expired := 0
	now := time.Now()
	for _, req := range aq.requests {
		if req.Status == ApprovalPending && now.Sub(req.CreatedAt) > maxAge {
			req.Status = ApprovalExpired
			req.ResolvedAt = &now
			req.ResolvedBy = "system"
			req.Comment = "auto-expired"
			aq.persist(req)
			expired++
		}
	}
	if expired > 0 {
		aq.logger.Info("expired old approvals", "count", expired)
	}
	return expired
}

// persist appends request to JSONL file.
func (aq *ApprovalQueue) persist(req *ApprovalRequest) {
	if aq.storePath == "" {
		return
	}
	data, err := json.Marshal(req)
	if err != nil {
		aq.logger.Error("marshal approval", "err", err)
		return
	}
	f, err := os.OpenFile(aq.storePath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0600)
	if err != nil {
		aq.logger.Error("open approval store", "err", err)
		return
	}
	f.Write(data)
	f.Write([]byte("\n"))
	f.Close()
}

// load restores approvals from JSONL file.
func (aq *ApprovalQueue) load() {
	if aq.storePath == "" {
		return
	}
	data, err := os.ReadFile(aq.storePath)
	if err != nil {
		return // no file yet
	}
	loaded := 0
	for _, line := range splitLines(string(data)) {
		line = trimSpace(line)
		if line == "" {
			continue
		}
		var req ApprovalRequest
		if err := json.Unmarshal([]byte(line), &req); err != nil {
			continue
		}
		aq.requests[req.ID] = &req
		loaded++
	}
	if loaded > 0 {
		aq.logger.Info("loaded approvals from store", "count", loaded)
	}
}

func generateApprovalID(agentID string) string {
	return fmt.Sprintf("apr_%s_%d", agentID[:min(8, len(agentID))], time.Now().UnixNano())
}

func sortRequests(reqs []*ApprovalRequest) {
	for i := 0; i < len(reqs)-1; i++ {
		for j := i + 1; j < len(reqs); j++ {
			if reqs[j].CreatedAt.After(reqs[i].CreatedAt) {
				reqs[i], reqs[j] = reqs[j], reqs[i]
			}
		}
	}
}

func splitLines(s string) []string {
	var lines []string
	start := 0
	for i := 0; i < len(s); i++ {
		if s[i] == '\n' {
			lines = append(lines, s[start:i])
			start = i + 1
		}
	}
	if start < len(s) {
		lines = append(lines, s[start:])
	}
	return lines
}

func trimSpace(s string) string {
	i, j := 0, len(s)
	for i < j && (s[i] == ' ' || s[i] == '\t' || s[i] == '\r' || s[i] == '\n') {
		i++
	}
	for j > i && (s[j-1] == ' ' || s[j-1] == '\t' || s[j-1] == '\r' || s[j-1] == '\n') {
		j--
	}
	return s[i:j]
}

// Package relay — tests for rate limiter
package relay

import (
	"log/slog"
	"sync"
	"testing"
	"time"
)

func TestNewRateLimiter(t *testing.T) {
	logger := slog.Default()

	// With valid limits
	rl := NewRateLimiter(30, 200, logger)
	if rl == nil {
		t.Fatal("expected non-nil rate limiter")
	}
	if rl.maxPerMin != 30 {
		t.Errorf("expected maxPerMin 30, got %d", rl.maxPerMin)
	}
	if rl.maxPerHour != 200 {
		t.Errorf("expected maxPerHour 200, got %d", rl.maxPerHour)
	}

	// With zero limits (should use defaults)
	rl2 := NewRateLimiter(0, 0, logger)
	if rl2.maxPerMin != 30 {
		t.Errorf("expected default maxPerMin 30, got %d", rl2.maxPerMin)
	}
	if rl2.maxPerHour != 200 {
		t.Errorf("expected default maxPerHour 200, got %d", rl2.maxPerHour)
	}

	// With negative limits (should use defaults)
	rl3 := NewRateLimiter(-1, -1, logger)
	if rl3.maxPerMin != 30 {
		t.Errorf("expected default maxPerMin 30, got %d", rl3.maxPerMin)
	}
}

func TestRateLimiter_Check(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(5, 100, logger) // 5 per minute, 100 per hour

	// First 5 requests should succeed
	for i := 0; i < 5; i++ {
		allowed, _ := rl.Check("client-1")
		if !allowed {
			t.Errorf("request %d should be allowed", i+1)
		}
	}

	// 6th request should fail
	allowed, retryAfter := rl.Check("client-1")
	if allowed {
		t.Error("6th request should be denied")
	}
	if retryAfter <= 0 {
		t.Error("expected positive retryAfter")
	}

	// Different client should still be allowed
	allowed, _ = rl.Check("client-2")
	if !allowed {
		t.Error("different client should be allowed")
	}
}

func TestRateLimiter_HourLimit(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(100, 3, logger) // 100 per minute, 3 per hour

	// 3 requests should succeed
	for i := 0; i < 3; i++ {
		allowed, _ := rl.Check("client-1")
		if !allowed {
			t.Errorf("request %d should be allowed", i+1)
		}
	}

	// 4th request should fail (hour limit)
	allowed, _ := rl.Check("client-1")
	if allowed {
		t.Error("4th request should be denied (hour limit)")
	}
}

func TestRateLimiter_SetClientLimits(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(5, 100, logger)

	// Set custom limits for client
	rl.SetClientLimits("vip-client", 50, 500)

	// VIP client should have higher limits
	for i := 0; i < 10; i++ {
		allowed, _ := rl.Check("vip-client")
		if !allowed {
			t.Errorf("VIP request %d should be allowed", i+1)
		}
	}

	// Regular client should still have default limits
	for i := 0; i < 5; i++ {
		rl.Check("regular-client")
	}
	allowed, _ := rl.Check("regular-client")
	if allowed {
		t.Error("regular client 6th request should be denied")
	}
}

func TestRateLimiter_ResetClientLimits(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(5, 100, logger)

	// Set custom limits
	rl.SetClientLimits("client-1", 50, 500)

	// Reset to defaults
	rl.ResetClientLimits("client-1")

	// Should now have default limits
	for i := 0; i < 5; i++ {
		rl.Check("client-1")
	}
	allowed, _ := rl.Check("client-1")
	if allowed {
		t.Error("6th request should be denied after reset")
	}
}

func TestRateLimiter_GetClientStats(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(10, 100, logger)

	// Make some requests
	for i := 0; i < 5; i++ {
		rl.Check("client-1")
	}

	stats := rl.GetClientStats("client-1")

	if stats.ClientID != "client-1" {
		t.Errorf("expected client-1, got %s", stats.ClientID)
	}
	if stats.RequestsPerMin != 10 {
		t.Errorf("expected RequestsPerMin 10, got %d", stats.RequestsPerMin)
	}
	if stats.UsedMin != 5 {
		t.Errorf("expected UsedMin 5, got %d", stats.UsedMin)
	}
	if stats.Status != "ok" {
		t.Errorf("expected status ok, got %s", stats.Status)
	}
}

func TestRateLimiter_GetClientStats_Warning(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(10, 100, logger)

	// Make 8 requests (80% of 10)
	for i := 0; i < 8; i++ {
		rl.Check("client-1")
	}

	stats := rl.GetClientStats("client-1")
	if stats.Status != "warning" {
		t.Errorf("expected status warning at 80%%, got %s", stats.Status)
	}
}

func TestRateLimiter_GetClientStats_Exceeded(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(5, 100, logger)

	// Make 5 requests (limit)
	for i := 0; i < 5; i++ {
		rl.Check("client-1")
	}

	stats := rl.GetClientStats("client-1")
	if stats.Status != "exceeded" {
		t.Errorf("expected status exceeded, got %s", stats.Status)
	}
}

func TestRateLimiter_GetAllClientStats(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(10, 100, logger)

	// Make requests for multiple clients
	rl.Check("client-1")
	rl.Check("client-2")
	rl.Check("client-3")

	stats := rl.GetAllClientStats()
	if len(stats) != 3 {
		t.Errorf("expected 3 client stats, got %d", len(stats))
	}
}

func TestRateLimiter_GetStats(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(10, 100, logger)

	// Make some requests
	for i := 0; i < 5; i++ {
		rl.Check("client-1")
	}
	rl.Check("client-2")

	stats := rl.GetStats()

	if stats.TotalRequests != 6 {
		t.Errorf("expected TotalRequests 6, got %d", stats.TotalRequests)
	}
	if stats.DefaultMaxPerMin != 10 {
		t.Errorf("expected DefaultMaxPerMin 10, got %d", stats.DefaultMaxPerMin)
	}
	if stats.DefaultMaxPerHour != 100 {
		t.Errorf("expected DefaultMaxPerHour 100, got %d", stats.DefaultMaxPerHour)
	}
}

func TestRateLimiter_ResetStats(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(10, 100, logger)

	// Make some requests
	for i := 0; i < 5; i++ {
		rl.Check("client-1")
	}

	// Reset
	rl.ResetStats()

	stats := rl.GetStats()
	if stats.TotalRequests != 0 {
		t.Errorf("expected TotalRequests 0 after reset, got %d", stats.TotalRequests)
	}
}

func TestRateLimiter_ResetClientCounters(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(10, 100, logger)

	// Make some requests
	for i := 0; i < 5; i++ {
		rl.Check("client-1")
	}

	// Reset counters for client
	rl.ResetClientCounters("client-1")

	stats := rl.GetClientStats("client-1")
	if stats.UsedMin != 0 {
		t.Errorf("expected UsedMin 0 after reset, got %d", stats.UsedMin)
	}
}

func TestRateLimiter_Concurrent(t *testing.T) {
	logger := slog.Default()
	rl := NewRateLimiter(100, 1000, logger)

	var wg sync.WaitGroup
	errors := make(chan error, 100)

	// Concurrent requests from multiple clients
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func(clientNum int) {
			defer wg.Done()
			clientID := string(rune('A' + clientNum))
			for j := 0; j < 10; j++ {
				rl.Check(clientID)
			}
		}(i)
	}

	wg.Wait()
	close(errors)

	for err := range errors {
		t.Error(err)
	}

	stats := rl.GetStats()
	if stats.TotalRequests != 100 {
		t.Errorf("expected 100 total requests, got %d", stats.TotalRequests)
	}
}

func TestFilterTimestamps(t *testing.T) {
	now := time.Now().Unix()
	cutoff := now - 60

	// All recent
	ts := []int64{now - 30, now - 20, now - 10}
	filtered := filterTimestamps(ts, cutoff)
	if len(filtered) != 3 {
		t.Errorf("expected 3, got %d", len(filtered))
	}

	// All old
	ts2 := []int64{now - 120, now - 110, now - 100}
	filtered2 := filterTimestamps(ts2, cutoff)
	if len(filtered2) != 0 {
		t.Errorf("expected 0, got %d", len(filtered2))
	}

	// Mixed
	ts3 := []int64{now - 120, now - 30, now - 10}
	filtered3 := filterTimestamps(ts3, cutoff)
	if len(filtered3) != 2 {
		t.Errorf("expected 2, got %d", len(filtered3))
	}

	// Empty
	filtered4 := filterTimestamps([]int64{}, cutoff)
	if len(filtered4) != 0 {
		t.Errorf("expected 0, got %d", len(filtered4))
	}
}

func TestRateLimiter_NilLogger(t *testing.T) {
	// Should not panic with nil logger
	rl := NewRateLimiter(10, 100, nil)
	if rl == nil {
		t.Error("expected non-nil rate limiter")
	}
}

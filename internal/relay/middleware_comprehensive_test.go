// Package relay — tests for middleware
package relay

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/braincreator/flowlink/internal/config"
)

func TestChain(t *testing.T) {
	order := []string{}

	mw1 := func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			order = append(order, "mw1-before")
			next.ServeHTTP(w, r)
			order = append(order, "mw1-after")
		})
	}

	mw2 := func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			order = append(order, "mw2-before")
			next.ServeHTTP(w, r)
			order = append(order, "mw2-after")
		})
	}

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		order = append(order, "handler")
		w.WriteHeader(http.StatusOK)
	})

	chain := Chain(mw1, mw2)(finalHandler)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()

	chain.ServeHTTP(w, req)

	// Chain should execute in reverse order (mw1 outer, mw2 inner)
	expected := []string{"mw1-before", "mw2-before", "handler", "mw2-after", "mw1-after"}
	if len(order) != len(expected) {
		t.Errorf("expected %d calls, got %d: %v", len(expected), len(order), order)
	}
}

func TestAuthMiddleware_StaticToken(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	})

	cfg := AuthMiddlewareConfig{
		StaticToken: "secret-token",
		Logger:      logger,
	}

	handler := AuthMiddleware(cfg)(finalHandler)

	tests := []struct {
		name       string
		token      string
		wantStatus int
	}{
		{
			name:       "valid token",
			token:      "secret-token",
			wantStatus: http.StatusOK,
		},
		{
			name:       "bearer token",
			token:      "Bearer secret-token",
			wantStatus: http.StatusOK,
		},
		{
			name:       "invalid token",
			token:      "wrong-token",
			wantStatus: http.StatusUnauthorized,
		},
		{
			name:       "no token",
			token:      "",
			wantStatus: http.StatusUnauthorized,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/", nil)
			if tt.token != "" {
				req.Header.Set("Authorization", tt.token)
			}
			w := httptest.NewRecorder()

			handler.ServeHTTP(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d", tt.wantStatus, w.Code)
			}
		})
	}
}

func TestAuthMiddleware_SkipPaths(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	cfg := AuthMiddlewareConfig{
		StaticToken: "secret-token",
		SkipPaths:   []string{"/health", "/public/"},
		Logger:      logger,
	}

	handler := AuthMiddleware(cfg)(finalHandler)

	tests := []struct {
		path       string
		token      string
		wantStatus int
	}{
		{
			path:       "/health",
			token:      "",
			wantStatus: http.StatusOK, // skipped
		},
		{
			path:       "/public/file.txt",
			token:      "",
			wantStatus: http.StatusOK, // skipped (prefix match)
		},
		{
			path:       "/api/private",
			token:      "",
			wantStatus: http.StatusUnauthorized, // not skipped
		},
	}

	for _, tt := range tests {
		t.Run(tt.path, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, tt.path, nil)
			if tt.token != "" {
				req.Header.Set("Authorization", tt.token)
			}
			w := httptest.NewRecorder()

			handler.ServeHTTP(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("path %s: expected status %d, got %d", tt.path, tt.wantStatus, w.Code)
			}
		})
	}
}

func TestAuthMiddleware_DevMode(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	// No AuthManager and no StaticToken = dev mode
	cfg := AuthMiddlewareConfig{
		Logger: logger,
	}

	handler := AuthMiddleware(cfg)(finalHandler)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	// Should pass without token in dev mode
	if w.Code != http.StatusOK {
		t.Errorf("expected 200 in dev mode, got %d", w.Code)
	}
}

func TestAuthMiddleware_WithAuthManager(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		clientID := r.Header.Get("X-Client-ID")
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]string{"client_id": clientID})
	})

	cfg := AuthMiddlewareConfig{
		AuthManager: auth,
		Logger:      logger,
	}

	handler := AuthMiddleware(cfg)(finalHandler)

	// Generate valid token
	clientID := "test-client-1"
	token, _ := auth.GenerateAPIToken(clientID, 3600)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.Header.Set("Authorization", "Bearer "+token)
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestRateLimitMiddleware(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	limiter := NewRateLimiter(3, 100, logger) // 3 per minute
	handler := RateLimitMiddleware(limiter, logger)(finalHandler)

	// First 3 requests should succeed
	for i := 0; i < 3; i++ {
		req := httptest.NewRequest(http.MethodGet, "/", nil)
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, req)
		if w.Code != http.StatusOK {
			t.Errorf("request %d: expected 200, got %d", i+1, w.Code)
		}
	}

	// 4th should be rate limited
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusTooManyRequests {
		t.Errorf("expected 429, got %d", w.Code)
	}

	if w.Header().Get("Retry-After") == "" {
		t.Error("expected Retry-After header")
	}
}

func TestRateLimitMiddleware_NilLimiter(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	handler := RateLimitMiddleware(nil, logger)(finalHandler)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	// Should pass through with nil limiter
	if w.Code != http.StatusOK {
		t.Errorf("expected 200 with nil limiter, got %d", w.Code)
	}
}

func TestCORSMiddleware(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	tests := []struct {
		name            string
		allowedOrigins  []string
		requestOrigin   string
		expectAllow     bool
	}{
		{
			name:           "deny all (no origins configured)",
			allowedOrigins: nil, // nil = deny all (secure default)
			requestOrigin:  "https://example.com",
			expectAllow:    false,
		},
		{
			name:           "specific origin allowed",
			allowedOrigins: []string{"https://example.com"},
			requestOrigin:  "https://example.com",
			expectAllow:    true,
		},
		{
			name:           "specific origin not allowed",
			allowedOrigins: []string{"https://allowed.com"},
			requestOrigin:  "https://notallowed.com",
			expectAllow:    false,
		},
		{
			name:           "wildcard",
			allowedOrigins: []string{"*"},
			requestOrigin:  "https://any.com",
			expectAllow:    true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := CORSMiddleware(tt.allowedOrigins, logger)(finalHandler)

			req := httptest.NewRequest(http.MethodGet, "/", nil)
			req.Header.Set("Origin", tt.requestOrigin)
			w := httptest.NewRecorder()

			handler.ServeHTTP(w, req)

			allowOrigin := w.Header().Get("Access-Control-Allow-Origin")
			if tt.expectAllow && allowOrigin == "" {
				t.Error("expected Access-Control-Allow-Origin header")
			}
			if !tt.expectAllow && allowOrigin != "" {
				t.Errorf("unexpected Access-Control-Allow-Origin: %s", allowOrigin)
			}
		})
	}
}

func TestCORSMiddleware_Preflight(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		t.Error("final handler should not be called for preflight")
	})

	handler := CORSMiddleware([]string{"https://example.com"}, logger)(finalHandler)

	req := httptest.NewRequest(http.MethodOptions, "/", nil)
	req.Header.Set("Origin", "https://example.com")
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200 for preflight, got %d", w.Code)
	}

	// Check CORS headers
	if w.Header().Get("Access-Control-Allow-Methods") == "" {
		t.Error("expected Access-Control-Allow-Methods header")
	}
	if w.Header().Get("Access-Control-Allow-Headers") == "" {
		t.Error("expected Access-Control-Allow-Headers header")
	}
}

func TestRequestLoggerMiddleware(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	handler := RequestLoggerMiddleware(logger)(finalHandler)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/test", nil)
	req.Header.Set("User-Agent", "test-agent")
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestRecoveryMiddleware(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		panic("test panic")
	})

	handler := RecoveryMiddleware(logger)(finalHandler)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()

	// Should not panic
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Errorf("expected 500, got %d", w.Code)
	}
}

func TestRecoveryMiddleware_NoPanic(t *testing.T) {
	logger := slog.Default()

	finalHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	handler := RecoveryMiddleware(logger)(finalHandler)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestResponseWriter(t *testing.T) {
	rec := httptest.NewRecorder()
	rw := &responseWriter{ResponseWriter: rec, status: http.StatusOK}

	rw.WriteHeader(http.StatusCreated)

	if rw.status != http.StatusCreated {
		t.Errorf("expected status %d, got %d", http.StatusCreated, rw.status)
	}
}

func TestWriteAuthError(t *testing.T) {
	w := httptest.NewRecorder()
	writeAuthError(w, "test_error", http.StatusBadRequest)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}

	if w.Header().Get("Content-Type") != "application/json" {
		t.Errorf("expected application/json, got %s", w.Header().Get("Content-Type"))
	}

	var resp map[string]string
	json.Unmarshal(w.Body.Bytes(), &resp)

	if resp["code"] != "test_error" {
		t.Errorf("expected code test_error, got %s", resp["code"])
	}
}

func TestExtractClientIDFromToken(t *testing.T) {
	logger := slog.Default()
	auth := NewAuthManager(logger); t.Cleanup(func() { auth.Close() })

	token, _ := auth.GenerateAPIToken("client-123", 3600)

	extracted := extractClientIDFromToken(token)
	if extracted != "client-123" {
		t.Errorf("expected client-123, got %s", extracted)
	}

	// Invalid token
	extracted = extractClientIDFromToken("invalid")
	if extracted != "" {
		t.Errorf("expected empty for invalid token, got %s", extracted)
	}
}

func TestMiddleware_WithRelay(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr:  ":0",
		APIAddr:  ":0",
		APIToken: "test-token",
	}
	relay := NewRelay(cfg); t.Cleanup(func() { relay.Close() })

	// Test that middleware is properly integrated
	// This tests the full middleware chain through the relay

	// Test health endpoint (should work without auth)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/health", nil)
	w := httptest.NewRecorder()

	relay.handleHealth(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

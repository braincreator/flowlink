package relay

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// TestAuthMiddleware_BearerToken — проверка Bearer token авторизации.
func TestAuthMiddleware_BearerToken(t *testing.T) {
	cfg := AuthMiddlewareConfig{
		StaticToken: "test-secret",
	}

	handler := Chain(AuthMiddleware(cfg))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	}))

	// Valid Bearer token
	req := httptest.NewRequest("GET", "/api/v1/test", nil)
	req.Header.Set("Authorization", "Bearer test-secret")
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}

	// Invalid token
	req2 := httptest.NewRequest("GET", "/api/v1/test", nil)
	req2.Header.Set("Authorization", "Bearer wrong-token")
	rec2 := httptest.NewRecorder()
	handler.ServeHTTP(rec2, req2)
	if rec2.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rec2.Code)
	}

	// No token
	req3 := httptest.NewRequest("GET", "/api/v1/test", nil)
	rec3 := httptest.NewRecorder()
	handler.ServeHTTP(rec3, req3)
	if rec3.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rec3.Code)
	}
}

// TestAuthMiddleware_QueryParamToken — проверка ?token= query param.
func TestAuthMiddleware_QueryParamToken(t *testing.T) {
	cfg := AuthMiddlewareConfig{
		StaticToken: "test-secret",
	}

	handler := Chain(AuthMiddleware(cfg))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	}))

	// Valid token via query param
	req := httptest.NewRequest("GET", "/api/v1/events?token=test-secret", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200 with query token, got %d", rec.Code)
	}

	// Invalid token via query param
	req2 := httptest.NewRequest("GET", "/api/v1/events?token=wrong", nil)
	rec2 := httptest.NewRecorder()
	handler.ServeHTTP(rec2, req2)
	if rec2.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401 with wrong query token, got %d", rec2.Code)
	}
}

// TestAuthMiddleware_SkipPaths — проверка skip paths (prefix matching).
func TestAuthMiddleware_SkipPaths(t *testing.T) {
	cfg := AuthMiddlewareConfig{
		StaticToken: "test-secret",
		SkipPaths:   []string{"/dashboard/", "/health"},
	}

	handler := Chain(AuthMiddleware(cfg))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	}))

	// Dashboard root — skip
	req := httptest.NewRequest("GET", "/dashboard/", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200 for /dashboard/ skip, got %d", rec.Code)
	}

	// Dashboard subpath — skip (prefix match)
	req2 := httptest.NewRequest("GET", "/dashboard/api/overview", nil)
	rec2 := httptest.NewRecorder()
	handler.ServeHTTP(rec2, req2)
	if rec2.Code != http.StatusOK {
		t.Fatalf("expected 200 for /dashboard/api/overview skip, got %d", rec2.Code)
	}

	// Health — skip
	req3 := httptest.NewRequest("GET", "/health", nil)
	rec3 := httptest.NewRecorder()
	handler.ServeHTTP(rec3, req3)
	if rec3.Code != http.StatusOK {
		t.Fatalf("expected 200 for /health skip, got %d", rec3.Code)
	}

	// API — NOT skip
	req4 := httptest.NewRequest("GET", "/api/v1/agents", nil)
	rec4 := httptest.NewRecorder()
	handler.ServeHTTP(rec4, req4)
	if rec4.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401 for /api/v1/agents, got %d", rec4.Code)
	}
}

// TestResponseWriterFlusher — проверка что responseWriter реализует http.Flusher.
func TestResponseWriterFlusher(t *testing.T) {
	cfg := AuthMiddlewareConfig{
		StaticToken: "test-secret",
	}

	handler := Chain(AuthMiddleware(cfg), RequestLoggerMiddleware(nil))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Check that Flusher interface is available
		if _, ok := w.(http.Flusher); !ok {
			t.Fatal("ResponseWriter does not implement http.Flusher")
		}
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/api/v1/events?token=test-secret", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}
}

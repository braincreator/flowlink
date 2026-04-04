package relay

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/braincreator/flowlink/internal/config"
)

type jsonBody struct {
	s string
	i int
}

func (j *jsonBody) Read(p []byte) (int, error) {
	if j.i >= len(j.s) {
		return 0, io.EOF
	}
	n := copy(p, j.s[j.i:])
	j.i += n
	return n, nil
}

func (j *jsonBody) Close() error { return nil }

func TestRateLimitEndpoints(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	relay := NewRelay(cfg)

	t.Run("GET /api/v1/rate-limits returns list", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/v1/rate-limits", nil)
		w := httptest.NewRecorder()
		relay.handleRateLimits(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
		}
	})

	t.Run("GET /api/v1/rate-limits/stats returns stats", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/v1/rate-limits/stats", nil)
		w := httptest.NewRecorder()
		relay.handleRateLimitStats(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
		}
	})

	t.Run("PUT /api/v1/rate-limits/{client_id}", func(t *testing.T) {
		body := `{"max_per_min": 50}`
		req := httptest.NewRequest(http.MethodPut, "/api/v1/rate-limits/test-client", &jsonBody{s: body})
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		relay.handleRateLimitByClient(w, req)

		if w.Code != http.StatusOK && w.Code != http.StatusBadRequest {
			t.Errorf("expected 200 or 400, got %d: %s", w.Code, w.Body.String())
		}
	})
}

// Package dashboard — встроенный web-dashboard для flowlink relay.
// Всё статика встроена через embed.FS, работает без интернета.
package dashboard

import (
	"embed"
	"encoding/json"
	"io/fs"
	"net/http"
	"strings"
)

//go:embed static
var staticFS embed.FS

// ClientInfo — клиент для dashboard.
type ClientInfo struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Email     string `json:"email"`
	Plan      string `json:"plan"`
	APIToken  string `json:"api_token"`
	MaxAgents int    `json:"max_agents"`
	IsActive  bool   `json:"is_active"`
}

// AgentInfo — агент для dashboard.
type AgentInfo struct {
	ID         string   `json:"id"`
	ClientID   string   `json:"client_id"`
	Label      string   `json:"label"`
	Tags       []string `json:"tags"`
	OS         string   `json:"os"`
	Arch       string   `json:"arch"`
	Version    string   `json:"version"`
	IsOnline   bool     `json:"is_online"`
	LastSeenAt string   `json:"last_seen_at"`
}

// AuditStatsInfo — статистика аудита для dashboard.
type AuditStatsInfo struct {
	TotalEntries int              `json:"total_entries"`
	ByAction     map[string]int   `json:"by_action"`
	Last24hCount int              `json:"last_24h_count"`
	Entries      []AuditEntryInfo `json:"entries,omitempty"`
}

// AuditEntryInfo — запись аудита для dashboard.
type AuditEntryInfo struct {
	Timestamp  string `json:"timestamp"`
	AgentID    string `json:"agent_id"`
	Action     string `json:"action"`
	Command    string `json:"command,omitempty"`
	Result     string `json:"result"`
	DurationMs int64  `json:"duration_ms"`
}

// DataProvider — интерфейс для получения данных (реализуется в relay).
type DataProvider interface {
	DashboardAgents() []AgentInfo
	DashboardClients() []ClientInfo
	DashboardAuditStats() *AuditStatsInfo
}

// NewHandler — возвращает http.Handler для dashboard routes.
// token — статический токен для авторизации (Bearer header или ?token= query param).
func NewHandler(dp DataProvider, token string) http.Handler {
	mux := http.NewServeMux()

	// Auth middleware для dashboard
	authMiddleware := func(next http.HandlerFunc) http.HandlerFunc {
		return func(w http.ResponseWriter, r *http.Request) {
			authed := false
			// Bearer token
			if auth := r.Header.Get("Authorization"); strings.HasPrefix(auth, "Bearer ") {
				if strings.TrimPrefix(auth, "Bearer ") == token {
					authed = true
				}
			}
			// ?token= query param
			if !authed && r.URL.Query().Get("token") == token {
				authed = true
			}
			if !authed {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusUnauthorized)
				json.NewEncoder(w).Encode(map[string]string{"code": "401", "error": "токен не указан"})
				return
			}
			next(w, r)
		}
	}

	// API endpoints для SPA (защищены)
	mux.HandleFunc("/api/overview", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		clients := dp.DashboardClients()
		agents := dp.DashboardAgents()
		stats := dp.DashboardAuditStats()
		online := 0
		for _, a := range agents {
			if a.IsOnline {
				online++
			}
		}
		json.NewEncoder(w).Encode(map[string]any{
			"agents": agents, "online_agents": online,
			"clients": clients,
			"stats":   stats,
		})
	}))

	mux.HandleFunc("/api/agents", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"agents": dp.DashboardAgents()})
	}))

	mux.HandleFunc("/api/clients", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"clients": dp.DashboardClients()})
	}))

	mux.HandleFunc("/api/audit", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		stats := dp.DashboardAuditStats()
		json.NewEncoder(w).Encode(map[string]any{"entries": stats.Entries, "total": stats.TotalEntries})
	}))

	// Static files (SPA — index.html serves login page, auth via token)
	sub, _ := fs.Sub(staticFS, "static")
	fileServer := http.FileServer(http.FS(sub))

	mux.HandleFunc("/", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		path := r.URL.Path
		if path == "/" {
			path = "/index.html"
		}

		f, err := sub.Open(path[1:])
		if err == nil {
			f.Close()
			fileServer.ServeHTTP(w, r)
			return
		}

		r.URL.Path = "/index.html"
		fileServer.ServeHTTP(w, r)
	}))

	return mux
}

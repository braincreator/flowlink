// Package dashboard — встроенный web-dashboard для flowlink relay.
// Всё статика встроена через embed.FS, работает без интернета.
package dashboard

import (
	"github.com/braincreator/flowlink/internal/protocol"
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

// BackupInfo — информация о бэкапе для dashboard.
type BackupInfo struct {
	ID          string `json:"id"`
	Description string `json:"description"`
	Timestamp   int64  `json:"timestamp"`
	Size        int64  `json:"size"`
	Paths       []string `json:"paths"`
	Filename    string `json:"filename"`
}

// StorageConfigInfo — конфигурация хранилища (редактируемая через integration proxy).
type StorageConfigInfo struct {
	Type     string `json:"type"`     // "local" — S3 идёт через integration proxy
	LocalDir string `json:"local_dir,omitempty"`
}

// BackupConfigInfo — конфигурация бэкапов.
type BackupConfigInfo struct {
	Enabled       bool   `json:"enabled"`
	MaxSnapshots  int    `json:"max_snapshots"`
	MaxTotalSize  int64  `json:"max_total_size"`
	RetentionDays int    `json:"retention_days"`
	BackupDir     string `json:"backup_dir"`
}

// ApprovalInfo — информация о команде, ожидающей подтверждения.
type ApprovalInfo struct {
	ID        string `json:"id"`
	AgentID   string `json:"agent_id"`
	Command   string `json:"command"`
	RiskLevel string `json:"risk_level"`
	Reason    string `json:"reason"`
	CreatedAt int64  `json:"created_at"`
}

// DataProvider — интерфейс для получения данных (реализуется в relay).
type DataProvider interface {
	DashboardAgents() []AgentInfo
	DashboardClients() []ClientInfo
	DashboardAuditStats() *AuditStatsInfo
	// Storage & Backup
	DashboardStorageConfig() *StorageConfigInfo
	DashboardBackupConfig() *BackupConfigInfo
	DashboardBackups() []BackupInfo
	DashboardCreateBackup(paths []string, reason string) (*BackupInfo, error)
	DashboardRestoreBackup(snapshotID string) error
	DashboardDeleteBackup(snapshotID string) error
	// Config management
	DashboardGetConfig() map[string]any
	DashboardUpdateConfig(updates map[string]any) error
	// Approvals
	DashboardApprovals() []ApprovalInfo
	DashboardApproveCommand(approvalID string, approved bool) error
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
				json.NewEncoder(w).Encode(map[string]string{"code": "401", "error": protocol.T(protocol.CodeTokenMissing)})
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

	// === Config ===
	mux.HandleFunc("/api/config", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodPut {
			var updates map[string]any
			if err := json.NewDecoder(r.Body).Decode(&updates); err != nil {
				w.WriteHeader(http.StatusBadRequest)
				json.NewEncoder(w).Encode(map[string]string{"error": "invalid JSON"})
				return
			}
			if err := dp.DashboardUpdateConfig(updates); err != nil {
				w.WriteHeader(http.StatusInternalServerError)
				json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
				return
			}
			json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
			return
		}
		json.NewEncoder(w).Encode(dp.DashboardGetConfig())
	}))

	// === Storage ===
	mux.HandleFunc("/api/storage", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodPut {
			var updates map[string]any
			if err := json.NewDecoder(r.Body).Decode(&updates); err != nil {
				w.WriteHeader(http.StatusBadRequest)
				json.NewEncoder(w).Encode(map[string]string{"error": "invalid JSON"})
				return
			}
			if err := dp.DashboardUpdateConfig(map[string]any{"storage": updates}); err != nil {
				w.WriteHeader(http.StatusInternalServerError)
				json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
				return
			}
			json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
			return
		}
		json.NewEncoder(w).Encode(dp.DashboardStorageConfig())
	}))

	// === Backups ===
	mux.HandleFunc("/api/backups", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		switch r.Method {
		case http.MethodGet:
			json.NewEncoder(w).Encode(map[string]any{"backups": dp.DashboardBackups()})
		case http.MethodPost:
			var req struct {
				Paths  []string `json:"paths"`
				Reason string   `json:"reason"`
			}
			if err := json.NewDecoder(r.Body).Decode(&req); err != nil || len(req.Paths) == 0 {
				w.WriteHeader(http.StatusBadRequest)
				json.NewEncoder(w).Encode(map[string]string{"error": "paths required"})
				return
			}
			info, err := dp.DashboardCreateBackup(req.Paths, req.Reason)
			if err != nil {
				w.WriteHeader(http.StatusInternalServerError)
				json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
				return
			}
			json.NewEncoder(w).Encode(info)
		}
	}))

	mux.HandleFunc("/api/backups/", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		snapshotID := strings.TrimPrefix(r.URL.Path, "/api/backups/")
		if snapshotID == "" {
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		switch r.Method {
		case http.MethodPost:
			if strings.HasSuffix(r.URL.Path, "/restore") {
				snapshotID = strings.TrimSuffix(snapshotID, "/restore")
				if err := dp.DashboardRestoreBackup(snapshotID); err != nil {
					w.WriteHeader(http.StatusInternalServerError)
					json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
					return
				}
				json.NewEncoder(w).Encode(map[string]string{"status": "restored"})
				return
			}
			w.WriteHeader(http.StatusNotFound)
		case http.MethodDelete:
			if err := dp.DashboardDeleteBackup(snapshotID); err != nil {
				w.WriteHeader(http.StatusInternalServerError)
				json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
				return
			}
			json.NewEncoder(w).Encode(map[string]string{"status": "deleted"})
		}
	}))

	// === Approvals ===
	mux.HandleFunc("/api/approvals", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"approvals": dp.DashboardApprovals()})
	}))

	mux.HandleFunc("/api/approvals/", authMiddleware(func(w http.ResponseWriter, r *http.Request) {
		approvalID := strings.TrimPrefix(r.URL.Path, "/api/approvals/")
		if r.Method != http.MethodPost {
			w.WriteHeader(http.StatusMethodNotAllowed)
			return
		}
		var req struct {
			Approved bool `json:"approved"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		if err := dp.DashboardApproveCommand(approvalID, req.Approved); err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
			return
		}
		status := "rejected"
		if req.Approved {
			status = "approved"
		}
		json.NewEncoder(w).Encode(map[string]string{"status": status})
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

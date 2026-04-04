// Package relay — HTTP middleware для реле.
// Auth, rate limiting, CORS, logging.
package relay

import (
	"bufio"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"strings"
	"time"
)

// Middleware — тип для HTTP middleware.
type Middleware func(http.Handler) http.Handler

// Chain — объединяет middleware в цепочку.
func Chain(middlewares ...Middleware) Middleware {
	return func(next http.Handler) http.Handler {
		for i := len(middlewares) - 1; i >= 0; i-- {
			next = middlewares[i](next)
		}
		return next
	}
}

// === Auth Middleware ===

// AuthMiddlewareConfig — конфигурация для auth middleware.
type AuthMiddlewareConfig struct {
	AuthManager  *AuthManager
	StaticToken  string   // Статический токен из конфига
	SkipPaths    []string // Пути без аутентификации
	Logger       *slog.Logger
}

// AuthMiddleware — middleware для аутентификации.
func AuthMiddleware(cfg AuthMiddlewareConfig) Middleware {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	// Пути без аутентификации
	skipMap := make(map[string]bool)
	for _, path := range cfg.SkipPaths {
		skipMap[path] = true
	}

	// Auth endpoints (без проверки токена — они сами выдают токены)
	authPaths := map[string]bool{
		"/api/v1/auth/token":   true,
		"/api/v1/auth/refresh": true,
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Пропускаем исключённые пути (exact match или prefix для /path/)
			skip := false
			for p := range skipMap {
				if r.URL.Path == p || strings.HasPrefix(r.URL.Path, p) {
					skip = true
					break
				}
			}
			if skip {
				next.ServeHTTP(w, r)
				return
			}

			// Пропускаем auth endpoints (они сами обрабатывают аутентификацию)
			if authPaths[r.URL.Path] {
				next.ServeHTTP(w, r)
				return
			}

			// Если нет AuthManager и нет StaticToken — пропускаем (dev mode)
			if cfg.AuthManager == nil && cfg.StaticToken == "" {
				next.ServeHTTP(w, r)
				return
			}

			// Получаем токен из заголовка или query param
		authHeader := r.Header.Get("Authorization")
			token := authHeader
			if token == "" {
				token = r.URL.Query().Get("token")
			}
			if token == "" {
				writeAuthError(w, "токен не указан", http.StatusUnauthorized)
				return
			}

			// Убираем "Bearer " префикс
			if strings.HasPrefix(token, "Bearer ") {
				token = strings.TrimPrefix(token, "Bearer ")
			}

			// Проверяем формат access_token:refresh_token (для auto-refresh)
			var accessToken, refreshToken string
			if strings.Contains(token, ":") {
				parts := strings.SplitN(token, ":", 2)
				accessToken = parts[0]
				refreshToken = parts[1]
			} else {
				accessToken = token
			}

			// Вариант 1: Проверка через AuthManager (динамические токены)
			if cfg.AuthManager != nil {
				clientID, err := cfg.AuthManager.ValidateTokenWithBlacklist(accessToken)

				if err == nil {
					// Токен валиден — добавляем client_id в заголовок
					r.Header.Set("X-Client-ID", clientID)
					next.ServeHTTP(w, r)
					return
				}

				// Если токен истёк и есть refresh token — пробуем auto-refresh
				if strings.Contains(err.Error(), "истёк") && refreshToken != "" {
					newPair, refreshErr := cfg.AuthManager.RefreshToken(refreshToken)
					if refreshErr == nil {
						// Auto-refresh успешен — возвращаем новые токены в заголовках
						w.Header().Set("X-New-Access-Token", newPair.AccessToken)
						w.Header().Set("X-New-Refresh-Token", newPair.RefreshToken)
						w.Header().Set("X-Token-Refreshed", "true")

						// Устанавливаем client_id и продолжаем
						r.Header.Set("X-Client-ID", extractClientIDFromToken(newPair.AccessToken))
						next.ServeHTTP(w, r)
						return
					}
					cfg.Logger.Debug("auto-refresh не удался", "err", refreshErr)
				}

				// Если ошибка не "токен не найден" и не "истёк" — логируем
				if !strings.Contains(err.Error(), "не найден") && !strings.Contains(err.Error(), "истёк") {
					cfg.Logger.Debug("ошибка валидации токена", "err", err)
				}
			}

			// Вариант 2: Проверка статического токена из конфига
			if cfg.StaticToken != "" && accessToken == cfg.StaticToken {
				r.Header.Set("X-Client-ID", "static-client")
				next.ServeHTTP(w, r)
				return
			}

			// Токен невалиден
			writeAuthError(w, "неверный токен", http.StatusUnauthorized)
		})
	}
}

// extractClientIDFromToken — извлекает client_id из токена без валидации.
func extractClientIDFromToken(tokenStr string) string {
	parts := strings.Split(tokenStr, ".")
	if len(parts) != 2 {
		return ""
	}

	tokenJSON, err := base64.RawURLEncoding.DecodeString(parts[0])
	if err != nil {
		return ""
	}

	var token struct {
		ClientID string `json:"client_id"`
	}
	if err := json.Unmarshal(tokenJSON, &token); err != nil {
		return ""
	}

	return token.ClientID
}

// === Rate Limit Middleware ===

// RateLimitMiddleware — middleware для rate limiting.
func RateLimitMiddleware(limiter *RateLimiter, logger *slog.Logger) Middleware {
	if logger == nil {
		logger = slog.Default()
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if limiter == nil {
				next.ServeHTTP(w, r)
				return
			}

			// Получаем client_id
			clientID := GetClientIDFromContext(r)

			// Проверяем лимит
			allowed, retryAfter := limiter.Check(clientID)
			if !allowed {
				w.Header().Set("Retry-After", fmt.Sprintf("%d", retryAfter))
				w.Header().Set("X-RateLimit-Limit", fmt.Sprintf("%d/%d", limiter.maxPerMin, limiter.maxPerHour))
				writeAuthError(w, "rate limit exceeded", http.StatusTooManyRequests)
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// === CORS Middleware ===

// CORSMiddleware — middleware для CORS.
func CORSMiddleware(allowedOrigins []string, logger *slog.Logger) Middleware {
	if logger == nil {
		logger = slog.Default()
	}

	// Если origins пустой — разрешаем все
	allowAll := len(allowedOrigins) == 0
	originMap := make(map[string]bool)
	for _, origin := range allowedOrigins {
		originMap[origin] = true
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			origin := r.Header.Get("Origin")
			if origin == "" {
				next.ServeHTTP(w, r)
				return
			}

			// Проверяем origin
			allowed := allowAll || originMap[origin] || originMap["*"]
			if !allowed {
				logger.Debug("CORS: origin не разрешён", "origin", origin)
				next.ServeHTTP(w, r)
				return
			}

			// Устанавливаем CORS заголовки
			w.Header().Set("Access-Control-Allow-Origin", origin)
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Client-ID")
			w.Header().Set("Access-Control-Expose-Headers", "X-Client-ID, X-RateLimit-Limit, Retry-After")
			w.Header().Set("Access-Control-Max-Age", "86400") // 24 часа

			// Preflight request
			if r.Method == http.MethodOptions {
				w.WriteHeader(http.StatusOK)
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// === Request Logger Middleware ===

// RequestLoggerMiddleware — middleware для логирования запросов (audit).
func RequestLoggerMiddleware(logger *slog.Logger) Middleware {
	if logger == nil {
		logger = slog.Default()
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			start := time.Now()

			// Создаём wrapper для response writer, чтобы получить статус
			rw := &responseWriter{ResponseWriter: w, status: http.StatusOK}

			// Выполняем запрос
			next.ServeHTTP(rw, r)

			// Логируем
			duration := time.Since(start)
			clientID := GetClientIDFromContext(r)

			// Логируем все запросы, кроме health checks и SSE
			if r.URL.Path != "/health" && r.URL.Path != "/metrics" && r.URL.Path != "/api/v1/events" {
				logger.Info("HTTP запрос",
					"method", r.Method,
					"path", r.URL.Path,
					"status", rw.status,
					"duration_ms", duration.Milliseconds(),
					"client_id", clientID,
					"remote_addr", r.RemoteAddr,
					"user_agent", r.UserAgent(),
				)
			}

			// Дополнительное логирование для ошибок
			if rw.status >= 400 {
				logger.Warn("HTTP ошибка",
					"method", r.Method,
					"path", r.URL.Path,
					"status", rw.status,
					"client_id", clientID,
				)
			}
		})
	}
}

// responseWriter — wrapper для http.ResponseWriter, сохраняющий статус.
type responseWriter struct {
	http.ResponseWriter
	status int
}

func (rw *responseWriter) WriteHeader(status int) {
	rw.status = status
	rw.ResponseWriter.WriteHeader(status)
}

// Flush реализует http.Flusher для поддержки SSE streaming.
func (rw *responseWriter) Flush() {
	if f, ok := rw.ResponseWriter.(http.Flusher); ok {
		f.Flush()
	}
}

// Hijack реализует http.Hijacker для поддержки WebSocket.
func (rw *responseWriter) Hijack() (net.Conn, *bufio.ReadWriter, error) {
	if h, ok := rw.ResponseWriter.(http.Hijacker); ok {
		return h.Hijack()
	}
	return nil, nil, http.ErrNotSupported
}

// === Recovery Middleware ===

// RecoveryMiddleware — middleware для восстановления после panic.
func RecoveryMiddleware(logger *slog.Logger) Middleware {
	if logger == nil {
		logger = slog.Default()
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			defer func() {
				if err := recover(); err != nil {
					logger.Error("panic восстановлен",
						"err", err,
						"path", r.URL.Path,
						"method", r.Method,
						"remote_addr", r.RemoteAddr,
					)

					writeAuthError(w, "internal server error", http.StatusInternalServerError)
				}
			}()

			next.ServeHTTP(w, r)
		})
	}
}

// === Helpers ===

// writeAuthError — записывает ошибку аутентификации в формате JSON.
func writeAuthError(w http.ResponseWriter, message string, status int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(map[string]string{
		"error": message,
		"code":  fmt.Sprintf("%d", status),
	})
}

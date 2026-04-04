package nginx

import (
	"os"
	"os/exec"
	"strings"
	"testing"
)

func TestGenerateHTTPConfig(t *testing.T) {
	config := Config{
		Domain:         "example.com",
		WSSPath:        "/ws",
		APIPrefix:      "/api/v1",
		DashboardPath:  "/",
		Port:           80,
		TLS:            false,
		BackendAPIPort: 8080,
		BackendWSSPort: 8443,
		RateLimit:      100,
		EnableGzip:     true,
	}

	gen := NewGenerator(config)
	output, err := gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	// Проверяем что конфиг содержит ключевые элементы
	tests := []struct {
		name     string
		contains string
	}{
		{"domain", "server_name example.com"},
		{"listen 80", "listen 80"},
		{"ws path", "location /ws"},
		{"api prefix", "location /api/v1/"},
		{"sse endpoint", "location /api/v1/events"},
		{"dashboard", "try_files $uri $uri/ /index.html"},
		{"gzip", "gzip on"},
		{"security header", "X-Frame-Options"},
		{"rate limit", "limit_req_zone"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if !strings.Contains(output, tt.contains) {
				t.Errorf("Config doesn't contain %q\nGot:\n%s", tt.contains, output)
			}
		})
	}

	// Проверяем что нет HTTPS специфичных элементов
	if strings.Contains(output, "ssl_certificate") {
		t.Error("HTTP config should not contain ssl_certificate")
	}
}

func TestGenerateHTTPSConfig(t *testing.T) {
	config := Config{
		Domain:         "secure.example.com",
		WSSPath:        "/ws",
		APIPrefix:      "/api/v1",
		DashboardPath:  "/",
		Port:           443,
		TLS:            true,
		CertPath:       "/etc/letsencrypt/live/secure.example.com/fullchain.pem",
		KeyPath:        "/etc/letsencrypt/live/secure.example.com/privkey.pem",
		BackendAPIPort: 8080,
		BackendWSSPort: 8443,
		RateLimit:      100,
		EnableGzip:     true,
	}

	gen := NewGenerator(config)
	output, err := gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	// Проверяем HTTPS специфичные элементы
	tests := []struct {
		name     string
		contains string
	}{
		{"listen 443", "listen 443 ssl http2"},
		{"ssl certificate", "ssl_certificate /etc/letsencrypt/live/secure.example.com/fullchain.pem"},
		{"ssl key", "ssl_certificate_key /etc/letsencrypt/live/secure.example.com/privkey.pem"},
		{"tls protocols", "ssl_protocols TLSv1.2 TLSv1.3"},
		{"http redirect", "return 301 https://"},
		{"lets encrypt", "/.well-known/acme-challenge/"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if !strings.Contains(output, tt.contains) {
				t.Errorf("Config doesn't contain %q\nGot:\n%s", tt.contains, output)
			}
		})
	}
}

func TestWSSUpgradeHeaders(t *testing.T) {
	config := DefaultConfig()
	config.Domain = "test.com"

	gen := NewGenerator(config)
	output, err := gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	// Проверяем WSS upgrade headers
	requiredHeaders := []string{
		"proxy_set_header Upgrade $http_upgrade",
		"proxy_set_header Connection \"upgrade\"",
	}

	for _, header := range requiredHeaders {
		if !strings.Contains(output, header) {
			t.Errorf("Config missing WSS header: %q", header)
		}
	}
}

func TestSSENoBuffering(t *testing.T) {
	config := DefaultConfig()
	config.Domain = "test.com"

	gen := NewGenerator(config)
	output, err := gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	// Проверяем SSE no-buffering настройки
	sseSettings := []string{
		"location /api/v1/events",
		"proxy_buffering off",
		"proxy_cache off",
		"X-Accel-Buffering \"no\"",
	}

	for _, setting := range sseSettings {
		if !strings.Contains(output, setting) {
			t.Errorf("Config missing SSE setting: %q", setting)
		}
	}
}

func TestSecurityHeaders(t *testing.T) {
	config := DefaultConfig()
	config.Domain = "test.com"

	gen := NewGenerator(config)
	output, err := gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	securityHeaders := []string{
		"X-Frame-Options",
		"X-Content-Type-Options",
		"X-XSS-Protection",
		"Referrer-Policy",
		"Permissions-Policy",
	}

	for _, header := range securityHeaders {
		if !strings.Contains(output, header) {
			t.Errorf("Config missing security header: %q", header)
		}
	}
}

func TestRateLimiting(t *testing.T) {
	// С rate limiting
	config := DefaultConfig()
	config.Domain = "test.com"
	config.RateLimit = 50

	gen := NewGenerator(config)
	output, err := gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	if !strings.Contains(output, "limit_req_zone") {
		t.Error("Rate limiting should be present when RateLimit > 0")
	}
	if !strings.Contains(output, "rate=50r/s") {
		t.Error("Rate limit should be 50r/s")
	}

	// Без rate limiting
	config.RateLimit = 0
	gen = NewGenerator(config)
	output, err = gen.Generate()
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	if strings.Contains(output, "limit_req_zone") {
		t.Error("Rate limiting should NOT be present when RateLimit = 0")
	}
}

func TestGenerateFullConfig(t *testing.T) {
	config := Config{
		Domain:         "example.com",
		WSSPath:        "/ws",
		APIPrefix:      "/api/v1",
		DashboardPath:  "/",
		Port:           443,
		TLS:            true,
		CertPath:       "/etc/letsencrypt/live/example.com/fullchain.pem",
		KeyPath:        "/etc/letsencrypt/live/example.com/privkey.pem",
		BackendAPIPort: 8080,
		BackendWSSPort: 8443,
		RateLimit:      100,
		EnableGzip:     true,
	}

	gen := NewGenerator(config)
	output, err := gen.GenerateFullConfig()
	if err != nil {
		t.Fatalf("GenerateFullConfig() error = %v", err)
	}

	// Проверяем что это полный конфиг
	requiredElements := []string{
		"worker_processes auto",
		"events {",
		"http {",
		"upstream flowlink_api",
		"upstream flowlink_wss",
		"127.0.0.1:8080",
		"127.0.0.1:8443",
	}

	for _, elem := range requiredElements {
		if !strings.Contains(output, elem) {
			t.Errorf("Full config missing element: %q", elem)
		}
	}
}

func TestConfigValidation(t *testing.T) {
	tests := []struct {
		name    string
		config  Config
		wantErr bool
		errMsg  string
	}{
		{
			name:    "empty domain",
			config:  Config{Port: 80},
			wantErr: true,
			errMsg:  "domain is required",
		},
		{
			name:    "invalid port",
			config:  Config{Domain: "test.com", Port: 8080},
			wantErr: true,
			errMsg:  "port must be 80 or 443",
		},
		{
			name: "TLS with port 80",
			config: Config{
				Domain: "test.com",
				Port:   80,
				TLS:    true,
			},
			wantErr: true,
			errMsg:  "TLS requires port 443",
		},
		{
			name: "TLS missing cert_path",
			config: Config{
				Domain: "test.com",
				Port:   443,
				TLS:    true,
				KeyPath: "/path/to/key.pem",
			},
			wantErr: true,
			errMsg:  "cert_path is required",
		},
		{
			name: "TLS missing key_path",
			config: Config{
				Domain:   "test.com",
				Port:     443,
				TLS:      true,
				CertPath: "/path/to/cert.pem",
			},
			wantErr: true,
			errMsg:  "key_path is required",
		},
		{
			name: "empty ws_path",
			config: Config{
				Domain:    "test.com",
				Port:      80,
				WSSPath:   "",
				APIPrefix: "/api/v1",
			},
			wantErr: true,
			errMsg:  "ws_path is required",
		},
		{
			name: "empty api_prefix",
			config: Config{
				Domain:    "test.com",
				Port:      80,
				WSSPath:   "/ws",
				APIPrefix: "",
			},
			wantErr: true,
			errMsg:  "api_prefix is required",
		},
		{
			name: "valid HTTP config",
			config: Config{
				Domain:    "test.com",
				Port:      80,
				WSSPath:   "/ws",
				APIPrefix: "/api/v1",
			},
			wantErr: false,
		},
		{
			name: "valid HTTPS config",
			config: Config{
				Domain:    "test.com",
				Port:      443,
				TLS:       true,
				WSSPath:   "/ws",
				APIPrefix: "/api/v1",
				CertPath:  "/path/to/cert.pem",
				KeyPath:   "/path/to/key.pem",
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.config.Validate()
			if tt.wantErr {
				if err == nil {
					t.Errorf("Validate() expected error containing %q, got nil", tt.errMsg)
				} else if !strings.Contains(err.Error(), tt.errMsg) {
					t.Errorf("Validate() error = %v, want error containing %q", err, tt.errMsg)
				}
			} else {
				if err != nil {
					t.Errorf("Validate() unexpected error = %v", err)
				}
			}
		})
	}
}

func TestDefaultConfig(t *testing.T) {
	config := DefaultConfig()

	if config.Domain != "example.com" {
		t.Errorf("Default domain = %q, want %q", config.Domain, "example.com")
	}
	if config.WSSPath != "/ws" {
		t.Errorf("Default WSSPath = %q, want %q", config.WSSPath, "/ws")
	}
	if config.APIPrefix != "/api/v1" {
		t.Errorf("Default APIPrefix = %q, want %q", config.APIPrefix, "/api/v1")
	}
	if config.BackendAPIPort != 8080 {
		t.Errorf("Default BackendAPIPort = %d, want %d", config.BackendAPIPort, 8080)
	}
	if config.BackendWSSPort != 8443 {
		t.Errorf("Default BackendWSSPort = %d, want %d", config.BackendWSSPort, 8443)
	}
}

func TestNewGeneratorDefaults(t *testing.T) {
	// Тестируем что NewGenerator применяет дефолты
	config := Config{
		Domain: "test.com",
		// Остальные поля пустые
	}

	gen := NewGenerator(config)

	if gen.config.WSSPath != "/ws" {
		t.Errorf("WSSPath not defaulted: %q", gen.config.WSSPath)
	}
	if gen.config.APIPrefix != "/api/v1" {
		t.Errorf("APIPrefix not defaulted: %q", gen.config.APIPrefix)
	}
	if gen.config.BackendAPIPort != 8080 {
		t.Errorf("BackendAPIPort not defaulted: %d", gen.config.BackendAPIPort)
	}
	if gen.config.BackendWSSPort != 8443 {
		t.Errorf("BackendWSSPort not defaulted: %d", gen.config.BackendWSSPort)
	}
	if gen.config.Port != 80 {
		t.Errorf("Port not defaulted for non-TLS: %d", gen.config.Port)
	}

	// С TLS должен быть порт 443
	config.TLS = true
	gen = NewGenerator(config)
	if gen.config.Port != 443 {
		t.Errorf("Port not defaulted for TLS: %d", gen.config.Port)
	}
}

// TestNginxConfigValidity проверяет валидность генерируемого конфига через nginx -t
// Тест пропускается если nginx не установлен.
func TestNginxConfigValidity(t *testing.T) {
	// Проверяем наличие nginx
	nginxPath, err := exec.LookPath("nginx")
	if err != nil {
		t.Skip("nginx not found, skipping validation test")
	}

	tests := []struct {
		name   string
		config Config
	}{
		{
			name: "HTTP config",
			config: Config{
				Domain:         "example.com",
				WSSPath:        "/ws",
				APIPrefix:      "/api/v1",
				DashboardPath:  "/",
				Port:           80,
				TLS:            false,
				BackendAPIPort: 8080,
				BackendWSSPort: 8443,
				RateLimit:      100,
				EnableGzip:     true,
			},
		},
		{
			name: "HTTPS config",
			config: Config{
				Domain:         "secure.example.com",
				WSSPath:        "/ws",
				APIPrefix:      "/api/v1",
				DashboardPath:  "/",
				Port:           443,
				TLS:            true,
				CertPath:       "/etc/letsencrypt/live/secure.example.com/fullchain.pem",
				KeyPath:        "/etc/letsencrypt/live/secure.example.com/privkey.pem",
				BackendAPIPort: 8080,
				BackendWSSPort: 8443,
				RateLimit:      100,
				EnableGzip:     true,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gen := NewGenerator(tt.config)
			output, err := gen.GenerateFullConfig()
			if err != nil {
				t.Fatalf("GenerateFullConfig() error = %v", err)
			}

			// Создаём временный файл для конфига
			tmpFile := t.TempDir() + "/nginx.conf"
			if err := writeFile(tmpFile, output); err != nil {
				t.Fatalf("Failed to write temp config: %v", err)
			}

			// Запускаем nginx -t для проверки синтаксиса
			// Примечание: nginx -t требует наличия директорий и сертификатов,
			// поэтому этот тест может не пройти в CI окружении
			cmd := exec.Command(nginxPath, "-t", "-c", tmpFile)
			outputBytes, err := cmd.CombinedOutput()
			if err != nil {
				// Проверяем что ошибка связана с отсутствием файлов, а не с синтаксисом
				outputStr := string(outputBytes)
				if strings.Contains(outputStr, "syntax is ok") {
					// Синтаксис валидный, но отсутствуют файлы
					t.Logf("Config syntax is valid (missing files expected): %s", outputStr)
				} else {
					t.Errorf("nginx -t failed:\n%s\nConfig:\n%s", outputStr, output)
				}
			}
		})
	}
}

func writeFile(path, content string) error {
	return os.WriteFile(path, []byte(content), 0644)
}

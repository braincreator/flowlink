// Package nginx — генератор nginx конфигурации для FlowLink Relay.
// Генерирует оптимальный конфиг для self-host пользователей с поддержкой
// WSS proxy, API reverse proxy, SSE endpoints, и security headers.
package nginx

import (
	"bytes"
	"fmt"
	"strings"
	"text/template"
)

// Config — параметры для генерации nginx конфига.
type Config struct {
	// Domain — домен сервера (например, "example.com")
	Domain string

	// WSSPath — путь для WebSocket соединений (по умолчанию "/ws")
	WSSPath string

	// APIPrefix — префикс для API endpoints (по умолчанию "/api/v1")
	APIPrefix string

	// DashboardPath — путь для dashboard SPA (по умолчанию "/")
	DashboardPath string

	// Port — порт для listen (80 или 443)
	Port int

	// TLS — включить HTTPS
	TLS bool

	// CertPath — путь к SSL сертификату (для TLS)
	CertPath string

	// KeyPath — путь к SSL ключу (для TLS)
	KeyPath string

	// BackendAPIPort — порт backend API (по умолчанию 8080)
	BackendAPIPort int

	// BackendWSSPort — порт backend WSS (по умолчанию 8443)
	BackendWSSPort int

	// RateLimit — лимит запросов в секунду (0 = без лимита)
	RateLimit int

	// EnableGzip — включить gzip сжатие
	EnableGzip bool
}

// DefaultConfig возвращает конфиг с значениями по умолчанию.
func DefaultConfig() Config {
	return Config{
		Domain:         "example.com",
		WSSPath:        "/ws",
		APIPrefix:      "/api/v1",
		DashboardPath:  "/",
		Port:           80,
		TLS:            false,
		CertPath:       "/etc/letsencrypt/live/example.com/fullchain.pem",
		KeyPath:        "/etc/letsencrypt/live/example.com/privkey.pem",
		BackendAPIPort: 8080,
		BackendWSSPort: 8443,
		RateLimit:      100,
		EnableGzip:     true,
	}
}

// Generator — генератор nginx конфигов.
type Generator struct {
	config Config
}

// NewGenerator создаёт новый генератор с указанной конфигурацией.
func NewGenerator(config Config) *Generator {
	// Применяем значения по умолчанию для пустых полей
	if config.WSSPath == "" {
		config.WSSPath = "/ws"
	}
	if config.APIPrefix == "" {
		config.APIPrefix = "/api/v1"
	}
	if config.DashboardPath == "" {
		config.DashboardPath = "/"
	}
	if config.Port == 0 {
		if config.TLS {
			config.Port = 443
		} else {
			config.Port = 80
		}
	}
	if config.BackendAPIPort == 0 {
		config.BackendAPIPort = 8080
	}
	if config.BackendWSSPort == 0 {
		config.BackendWSSPort = 8443
	}
	if config.CertPath == "" {
		config.CertPath = fmt.Sprintf("/etc/letsencrypt/live/%s/fullchain.pem", config.Domain)
	}
	if config.KeyPath == "" {
		config.KeyPath = fmt.Sprintf("/etc/letsencrypt/live/%s/privkey.pem", config.Domain)
	}

	return &Generator{config: config}
}

// Generate генерирует полный nginx server block.
func (g *Generator) Generate() (string, error) {
	var buf bytes.Buffer

	// Если TLS включен, генерируем HTTP→HTTPS redirect + HTTPS server
	if g.config.TLS {
		httpRedirect, err := g.generateHTTPRedirect()
		if err != nil {
			return "", fmt.Errorf("failed to generate HTTP redirect: %w", err)
		}
		buf.WriteString(httpRedirect)
		buf.WriteString("\n")
	}

	// Генерируем основной server block
	serverBlock, err := g.generateServerBlock()
	if err != nil {
		return "", fmt.Errorf("failed to generate server block: %w", err)
	}
	buf.WriteString(serverBlock)

	return buf.String(), nil
}

// generateHTTPRedirect генерирует server block для HTTP→HTTPS редиректа.
func (g *Generator) generateHTTPRedirect() (string, error) {
	tmpl := `# HTTP → HTTPS redirect
server {
    listen 80;
    listen [::]:80;
    server_name {{.Domain}};

    # Let's Encrypt ACME challenge
    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
        allow all;
    }

    # Redirect all other traffic to HTTPS
    location / {
        return 301 https://$server_name$request_uri;
    }
}
`
	t, err := template.New("http-redirect").Parse(tmpl)
	if err != nil {
		return "", err
	}

	var buf bytes.Buffer
	if err := t.Execute(&buf, g.config); err != nil {
		return "", err
	}

	return buf.String(), nil
}

// generateServerBlock генерирует основной server block.
func (g *Generator) generateServerBlock() (string, error) {
	tmpl := `# FlowLink Relay — {{.Domain}}
server {
    listen {{if .TLS}}443 ssl http2{{else}}80{{end}};
    {{if .TLS}}listen [::]:443 ssl http2;{{else}}listen [::]:80;{{end}}
    server_name {{.Domain}};

    {{if .TLS}}
    # TLS Configuration
    ssl_certificate {{.CertPath}};
    ssl_certificate_key {{.KeyPath}};
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:10m;
    ssl_session_tickets off;

    # HSTS (optional, enable after confirming HTTPS works)
    # add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    {{end}}

    {{if .EnableGzip}}
    # Gzip Compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_types text/plain text/css text/xml application/json application/javascript application/xml application/xml+rss text/javascript application/x-javascript image/svg+xml;
    gzip_disable "msie6";
    {{end}}

    # Security Headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Permissions-Policy "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()" always;

    {{if .RateLimit}}
    # Rate Limiting
    limit_req_zone $binary_remote_addr zone=flowlink_limit:10m rate={{.RateLimit}}r/s;
    limit_req zone=flowlink_limit burst=20 nodelay;
    {{end}}

    # Logging
    access_log /var/log/nginx/flowlink_access.log;
    error_log /var/log/nginx/flowlink_error.log warn;

    # Client body size (for file uploads)
    client_max_body_size 100M;

    # 1. SSE Events Endpoint — no buffering, long-lived connections
    location {{.APIPrefix}}/events {
        proxy_pass http://127.0.0.1:{{.BackendAPIPort}};
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # SSE-specific: disable buffering
        proxy_buffering off;
        proxy_cache off;
        proxy_set_header Connection '';
        proxy_read_timeout 86400s;
        chunked_transfer_encoding off;

        # CORS for SSE (if needed)
        add_header Cache-Control "no-cache, no-store";
        add_header X-Accel-Buffering "no";
    }

    # 2. WebSocket (WSS) Endpoint — upgrade headers
    location {{.WSSPath}} {
        proxy_pass http://127.0.0.1:{{.BackendWSSPort}};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket timeout settings
        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
    }

    # 3. API Reverse Proxy
    location {{.APIPrefix}}/ {
        proxy_pass http://127.0.0.1:{{.BackendAPIPort}};
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Request-ID $request_id;

        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # 4. Dashboard SPA (try_files fallback)
    location {{.DashboardPath}} {
        root /var/www/flowlink;
        try_files $uri $uri/ /index.html;

        # Cache static assets
        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)$ {
            root /var/www/flowlink;
            expires 1y;
            add_header Cache-Control "public, immutable";
        }
    }

    # Health check endpoint
    location /health {
        access_log off;
        return 200 "OK\n";
        add_header Content-Type text/plain;
    }

    # Deny access to hidden files
    location ~ /\. {
        deny all;
        access_log off;
        log_not_found off;
    }
}
`
	t, err := template.New("server-block").Parse(tmpl)
	if err != nil {
		return "", err
	}

	var buf bytes.Buffer
	if err := t.Execute(&buf, g.config); err != nil {
		return "", err
	}

	return strings.TrimSuffix(buf.String(), "\n"), nil
}

// GenerateFullConfig генерирует полный nginx.conf файл с http block.
func (g *Generator) GenerateFullConfig() (string, error) {
	serverBlock, err := g.Generate()
	if err != nil {
		return "", err
	}

	config := fmt.Sprintf(`# FlowLink Relay Nginx Configuration
# Generated for domain: %s
#
# Usage:
#   1. Save to /etc/nginx/sites-available/flowlink
#   2. Enable: sudo ln -s /etc/nginx/sites-available/flowlink /etc/nginx/sites-enabled/
#   3. Test: sudo nginx -t
#   4. Reload: sudo nginx -s reload
#
# Prerequisites:
#   - Nginx with websocket support
#   - Certbot for TLS (if enabled)
#   - Dashboard files in /var/www/flowlink/

worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {
    worker_connections 1024;
    use epoll;
    multi_accept on;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    # Logging format
    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for" '
                    'rt=$request_time uct="$upstream_connect_time" '
                    'uht="$upstream_header_time" urt="$upstream_response_time"';

    access_log /var/log/nginx/access.log main;

    # Performance optimizations
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    types_hash_max_size 2048;

    # Hide nginx version
    server_tokens off;

    # Buffer settings
    client_body_buffer_size 16k;
    client_header_buffer_size 1k;
    client_max_body_size 100M;
    large_client_header_buffers 4 8k;

    # Real IP from Cloudflare (if used)
    # set_real_ip_from 103.21.244.0/22;
    # set_real_ip_from 103.22.200.0/22;
    # set_real_ip_from 103.31.4.0/22;
    # set_real_ip_from 104.16.0.0/13;
    # set_real_ip_from 104.24.0.0/14;
    # set_real_ip_from 108.162.192.0/18;
    # set_real_ip_from 131.0.72.0/22;
    # set_real_ip_from 141.101.64.0/18;
    # set_real_ip_from 162.158.0.0/15;
    # set_real_ip_from 172.64.0.0/13;
    # set_real_ip_from 173.245.48.0/20;
    # set_real_ip_from 188.114.96.0/20;
    # set_real_ip_from 190.131.48.0/20;
    # set_real_ip_from 197.234.240.0/22;
    # set_real_ip_from 198.41.128.0/17;
    # real_ip_header CF-Connecting-IP;

    # Upstream backends
    upstream flowlink_api {
        server 127.0.0.1:%d;
        keepalive 32;
    }

    upstream flowlink_wss {
        server 127.0.0.1:%d;
        keepalive 32;
    }

    # Server blocks
%s
}
`, g.config.Domain, g.config.BackendAPIPort, g.config.BackendWSSPort, indentString(serverBlock, "    "))

	return config, nil
}

// indentString добавляет отступ к каждой строке.
func indentString(s, indent string) string {
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		if line != "" {
			lines[i] = indent + line
		}
	}
	return strings.Join(lines, "\n")
}

// Validate проверяет валидность конфигурации.
func (c Config) Validate() error {
	if c.Domain == "" {
		return fmt.Errorf("domain is required")
	}

	if c.Port != 0 && c.Port != 80 && c.Port != 443 {
		return fmt.Errorf("port must be 80 or 443")
	}

	if c.TLS {
		if c.Port == 80 {
			return fmt.Errorf("TLS requires port 443, not 80")
		}
		if c.CertPath == "" {
			return fmt.Errorf("cert_path is required when TLS is enabled")
		}
		if c.KeyPath == "" {
			return fmt.Errorf("key_path is required when TLS is enabled")
		}
	}

	if c.WSSPath == "" {
		return fmt.Errorf("ws_path is required")
	}

	if c.APIPrefix == "" {
		return fmt.Errorf("api_prefix is required")
	}

	return nil
}

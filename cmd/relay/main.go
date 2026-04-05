package main

import (
	"bufio"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"strings"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/nginx"
	"github.com/braincreator/flowlink/internal/relay"
	"github.com/braincreator/flowlink/pkg/version"
)

func main() {
	// Subcommand routing
	if len(os.Args) > 1 {
		switch os.Args[1] {
		case "setup":
			cmdSetup(os.Args[2:])
			return
		case "validate":
			cmdValidate(os.Args[2:])
			return
		case "nginx-config":
			cmdNginxConfig(os.Args[2:])
			return
		case "version", "--version", "-v":
			fmt.Printf("flowlink-relay %s\n", version.Version)
			return
		case "help", "--help", "-h":
			printHelp()
			return
		}
	}

	// Default: serve (legacy behavior with flags)
	serveCmd()
}

func printHelp() {
	fmt.Printf("flowlink-relay %s — FlowLink Relay Server\n\n", version.Version)
	fmt.Println("Usage:")
	fmt.Println("  flowlink-relay setup              Interactive setup wizard")
	fmt.Println("  flowlink-relay setup --non-interactive  Setup from env vars")
	fmt.Println("  flowlink-relay serve              Start relay server (default)")
	fmt.Println("  flowlink-relay validate           Validate config file")
	fmt.Println("  flowlink-relay nginx-config       Generate nginx config")
	fmt.Println("  flowlink-relay version            Show version")
	fmt.Println()
	fmt.Println("Nginx config generation:")
	fmt.Println("  flowlink-relay nginx-config --domain example.com [--tls]")
	fmt.Println("  flowlink-relay nginx-config --domain example.com --tls --output /etc/nginx/sites-available/flowlink")
	fmt.Println()
	fmt.Println("  Flags:")
	fmt.Println("    --domain         Domain name (required)")
	fmt.Println("    --tls            Enable HTTPS (generates HTTP→HTTPS redirect)")
	fmt.Println("    --ws-path        WebSocket path (default: /ws)")
	fmt.Println("    --api-prefix     API prefix (default: /api/v1)")
	fmt.Println("    --output         Output file path (default: stdout)")
	fmt.Println("    --full           Generate full nginx.conf (not just server block)")
	fmt.Println("    --cert-path      SSL certificate path (for TLS)")
	fmt.Println("    --key-path       SSL key path (for TLS)")
	fmt.Println("    --rate-limit     Rate limit requests/sec (default: 100)")
	fmt.Println("    --no-gzip        Disable gzip compression")
	fmt.Println()
	fmt.Println("Environment variables for setup:")
	fmt.Println("  FLOWLINK_API_TOKEN      API token")
	fmt.Println("  FLOWLINK_WSS_PORT       WSS port (default: 8443)")
	fmt.Println("  FLOWLINK_API_PORT       API port (default: 8080)")
	fmt.Println("  FLOWLINK_TLS_MODE       TLS mode: none, self-signed, letsencrypt")
	fmt.Println("  FLOWLINK_TLS_DOMAIN     Domain for Let's Encrypt")
	fmt.Println("  FLOWLINK_ADMIN_NAME     Admin name")
	fmt.Println("  FLOWLINK_ADMIN_EMAIL    Admin email")
	fmt.Println()
	fmt.Println("Flags for serve:")
	fmt.Println("  -config <path>         Config file path (default: relay.json)")
	fmt.Println("  -api-token <token>      API token (or FLOWLINK_API_TOKEN)")
	fmt.Println("  -letsencrypt-domain    Domain for Let's Encrypt")
}

// ====================
// Setup Command
// ====================

func cmdSetup(args []string) {
	nonInteractive := false
	for _, a := range args {
		if a == "--non-interactive" || a == "-y" {
			nonInteractive = true
		}
	}

	reader := bufio.NewReader(os.Stdin)

	fmt.Println()
	fmt.Println("╔═══════════════════════════════════════════════╗")
	fmt.Println("║   FlowLink Relay — Setup Wizard              ║")
	fmt.Printf("║   Version: %-35s║\n", version.Version)
	fmt.Println("╚═══════════════════════════════════════════════╝")
	fmt.Println()

	// Step 1: API Token
	apiToken := os.Getenv("FLOWLINK_API_TOKEN")
	if apiToken == "" {
		apiToken = generateToken(32)
	}

	if nonInteractive {
		// Use defaults from env
	} else {
		fmt.Println("━━━ Step 1/5: Security ━━━")
		fmt.Printf("  API Token (auto-generated, press Enter to keep):\n  ")
		fmt.Println(apiToken)
		fmt.Println()

		for {
			fmt.Print("  Generate new token? [y/N]: ")
			input, _ := reader.ReadString('\n')
			input = strings.TrimSpace(input)
			if input == "" || strings.EqualFold(input, "n") {
				break
			}
			if strings.EqualFold(input, "y") {
				apiToken = generateToken(32)
				fmt.Printf("  New token: %s\n", apiToken)
				break
			}
		}
		fmt.Println()
	}

	// Step 2: Ports
	wssPort := envOrDefault("FLOWLINK_WSS_PORT", "8443")
	apiPort := envOrDefault("FLOWLINK_API_PORT", "8080")

	if !nonInteractive {
		fmt.Println("━━━ Step 2/5: Network ━━━")
		fmt.Printf("  WSS port [%s]: ", wssPort)
		input, _ := reader.ReadString('\n')
		if strings.TrimSpace(input) != "" {
			wssPort = strings.TrimSpace(input)
		}
		fmt.Printf("  API port [%s]: ", apiPort)
		input, _ = reader.ReadString('\n')
		if strings.TrimSpace(input) != "" {
			apiPort = strings.TrimSpace(input)
		}
		fmt.Println()
	}

	// Step 3: TLS
	tlsMode := envOrDefault("FLOWLINK_TLS_MODE", "none")
	tlsDomain := os.Getenv("FLOWLINK_TLS_DOMAIN")

	if !nonInteractive {
		fmt.Println("━━━ Step 3/5: TLS ━━━")
		fmt.Println("  Options: none, self-signed, letsencrypt")
		fmt.Printf("  TLS mode [%s]: ", tlsMode)
		input, _ := reader.ReadString('\n')
		if strings.TrimSpace(input) != "" {
			tlsMode = strings.TrimSpace(input)
		}
		if tlsMode == "letsencrypt" || tlsMode == "self-signed" {
			fmt.Printf("  Domain [%s]: ", tlsDomain)
			input, _ = reader.ReadString('\n')
			if strings.TrimSpace(input) != "" {
				tlsDomain = strings.TrimSpace(input)
			}
		}
		fmt.Println()
	}

	// Step 4: Admin info
	adminName := envOrDefault("FLOWLINK_ADMIN_NAME", "")
	adminEmail := envOrDefault("FLOWLINK_ADMIN_EMAIL", "")

	if !nonInteractive {
		fmt.Println("━━━ Step 4/5: Admin ━━━")
		fmt.Print("  Your name: ")
		adminName, _ = reader.ReadString('\n')
		adminName = strings.TrimSpace(adminName)
		fmt.Print("  Your email: ")
		adminEmail, _ = reader.ReadString('\n')
		adminEmail = strings.TrimSpace(adminEmail)
		fmt.Println()
	}

	// Build config
	cfg := &config.RelayConfig{
		APIToken:      apiToken,
		WSSAddr:       ":" + wssPort,
		APIAddr:       ":" + apiPort,
		TLSMode:       tlsMode,
		TLSDomain:     tlsDomain,
		HeartbeatTimeout: 90,
		MaxAgents:     100,
	}

	// Save config
	configPath := "relay.json"
	if len(os.Args) > 2 {
		for i, a := range os.Args {
			if a == "setup" && i+1 < len(os.Args) && !strings.HasPrefix(os.Args[i+1], "-") {
				configPath = os.Args[i+1]
				break
			}
		}
	}

	if err := saveRelayConfig(configPath, cfg); err != nil {
		fmt.Fprintf(os.Stderr, "❌ Error saving config: %v\n", err)
		os.Exit(1)
	}

	// Step 5: Create first client + agent
	if !nonInteractive {
		fmt.Println("━━━ Step 5/5: First Client & Agent ━━━")
	}

	// Create a temporary relay instance to generate client + agent
	r := relay.NewRelay(cfg)
	client, err := r.CreateFirstClient(adminName, adminEmail)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Error creating client: %v\n", err)
		os.Exit(1)
	}

	agent, err := r.CreateFirstAgent(client.ID, "default-agent")
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Error creating agent: %v\n", err)
		os.Exit(1)
	}

	// Print results
	fmt.Println()
	fmt.Println("╔═══════════════════════════════════════════════╗")
	fmt.Println("║   ✅ Setup Complete!                          ║")
	fmt.Println("╚═══════════════════════════════════════════════╝")
	fmt.Println()
	fmt.Println("📋 Configuration saved to:", configPath)
	fmt.Println()
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Println("  🔑 Credentials (SAVE THESE!)")
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Printf("  Admin API Token:  %s\n", apiToken)
	fmt.Println()
	fmt.Printf("  Client ID:        %s\n", client.ID)
	fmt.Printf("  Client API Token: %s\n", client.APIToken)
	fmt.Println()
	fmt.Printf("  Agent ID:         %s\n", agent.ID)
	fmt.Printf("  Agent Token:      %s\n", agent.Token)
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Println()
	fmt.Println("🚀 Next steps:")
	fmt.Println("  1. Start relay:     ./flowlink-relay")
	fmt.Println("  2. Open dashboard:  http://localhost" + apiPort + "/dashboard/?token=" + apiToken)
	fmt.Println()
	fmt.Println("  On agent machine:")
	fmt.Printf("  ./flowlink init --relay ws://YOUR_IP:%s/ws --token %s\n", wssPort, agent.Token)
	fmt.Println()
}

// ====================
// Validate Command
// ====================

func cmdValidate(args []string) {
	configPath := "relay.json"
	if len(args) > 0 {
		configPath = args[0]
	}

	cfg, err := config.LoadRelayConfig(configPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Config error: %v\n", err)
		os.Exit(1)
	}

	warnings := 0
	fmt.Printf("Validating %s...\n\n", configPath)

	// Check API token
	if cfg.APIToken == "" {
		fmt.Println("  ❌ API token not set")
	} else if len(cfg.APIToken) < 16 {
		fmt.Println("  ⚠️  API token is short (< 16 chars)")
		warnings++
	} else {
		fmt.Println("  ✅ API token set")
	}

	// Check TLS
	if cfg.TLSMode == "" || cfg.TLSMode == "none" {
		fmt.Println("  ⚠️  TLS disabled — connections are not encrypted")
		warnings++
	} else {
		fmt.Printf("  ✅ TLS mode: %s\n", cfg.TLSMode)
		if cfg.TLSMode == "letsencrypt" && cfg.TLSDomain == "" {
			fmt.Println("  ❌ Let's Encrypt mode requires a domain")
		}
	}

	// Check ports
	if cfg.WSSAddr == "" {
		fmt.Println("  ❌ WSS address not set")
	} else {
		fmt.Printf("  ✅ WSS: %s\n", cfg.WSSAddr)
	}
	if cfg.APIAddr == "" {
		fmt.Println("  ❌ API address not set")
	} else {
		fmt.Printf("  ✅ API: %s\n", cfg.APIAddr)
	}

	// Check heartbeat
	if cfg.HeartbeatTimeout < 30 {
		fmt.Println("  ⚠️  Heartbeat timeout < 30s (agents may disconnect prematurely)")
		warnings++
	} else {
		fmt.Printf("  ✅ Heartbeat timeout: %ds\n", cfg.HeartbeatTimeout)
	}

	fmt.Println()
	if warnings == 0 {
		fmt.Println("✅ Configuration is valid!")
	} else {
		fmt.Printf("⚠️  %d warnings found\n", warnings)
	}
}

// ====================
// Nginx Config Command
// ====================

func cmdNginxConfig(args []string) {
	// Парсим флаги
	var (
		domain     string
		tls        bool
		wsPath     string
		apiPrefix  string
		outputPath string
		fullConfig bool
		certPath   string
		keyPath    string
		rateLimit  int
		noGzip     bool
	)

	fs := flag.NewFlagSet("nginx-config", flag.ExitOnError)
	fs.StringVar(&domain, "domain", "", "Domain name (required)")
	fs.BoolVar(&tls, "tls", false, "Enable HTTPS")
	fs.StringVar(&wsPath, "ws-path", "/ws", "WebSocket path")
	fs.StringVar(&apiPrefix, "api-prefix", "/api/v1", "API prefix")
	fs.StringVar(&outputPath, "output", "", "Output file path (default: stdout)")
	fs.BoolVar(&fullConfig, "full", false, "Generate full nginx.conf")
	fs.StringVar(&certPath, "cert-path", "", "SSL certificate path")
	fs.StringVar(&keyPath, "key-path", "", "SSL key path")
	fs.IntVar(&rateLimit, "rate-limit", 100, "Rate limit requests/sec")
	fs.BoolVar(&noGzip, "no-gzip", false, "Disable gzip compression")

	if err := fs.Parse(args); err != nil {
		fmt.Fprintf(os.Stderr, "Error parsing flags: %v\n", err)
		os.Exit(1)
	}

	if domain == "" {
		fmt.Fprintln(os.Stderr, "❌ --domain is required")
		fs.PrintDefaults()
		os.Exit(1)
	}

	// Импортируем пакет nginx
	config := nginx.Config{
		Domain:         domain,
		WSSPath:        wsPath,
		APIPrefix:      apiPrefix,
		TLS:            tls,
		CertPath:       certPath,
		KeyPath:        keyPath,
		BackendAPIPort: 8080,
		BackendWSSPort: 8443,
		RateLimit:      rateLimit,
		EnableGzip:     !noGzip,
	}

	if tls {
		config.Port = 443
		if certPath == "" {
			config.CertPath = fmt.Sprintf("/etc/letsencrypt/live/%s/fullchain.pem", domain)
		}
		if keyPath == "" {
			config.KeyPath = fmt.Sprintf("/etc/letsencrypt/live/%s/privkey.pem", domain)
		}
	} else {
		config.Port = 80
	}

	// Валидация
	if err := config.Validate(); err != nil {
		fmt.Fprintf(os.Stderr, "❌ Invalid config: %v\n", err)
		os.Exit(1)
	}

	// Генерируем конфиг
	gen := nginx.NewGenerator(config)
	var output string
	var err error

	if fullConfig {
		output, err = gen.GenerateFullConfig()
	} else {
		output, err = gen.Generate()
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Error generating config: %v\n", err)
		os.Exit(1)
	}

	// Выводим результат
	if outputPath != "" {
		if err := os.WriteFile(outputPath, []byte(output), 0644); err != nil {
			fmt.Fprintf(os.Stderr, "❌ Error writing file: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("✅ Nginx config written to %s\n", outputPath)
		fmt.Println("\nNext steps:")
		fmt.Println("  1. Review the config: cat", outputPath)
		fmt.Println("  2. Test config: sudo nginx -t")
		fmt.Println("  3. Reload nginx: sudo nginx -s reload")
	} else {
		fmt.Println(output)
	}
}

// ====================
// Serve Command (default)
// ====================

func serveCmd() {
	var (
		configPath        string
		apiToken          string
		showVer           bool
		letsencryptDomain string
	)

	flag.StringVar(&configPath, "config", "", "путь к файлу конфигурации")
	flag.StringVar(&apiToken, "api-token", "", "API токен (или через FLOWLINK_API_TOKEN)")
	flag.BoolVar(&showVer, "version", false, "показать версию")
	flag.StringVar(&letsencryptDomain, "letsencrypt-domain", "", "домен для Let's Encrypt")
	flag.Parse()

	if showVer {
		fmt.Printf("flowlink-relay %s\n", version.Version)
		return
	}

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	})))

	if configPath == "" {
		configPath = "relay.json"
	}

	cfg, err := config.LoadRelayConfig(configPath)
	if err != nil {
		slog.Error("ошибка загрузки конфига", "err", err)
		os.Exit(1)
	}

	if apiToken != "" {
		cfg.APIToken = apiToken
	} else if envToken := os.Getenv("FLOWLINK_API_TOKEN"); envToken != "" {
		cfg.APIToken = envToken
	}

	if letsencryptDomain != "" {
		cfg.TLSDomain = letsencryptDomain
		if cfg.TLSMode == "" {
			cfg.TLSMode = "letsencrypt"
		}
	}

	if cfg.APIToken == "" {
		slog.Warn("API токен не задан — HTTP API будет без авторизации")
	}

	slog.Info("запуск flowlink relay",
		"version", version.Version,
		"wss", cfg.WSSAddr,
		"api", cfg.APIAddr,
		"tls_mode", cfg.TLSMode,
		"tls_domain", cfg.TLSDomain,
		"llm_backends", len(cfg.LLMBackends),
	)

	r := relay.NewRelay(cfg)

	if len(cfg.LLMBackends) > 0 {
		backends := make([]relay.LLMBackend, len(cfg.LLMBackends))
		for i, b := range cfg.LLMBackends {
			backends[i] = relay.LLMBackend{
				Name:     b.Name,
				URL:      b.URL,
				APIKey:   b.APIKey,
				Priority: b.Priority,
				Provider: b.Provider,
			}
		}
		r.SetLLMProxy(relay.NewLLMProxy(backends))
		slog.Info("LLM proxy настроен", "backends", len(backends))
	} else {
		slog.Warn("LLM backends not configured — autonomous tasks (L2) disabled")
	}
	if err := r.Start(); err != nil {
		slog.Error("ошибка запуска реле", "err", err)
		os.Exit(1)
	}
}

// ====================
// Helpers
// ====================

func generateToken(length int) string {
	b := make([]byte, length)
	if _, err := rand.Read(b); err != nil {
		// Fallback
		return fmt.Sprintf("fl-%x", os.Getpid())
	}
	return base64.RawURLEncoding.EncodeToString(b)[:length]
}

func envOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func saveRelayConfig(path string, cfg *config.RelayConfig) error {
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0600)
}

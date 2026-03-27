package main

import (
	"flag"
	"fmt"
	"log/slog"
	"os"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/relay"
	"github.com/braincreator/flowlink/pkg/version"
)

func main() {
	var (
		configPath string
		apiToken   string
		showVer    bool
	)

	flag.StringVar(&configPath, "config", "", "путь к файлу конфигурации")
	flag.StringVar(&apiToken, "api-token", "", "API токен (или через FLOWLINK_API_TOKEN)")
	flag.BoolVar(&showVer, "version", false, "показать версию")
	flag.Parse()

	if showVer {
		fmt.Printf("flowlink-relay %s\n", version.Version)
		return
	}

	// Логирование
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	})))

	// Загрузка конфигурации
	if configPath == "" {
		configPath = "relay.json"
	}

	cfg, err := config.LoadRelayConfig(configPath)
	if err != nil {
		slog.Error("ошибка загрузки конфига", "err", err)
		os.Exit(1)
	}

	// API токен из флага или env
	if apiToken != "" {
		cfg.APIToken = apiToken
	} else if envToken := os.Getenv("FLOWLINK_API_TOKEN"); envToken != "" {
		cfg.APIToken = envToken
	}

	if cfg.APIToken == "" {
		slog.Warn("API токен не задан — HTTP API будет без авторизации")
	}

	slog.Info("запуск flowlink relay",
		"version", version.Version,
		"wss", cfg.WSSAddr,
		"api", cfg.APIAddr,
		"llm_backends", len(cfg.LLMBackends),
	)

	r := relay.NewRelay(cfg)

	// Инициализируем LLM proxy если есть бэкенды
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
		slog.Warn("LLM backends не настроены — автономные задачи (L2) не будут работать")
	}
	if err := r.Start(); err != nil {
		slog.Error("ошибка запуска реле", "err", err)
		os.Exit(1)
	}
}

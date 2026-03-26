package main

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"runtime"
	"syscall"

	"github.com/braincreator/flowlink/internal/agent"
	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/pkg/version"
)

func main() {
	// Парсим аргументы
	var (
		showVersion bool
		initMode    bool
		relayURL    string
		label       string
		approval    string
		verbose     bool
	)

	flag.BoolVar(&showVersion, "version", false, "показать версию")
	flag.BoolVar(&initMode, "init", false, "инициализация (создать конфиг)")
	flag.StringVar(&relayURL, "relay", "", "URL реле (для init)")
	flag.StringVar(&label, "label", "", "имя агента (для init)")
	flag.StringVar(&approval, "approval", "", "режим подтверждения: auto|ask|deny")
	flag.BoolVar(&verbose, "v", false, "verbose логирование")
	flag.Parse()

	// Версия
	if showVersion {
		fmt.Printf("flowlink %s (%s %s)\n", version.Version, runtime.GOOS, runtime.GOARCH)
		fmt.Printf("commit: %s\n", version.GitCommit)
		fmt.Printf("built: %s\n", version.BuildDate)
		return
	}

	// Логирование
	logLevel := slog.LevelInfo
	if verbose {
		logLevel = slog.LevelDebug
	}
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: logLevel}))
	slog.SetDefault(logger)

	// Инициализация
	if initMode {
		err := initConfig(relayURL, label, approval)
		if err != nil {
			logger.Error("ошибка инициализации", "err", err)
			os.Exit(1)
		}
		return
	}

	// Запуск агента
	runAgent()
}

// initConfig — создаёт конфигурацию с генерацией agent_id и токена.
func initConfig(relayURL, label, approval string) error {
	cfg := config.DefaultConfig()

	// Генерируем agent_id
	idBytes := make([]byte, 16)
	if _, err := rand.Read(idBytes); err != nil {
		return fmt.Errorf("генерация agent_id: %w", err)
	}
	cfg.AgentID = hex.EncodeToString(idBytes)

	// Генерируем токен
	tokenBytes := make([]byte, 32)
	if _, err := rand.Read(tokenBytes); err != nil {
		return fmt.Errorf("генерация токена: %w", err)
	}
	cfg.Token = hex.EncodeToString(tokenBytes)

	// Пользовательские параметры
	if relayURL != "" {
		cfg.RelayURL = relayURL
	}
	if label != "" {
		cfg.Label = label
	}
	if approval != "" {
		cfg.Approval.Mode = approval
	}

	// Сохраняем
	if err := config.SaveConfig(&cfg); err != nil {
		return fmt.Errorf("сохранение конфига: %w", err)
	}

	fmt.Println("✅ FlowLink инициализирован")
	fmt.Printf("   Agent ID:  %s\n", cfg.AgentID)
	fmt.Printf("   Token:     %s\n", cfg.Token)
	fmt.Printf("   Реле:      %s\n", cfg.RelayURL)
	fmt.Printf("   Имя:       %s\n", cfg.Label)
	fmt.Printf("   Approval:  %s\n", cfg.Approval.Mode)
	fmt.Println()
	fmt.Println("⚠️  Скопируйте Agent ID и Token и отправьте оператору!")
	fmt.Println("   Конфиг: ~/.flowlink/config.json")
	fmt.Println()
	fmt.Println("Запуск: flowlink agent start")

	return nil
}

// runAgent — запускает агента.
func runAgent() {
	cfg, err := config.LoadConfig()
	if err != nil {
		slog.Error("ошибка загрузки конфига", "err", err)
		fmt.Fprintln(os.Stderr, "Сначала запустите: flowlink agent --init")
		os.Exit(1)
	}

	if cfg.AgentID == "" || cfg.Token == "" {
		fmt.Fprintln(os.Stderr, "Конфиг не инициализирован. Запустите: flowlink agent --init")
		os.Exit(1)
	}

	slog.Info("запуск flowlink агента",
		"version", version.Version,
		"agent", cfg.AgentID,
		"label", cfg.Label,
		"relay", cfg.RelayURL,
	)

	a := agent.NewAgent(cfg)

	// Graceful shutdown
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		sig := <-sigCh
		slog.Info("получен сигнал, завершение...", "signal", sig)
		cancel()
		a.Disconnect()
		os.Exit(0)
	}()

	// Подключаемся к реле
	if err := a.Connect(ctx); err != nil {
		slog.Error("ошибка подключения", "err", err)
		os.Exit(1)
	}

	// Блокируемся
	<-ctx.Done()
}

package main

import (
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"strings"
	"syscall"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/tgbot"
)

func main() {
	// Парсинг аргументов
	configPath := flag.String("config", "", "путь к файлу конфигурации реле")
	relayURL := flag.String("relay", "http://localhost:8080", "URL HTTP API реле")
	apiToken := flag.String("token", "", "токен для HTTP API реле")
	tgToken := flag.String("tg-token", "", "Telegram Bot Token")
	allowedIDs := flag.String("allowed-ids", "", "список разрешённых Telegram ID (через запятую)")
	flag.Parse()

	// Загружаем конфигурацию реле для получения настроек Telegram
	var tgCfg tgbot.TelegramBotConfig

	if *configPath != "" {
		relayCfg, err := config.LoadRelayConfig(*configPath)
		if err != nil {
			slog.Error("ошибка загрузки конфига реле", "err", err)
			os.Exit(1)
		}

		// API токен из конфига если не передан через флаг
		if *apiToken == "" {
			*apiToken = relayCfg.APIToken
		}
		if *relayURL == "http://localhost:8080" && relayCfg.APIAddr != "" {
			*relayURL = "http://localhost" + relayCfg.APIAddr
		}
	}

	// Приоритет: флаги > env > config
	if *tgToken == "" {
		*tgToken = os.Getenv("FLOWLINK_TG_TOKEN")
	}
	if *apiToken == "" {
		*apiToken = os.Getenv("FLOWLINK_API_TOKEN")
	}

	// Формируем конфиг бота
	tgCfg.Token = *tgToken
	if *allowedIDs != "" {
		for _, idStr := range splitCSV(*allowedIDs) {
			var id int64
			if _, err := fmt.Sscanf(idStr, "%d", &id); err == nil {
				tgCfg.AllowedIDs = append(tgCfg.AllowedIDs, id)
			}
		}
	}
	tgCfg.NotifyOn = []string{"exec", "backup", "error", "approval"}

	// Валидация
	if tgCfg.Token == "" {
		slog.Error("не указан Telegram Bot Token (флаг -tg-token или FLOWLINK_TG_TOKEN)")
		os.Exit(1)
	}
	if *apiToken == "" {
		slog.Error("не указан API токен реле (флаг -token или FLOWLINK_API_TOKEN)")
		os.Exit(1)
	}

	// Логгер
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	}))

	// Создаём бота
	bot := tgbot.New(&tgCfg, *relayURL, *apiToken, logger)

	// Graceful shutdown
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigCh
		logger.Info("получен сигнал остановки, завершаем работу...")
		os.Exit(0)
	}()

	// Запуск
	logger.Info("запуск flowlink-bot",
		"relay", *relayURL,
		"allowed_ids", tgCfg.AllowedIDs,
	)

	if err := bot.Start(); err != nil {
		logger.Error("ошибка бота", "err", err)
		os.Exit(1)
	}
}

// splitCSV — разбивает строку по запятым, обрезая пробелы.
func splitCSV(s string) []string {
	var result []string
	for _, part := range strings.Split(s, ",") {
		part = strings.TrimSpace(part)
		if part != "" {
			result = append(result, part)
		}
	}
	return result
}

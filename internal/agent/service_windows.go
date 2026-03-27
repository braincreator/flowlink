//go:build windows

package agent

import (
	"fmt"
	"log/slog"
	"os"
)

// RunAsService — запускает агента как Windows Service.
// На Windows без golang.org/x/svc используется простой режим консольного приложения.
// Для полноценного Windows Service нужен sc.exe или nssm (external).
func RunAsService(name string) error {
	slog.Info("запуск в режиме консольного приложения (Windows)",
		"name", name)

	// Проверяем, запущены ли мы как сервис
	// Windows Service запускается от Service Control Manager
	// Без golang.org/x/sys/windows/svc мы работаем как обычный процесс
	if os.Getenv("FLOWLINK_SERVICE") == "1" {
		slog.Info("работа как Windows Service")
		// Здесь основной цикл работы агента
		select {} // блокировка до завершения
	}

	return fmt.Errorf("для запуска как Windows Service установите через: sc create %s binPath= \"%s\" && sc start %s",
		name, os.Args[0], name)
}

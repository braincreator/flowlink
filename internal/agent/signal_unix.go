//go:build darwin || linux

package agent

import (
	"os"
	"os/signal"
	"syscall"
)

// notifyPlatformSignals — регистрирует обработчики сигналов для Unix-систем.
func notifyPlatformSignals(sigChan chan os.Signal) {
	signal.Notify(sigChan, syscall.SIGTERM, syscall.SIGINT)
}

//go:build windows

package agent

import (
	"os"
	"os/signal"
	"syscall"
)

// notifyPlatformSignals — регистрирует обработчики сигналов для Windows.
// На Windows доступны только SIGINT (Ctrl+C) и SIGKILL.
func notifyPlatformSignals(sigChan chan os.Signal) {
	signal.Notify(sigChan, syscall.SIGINT)
}

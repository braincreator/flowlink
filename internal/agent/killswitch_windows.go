//go:build windows

package agent

import (
	"os"
	"syscall"
	"time"
	"unsafe"
)

var (
	kernel32          = syscall.NewLazyDLL("kernel32.dll")
	getSystemTimes    = kernel32.NewProc("GetSystemTimes")
	getDiskFreeSpaceEx = kernel32.NewProc("GetDiskFreeSpaceExW")
)

// cpuSnapshot — снимок CPU-счётчиков Windows (idle, kernel, user).
type cpuSnapshot struct {
	idle   uint64
	kernel uint64
	user   uint64
}

var lastCPUSnapshot cpuSnapshot
var lastCPUTime     time.Time

// getPlatformCPUUsage — возвращает использование CPU через GetSystemTimes на Windows.
// Вычисляет процент как (1 - idle/total) * 100 между двумя замерами.
func (k *KillSwitch) getPlatformCPUUsage() float64 {
	current := getWindowsCPUTimes()
	now := time.Now()

	// Первый замер — сохраняем и возвращаем 0
	if lastCPUTime.IsZero() {
		lastCPUSnapshot = current
		lastCPUTime = now
		return 0.0
	}

	elapsed := now.Sub(lastCPUTime)
	if elapsed < time.Second {
		// Слишком рано для нового замера
		return 0.0
	}

	idleDelta := current.idle - lastCPUSnapshot.idle
	totalDelta := (current.kernel - lastCPUSnapshot.kernel) + (current.user - lastCPUSnapshot.user)

	lastCPUSnapshot = current
	lastCPUTime = now

	if totalDelta == 0 {
		return 0.0
	}

	// Использование CPU = 100% - idle%
	usage := float64(totalDelta-idleDelta) / float64(totalDelta) * 100.0
	return usage
}

// getWindowsCPUTimes — получает CPU-счётчики через GetSystemTimes.
func getWindowsCPUTimes() cpuSnapshot {
	var idle, kernel, user syscall.Filetime

	getSystemTimes.Call(
		uintptr(unsafe.Pointer(&idle)),
		uintptr(unsafe.Pointer(&kernel)),
		uintptr(unsafe.Pointer(&user)),
	)

	return cpuSnapshot{
		idle:   filetimeToUint64(idle),
		kernel: filetimeToUint64(kernel),
		user:   filetimeToUint64(user),
	}
}

// filetimeToUint64 — конвертирует syscall.Filetime в uint64.
func filetimeToUint64(ft syscall.Filetime) uint64 {
	return uint64(ft.HighDateTime)<<32 + uint64(ft.LowDateTime)
}

// getPlatformDiskUsage — возвращает процент использования диска через GetDiskFreeSpaceExW на Windows.
func (k *KillSwitch) getPlatformDiskUsage() float64 {
	home, _ := os.UserHomeDir()
	if home == "" {
		home = "C:\\"
	}

	// GetDiskFreeSpaceExW принимает: directory, freeBytesAvailable, totalBytes, freeBytes
	var freeAvailable, totalBytes, freeBytes uint64

	path, err := syscall.UTF16PtrFromString(home)
	if err != nil {
		return 0.0
	}

	ret, _, _ := getDiskFreeSpaceEx.Call(
		uintptr(unsafe.Pointer(path)),
		uintptr(unsafe.Pointer(&freeAvailable)),
		uintptr(unsafe.Pointer(&totalBytes)),
		uintptr(unsafe.Pointer(&freeBytes)),
	)

	if ret == 0 {
		return 0.0
	}

	if totalBytes == 0 {
		return 0.0
	}

	used := totalBytes - freeAvailable
	return float64(used) / float64(totalBytes) * 100
}

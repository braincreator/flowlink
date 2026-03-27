//go:build darwin

package agent

import (
	"os"
	"syscall"
)

// getPlatformCPUUsage — возвращает load average через syscall на macOS.
func (k *KillSwitch) getPlatformCPUUsage() float64 {
	loadAvg, err := syscall.SysctlUint32("vm.loadavg")
	if err != nil {
		return 0.0
	}
	return float64(loadAvg)
}

// getPlatformDiskUsage — возвращает процент использования диска через Statfs на macOS.
func (k *KillSwitch) getPlatformDiskUsage() float64 {
	var stat syscall.Statfs_t
	home, _ := os.UserHomeDir()
	if err := syscall.Statfs(home, &stat); err != nil {
		return 0.0
	}

	total := stat.Blocks * uint64(stat.Bsize)
	free := stat.Bavail * uint64(stat.Bsize)
	used := total - free

	if total == 0 {
		return 0.0
	}

	return float64(used) / float64(total) * 100
}

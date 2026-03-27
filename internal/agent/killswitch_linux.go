//go:build linux

package agent

import (
	"os"
	"strconv"
	"strings"
	"syscall"
)

// getPlatformCPUUsage — возвращает load average через /proc/loadavg на Linux.
func (k *KillSwitch) getPlatformCPUUsage() float64 {
	data, err := os.ReadFile("/proc/loadavg")
	if err != nil {
		return 0.0
	}
	fields := strings.Fields(string(data))
	if len(fields) < 2 {
		return 0.0
	}
	load1, err := strconv.ParseFloat(fields[0], 64)
	if err != nil {
		return 0.0
	}
	return load1
}

// getPlatformDiskUsage — возвращает процент использования диска через Statfs на Linux.
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

//go:build linux

package agent

import (
	"os"
	"strconv"
	"strings"
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

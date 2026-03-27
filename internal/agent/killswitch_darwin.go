//go:build darwin

package agent

import "syscall"

// getPlatformCPUUsage — возвращает load average через syscall на macOS.
func (k *KillSwitch) getPlatformCPUUsage() float64 {
	loadAvg, err := syscall.SysctlUint32("vm.loadavg")
	if err != nil {
		return 0.0
	}
	return float64(loadAvg)
}

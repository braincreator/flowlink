# eBPF Requirements for FlowLink ServerGuard

This document describes the kernel requirements, configuration, and fallback behavior for eBPF-based process monitoring in FlowLink ServerGuard.

## Overview

ServerGuard supports three monitoring modes:
- **eBPF mode**: Kernel-level process interception (Linux only, requires BPF_LSM support)
- **Userspace mode**: Process monitoring via `/proc` polling (Linux, works on all kernels)
- **Hybrid mode**: Uses eBPF when available, falls back to userspace when not

## Kernel Requirements

### Minimum Kernel Version

**5.8+** (release date: June 2020)

The following kernel features were introduced in Linux 5.8:
- BPF LSM (Linux Security Modules) for eBPF security contexts
- BPF helpers for tracing system calls

### Required Kernel Config Flags

The following kernel configuration options must be enabled:

```bash
# Check current config
grep CONFIG_BPF /boot/config-$(uname -r)
```

**Required flags:**
```
CONFIG_BPF=y
CONFIG_BPF_SYSCALL=y
CONFIG_BPF_JIT=y
CONFIG_BPF_LSM=y
CONFIG_BPF_PRELOAD=y
```

**Optional but recommended:**
```
CONFIG_BPF_STREAM_PARSER=y
CONFIG_BPF_ANNOTATE_KERN_PROGS=y
CONFIG_BPF_CGROUP_DEVICE=y
CONFIG_BPF_CGROUP_SYSCTL=y
```

### Enabling on Ubuntu/Debian

```bash
# Check current kernel version
uname -r  # Should be >= 5.8

# Update to latest kernel (Ubuntu 20.04+)
sudo apt update
sudo apt install linux-generic-hwe-20.04

# Or install a newer kernel if needed
sudo apt install linux-generic-hwe-22.04
```

Reboot after kernel update.

**Verify kernel config:**
```bash
zcat /proc/config.gz | grep CONFIG_BPF_LSM
```

### Enabling on RHEL/CentOS/Rocky/Alma

```bash
# Check current kernel version
uname -r  # Should be >= 5.8

# Install kernel-ml (mainline) from EPEL or ELRepo
sudo yum install kernel-ml kernel-ml-modules

# Or install EPEL repo first if not present
sudo yum install epel-release
sudo yum update
```

**Verify kernel config:**
```bash
cat /boot/config-$(uname -r) | grep CONFIG_BPF_LSM
```

### Enabling on Fedora

```bash
# Fedora ships with recent kernels (>= 5.8 by default)
# Just ensure BPF features are enabled

sudo dnf install kernel-core
# Reboot if needed
sudo reboot
```

## Feature Flags

### Compile-time Feature

eBPF support is behind the `ebpf` feature flag:

```toml
# In flowlink-gitops/Cargo.toml
[features]
default = []
ebpf = ["aya", "aya-executor"]
```

Build with:
```bash
cargo build --features ebpf
```

### Runtime Detection

ServerGuard automatically detects eBPF support at runtime:

```rust
// In server_guard/mod.rs
fn check_ebpf_support() -> bool {
    // Check for kernel version >= 5.8
    // Check for CONFIG_BPF_LSM
    // Try to load a minimal BPF program
}
```

**Fallback behavior:**
```
eBPF not available → "eBPF LSM not available, falling back to userspace monitoring"
Mode → userspace-only
```

## Monitoring Modes

### 1. Userspace Mode (Default)

Uses `/proc` polling for process monitoring:

- **Pros**: Works on all Linux kernels, no root required for basic use
- **Cons**: Higher CPU usage, can miss fast-changing states
- **Event rate**: ~10-100ms polling interval
- **Kernel version**: Any

**Configuration:**
```json
{
  "monitor_mode": "userspace",
  "poll_interval_ms": 100
}
```

### 2. eBPF Mode

Kernel-level process interception via LSM:

- **Pros**: Real-time detection, minimal overhead, no missed events
- **Cons**: Requires kernel >= 5.8, BPF_LSM support, root capabilities
- **Event rate**: Near-instant, zero idle overhead
- **Kernel version**: 5.8+

**Configuration:**
```json
{
  "monitor_mode": "ebpf",
  "ebpf_program_path": "/usr/local/lib/flowlink/monitor.bpf.o",
  "ring_buffer_pages": 64
}
```

### 3. Hybrid Mode (Recommended)

Automatic fallback based on kernel support:

- **Pros**: Best of both worlds, automatic adaptation
- **Cons**: Slightly more complex runtime logic
- **Fallback trigger**: eBPF load failure, kernel version check
- **Event rate**: Instant (eBPF) or 100ms (fallback)

**Configuration:**
```json
{
  "monitor_mode": "hybrid"
}
```

## Building eBPF Programs

### Required Dependencies

```bash
# Install BPF toolchain
sudo apt install clang llvm libelf-dev libbpf-dev bpfcc-tools linux-headers-$(uname -r)

# Or on RHEL/CentOS
sudo yum install clang llvm libelf-devel bpftool kernel-devel
```

### Building the BPF Program

```bash
# Navigate to shield crate
cd crates/shield

# Compile eBPF C to ELF
clang -target bpf -g -O2 -c ebpf/execve.bpf.c -o ebpf/execve.bpf.o

# Verify ELF structure
llvm-objdump -h ebpf/execve.bpf.o
```

### Embedding in Rust

```rust
// In shield/src/ebpf.rs
let bpf = Bpf::load(include_bytes!("../../../bpf/execve.bpf.o"))?;
```

## Troubleshooting

### eBPF Load Failures

**Symptom:** `Failed to load BPF from path: Permission denied`

**Solutions:**
1. Run with sudo/root: `sudo flowlink-guard`
2. Enable BPF in kernel: Check `CONFIG_BPF_LSM=y`
3. Verify capabilities: `capsh --print | grep BPF`

**Symptom:** `Failed to load BPF: Operation not permitted`

**Solutions:**
1. Verify kernel >= 5.8
2. Check BPF JIT support: `grep CONFIG_BPF_JIT /boot/config-$(uname -r)`
3. Ensure CAP_BPF capability: `sudo setcap cap_bpf+ep /usr/bin/flowlink-guard`

**Symptom:** `eBPF LSM not available`

**Solutions:**
1. Update kernel to 5.8+
2. Enable BPF_LSM in kernel config
3. Reboot after config change

### Userspace Mode Issues

**Symptom:** Missing process events

**Solutions:**
1. Check `/proc` permissions: `ls -la /proc/1/exe`
2. Verify poll interval: Increase if system is busy
3. Check file watcher path: Ensure path exists and is accessible

## Performance Characteristics

### CPU Usage

| Mode | Idle | Active (1 event/sec) | Active (10 events/sec) |
|------|------|---------------------|------------------------|
| Userspace | <1% | ~2% | ~10% |
| eBPF | <1% | ~1% | ~2% |

### Memory Usage

| Mode | RSS |
|------|-----|
| Userspace | ~20MB |
| eBPF | ~25MB |

### Event Latency

| Mode | P99 latency |
|------|-------------|
| Userspace | ~100-200ms |
| eBPF | <10ms |

## Security Considerations

### Capabilities Required

For eBPF mode:
- `CAP_BPF` (Linux 5.8+)
- `CAP_SYS_ADMIN` (optional, for tracepoint attachment)

For userspace mode:
- None (read-only access to `/proc`)

### Sandboxing

eBPF programs run in a restricted kernel sandbox:
- No direct memory access to user-space
- Limited helper functions
- LSM-based security context enforcement

Userspace fallback provides:
- Read-only `/proc` access
- No kernel-level modifications

## Testing

### Local Testing

```bash
# Run e2e tests (requires root for eBPF)
cargo test --test e2e_guard --features ebpf

# Run without eBPF
cargo test --test e2e_guard
```

### VPS Testing

```bash
# Build Linux binary
cross build --release --target x86_64-unknown-linux-musl -p flowlink

# Upload to VPS
scp target/x86_64-unknown-linux-musl/release/flowlink user@93.93.207.44:~/

# Run on VPS
ssh root@93.93.207.44
sudo ./flowlink-guard --config /etc/flowlink/guard.json
```

### Verify eBPF Support

```bash
# Check kernel version
uname -r  # Should be >= 5.8

# Check kernel config
zcat /proc/config.gz | grep CONFIG_BPF_LSM
# Should output: CONFIG_BPF_LSM=y

# Try loading BPF program
sudo flowlink-guard --debug
# Look for "eBPF LSM not available" or "eBPF monitor loaded successfully"
```

## References

- [Linux 5.8 Release Notes](https://kernelnewbies.org/Linux_5.8)
- [BPF Documentation](https://www.kernel.org/doc/html/latest/bpf/)
- [BPF LSM](https://docs.kernel.org/lsm/index.html#bpf)
- [Aya eBPF Framework](https://github.com/aya-rs/aya)
- [BCC Tools](https://github.com/iovisor/bcc)

## Appendix: Kernel Config Examples

### Ubuntu 22.04 (LTS) Kernel Config
```
CONFIG_BPF=y
CONFIG_BPF_SYSCALL=y
CONFIG_BPF_JIT=y
CONFIG_BPF_LSM=y
CONFIG_BPF_PRELOAD=y
CONFIG_BPF_STREAM_PARSER=y
CONFIG_BPF_ANNOTATE_KERN_PROGS=y
```

### RHEL 9 Kernel Config
```
CONFIG_BPF=y
CONFIG_BPF_SYSCALL=y
CONFIG_BPF_JIT=y
CONFIG_BPF_LSM=y
CONFIG_BPF_PRELOAD=y
CONFIG_BPF_STREAM_PARSER=y
```

### Minimal Config (For Testing)
```
CONFIG_BPF=y
CONFIG_BPF_SYSCALL=y
CONFIG_BPF_JIT=y
CONFIG_BPF_LSM=y
```

---

**Last Updated:** 2026-04-27
**Maintained by:** FlowLink Core Team

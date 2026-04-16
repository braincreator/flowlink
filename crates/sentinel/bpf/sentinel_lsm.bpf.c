// FlowLink Sentinel — LSM BPF programs for kernel-level blocking
//
// Uses BPF_PROG macro from bpf_tracing.h for proper LSM argument access.
// Requires: CONFIG_BPF_LSM=y, kernel >= 5.7, "bpf" in /sys/kernel/security/lsm
// Compile: clang -O2 -g -target bpf -D__TARGET_ARCH_x86 -I<bpf_headers> -c sentinel_lsm.bpf.c
//
// PORTABILITY: vmlinux.h is generated at build time from /sys/kernel/btf/vmlinux.
// The BPF_PROG macro + CO-RE ensures compatibility across kernel versions.

#ifdef HAS_VMLINUX
#include "vmlinux_local.h"
#else
#error "vmlinux.h required for LSM BPF — generate with: bpftool btf dump file /sys/kernel/btf/vmlinux format c > vmlinux_local.h"
#endif

// We need bpf_helpers.h and bpf_tracing.h from libbpf for BPF_PROG macro
// These are bundled in the bpf_headers/ directory at build time
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

char LICENSE[] SEC("license") = "GPL";

#define MAX_COMM_LEN 64

// Command blocklist map: key=command name (64 bytes), value=action (1=block)
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 4096);
    __type(key, char[MAX_COMM_LEN]);
    __type(value, __u32);
} blocked_commands SEC(".maps");

// Path protection map (reserved for future use)
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 4096);
    __type(key, char[128]);
    __type(value, __u32);
} protected_paths SEC(".maps");

// Per-PID whitelist — these PIDs bypass all blocking
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1024);
    __type(key, __u32);
    __type(value, __u32);
} whitelist_pids SEC(".maps");

// Per-PID blocklist — these PIDs are blocked from everything
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 65536);
    __type(key, __u32);
    __type(value, __u32);
} blocked_pids SEC(".maps");

// LSM: bprm_check_security — block dangerous commands at exec time
SEC("lsm/bprm_check_security")
int BPF_PROG(block_exec, struct linux_binprm *bprm)
{
    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u32 pid = pid_tgid >> 32;

    // Check whitelist
    __u32 *wv = bpf_map_lookup_elem(&whitelist_pids, &pid);
    if (wv && *wv == 1) return 0;

    // Check per-PID block
    __u32 *pv = bpf_map_lookup_elem(&blocked_pids, &pid);
    if (pv && *pv == 1) return -1;

    // Read bprm->filename into local buffer
    char fname[MAX_COMM_LEN] = {};
    const char *fn = bprm->filename;
    if (!fn) return 0;
    bpf_probe_read_kernel_str(fname, sizeof(fname), fn);

    if (fname[0] == '\0') return 0;

    // Check full path in blocked_commands
    __u32 *cv = bpf_map_lookup_elem(&blocked_commands, fname);
    if (cv && *cv == 1) return -1;

    // Extract basename (after last '/')
    char base[MAX_COMM_LEN] = {};
    int last_slash = -1;
    #pragma unroll
    for (int i = 0; i < MAX_COMM_LEN - 1; i++) {
        if (fname[i] == '/') last_slash = i;
        if (fname[i] == '\0') break;
    }
    int base_start = (last_slash >= 0) ? last_slash + 1 : 0;
    #pragma unroll
    for (int i = 0; i < MAX_COMM_LEN - 1; i++) {
        int src = base_start + i;
        if (src >= MAX_COMM_LEN || fname[src] == '\0') break;
        base[i] = fname[src];
    }

    // Check basename in blocked_commands
    __u32 *bv = bpf_map_lookup_elem(&blocked_commands, base);
    if (bv && *bv == 1) return -1;

    return 0;
}

// LSM: file_open — pass-through (reserved for path protection)
SEC("lsm/file_open")
int BPF_PROG(block_file_open, struct file *file) { return 0; }

// LSM: inode_unlink — pass-through (reserved for file deletion monitoring)
SEC("lsm/inode_unlink")
int BPF_PROG(monitor_unlink, struct inode *dir, struct dentry *dentry) { return 0; }

// LSM: socket_bind — block network for blocked PIDs
SEC("lsm/socket_bind")
int BPF_PROG(monitor_bind, struct socket *sock)
{
    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u32 pid = pid_tgid >> 32;

    __u32 *wv = bpf_map_lookup_elem(&whitelist_pids, &pid);
    if (wv && *wv == 1) return 0;

    __u32 *pv = bpf_map_lookup_elem(&blocked_pids, &pid);
    if (pv && *pv == 1) return -1;

    return 0;
}

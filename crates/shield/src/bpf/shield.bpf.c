// SPDX-License-Identifier: GPL-2.0
// FlowLink Shield — eBPF kernel-level L1 interceptor
// Hooks sys_enter_execve, SIGSTOPs dangerous processes before they execute.

#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct event {
    u32 pid;
    u32 ppid;
    u32 uid;
    char comm[64];
    char args[256];
    int signal_sent;  // 1 = SIGSTOP sent
};

struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 256 * 1024);
} events SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 64);
    __uint(key_size, sizeof(u32));
    __uint(value_size, sizeof(u32));
} allowed_uids SEC(".maps");

// Dangerous pattern: binary name + check flags
struct dangerous_pattern {
    char binary[32];
    u8 check_args;     // 1 = check args for dangerous flags
    u8 check_paths;    // 1 = check args for dangerous paths
};

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 32);
    __uint(key_size, sizeof(u32));
    __uint(value_size, sizeof(struct dangerous_pattern));
} patterns SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(struct trace_event_raw_sys_enter *ctx)
{
    u32 uid = bpf_get_current_uid_gid();

    // Skip allowed UIDs
    u32 *allowed = bpf_map_lookup_elem(&allowed_uids, &uid);
    if (allowed)
        return 0;

    // Read filename
    const char *filename = (const char *)ctx->args[0];
    char comm[64] = {};
    bpf_probe_read_user_str(comm, sizeof(comm), filename);

    // Check against dangerous patterns
    for (int i = 0; i < 32; i++) {
        u32 key = i;
        struct dangerous_pattern *p = bpf_map_lookup_elem(&patterns, &key);
        if (!p)
            break;
        if (p->binary[0] == '\0')
            break;

        // Prefix match on binary name (basename — skip path prefix)
        int match = 0;
        int start = 0;
        // Find last '/' to get basename
        for (int s = 63; s >= 0; s--) {
            if (comm[s] == '/') {
                start = s + 1;
                break;
            }
        }
        // Simple prefix match
        match = 1;
        for (int j = 0; j < 32 && p->binary[j] != '\0'; j++) {
            if (comm[start + j] != p->binary[j]) {
                match = 0;
                break;
            }
        }

        if (match) {
            // Read args for further checking
            char args[256] = {};
            const char **argv = (const char **)ctx->args[1];
            int offset = 0;
            for (int k = 1; k < 16 && offset < 200; k++) {
                const char *arg = NULL;
                bpf_probe_read_user(&arg, sizeof(arg), &argv[k]);
                if (!arg)
                    break;
                int len = bpf_probe_read_user_str(args + offset,
                                                  256 - offset, arg);
                if (len <= 0)
                    break;
                offset += len;
                args[offset - 1] = ' ';  // space separator
            }

            int should_stop = 1;

            if (p->check_args) {
                // Require dangerous flags (-rf, -fr, --force, -f alone)
                should_stop = 0;
                for (int j = 0; j < 250; j++) {
                    if (args[j] == '-' && args[j+1] == 'r' && args[j+2] == 'f')
                        should_stop = 1;
                    if (args[j] == '-' && args[j+1] == 'f' && args[j+2] == 'r')
                        should_stop = 1;
                    if (args[j] == '-' && args[j+1] == '-' &&
                        args[j+2] == 'f' && args[j+3] == 'o' &&
                        args[j+4] == 'r' && args[j+5] == 'c' &&
                        args[j+6] == 'e')
                        should_stop = 1;
                }
            }

            if (p->check_paths) {
                // Check for dangerous paths: / or /* or /dev/
                if (!should_stop) {
                    for (int j = 0; j < 250; j++) {
                        if (args[j] == 'o' && args[j+1] == 'f' &&
                            args[j+2] == '=' && args[j+3] == '/' &&
                            args[j+4] == 'd' && args[j+5] == 'e' &&
                            args[j+6] == 'v' && args[j+7] == '/')
                            should_stop = 1;
                        if (args[j] == '/' &&
                            (args[j+1] == '\0' || args[j+1] == ' '))
                            should_stop = 1;
                        if (args[j] == '/' && args[j+1] == '*')
                            should_stop = 1;
                    }
                }
            }

            if (should_stop) {
                // FREEZE PROCESS BEFORE EXECVE COMPLETES
                bpf_send_signal(19);  // SIGSTOP

                // Send event to userspace
                struct event *e = bpf_ringbuf_reserve(&events,
                                                       sizeof(*e), 0);
                if (e) {
                    e->pid = bpf_get_current_pid_tgid() >> 32;
                    e->ppid = 0;  // filled by userspace
                    e->uid = uid;
                    __builtin_memcpy(e->comm, comm, sizeof(e->comm));
                    __builtin_memcpy(e->args, args, sizeof(e->args));
                    e->signal_sent = 1;
                    bpf_ringbuf_submit(e, 0);
                }
                return 0;
            }
        }
    }
    return 0;
}

char LICENSE[] SEC("license") = "GPL";

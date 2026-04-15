// FlowLink Sentinel — eBPF kernel programs
// Compile: clang -O2 -g -target bpf -nostdinc -c sentinel.bpf.c -o sentinel.bpf.o \
//          -I/usr/include/x86_64-linux-gnu -I/usr/include/linux

// Minimal type definitions for BPF (avoid glibc headers)
typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long long uint64_t;

// BPF map definition helpers
#define __uint(name, val) int (*name)[val]
#define __type(name, val) typeof(val) *name
#define SEC(name) __attribute__((section(name), used))

// BPF helpers we use - declare as extern to avoid needing libbpf headers
#define __always_inline __attribute__((always_inline))
#define NULL ((void *)0)

static void *(*const bpf_map_lookup_elem)(void *map, const void *key) = (void *)1;
static long (*const bpf_perf_event_output)(void *ctx, void *map, uint64_t flags, void *data, uint64_t size) = (void *)25;
static uint64_t (*const bpf_get_current_pid_tgid)(void) = (void *)14;
static uint64_t (*const bpf_get_current_uid_gid)(void) = (void *)15;
static void *(*const bpf_get_current_task)(void) = (void *)17;
static long (*const bpf_probe_read_user_str)(void *dst, uint32_t size, const void *ptr) = (void *)112;
static long (*const bpf_probe_read_user)(void *dst, uint32_t size, const void *ptr) = (void *)113;
static long (*const bpf_probe_read_kernel)(void *dst, uint32_t size, const void *ptr) = (void *)114;

#define BPF_F_CURRENT_CPU 0xffffffffULL

#ifndef AF_INET
#define AF_INET 2
#endif
#ifndef AF_INET6
#define AF_INET6 10
#endif

// Reduced sizes to fit BPF constraints
#define MAX_COMM_LEN 64
#define MAX_ARGS 4
#define MAX_ARG_LEN 64
#define MAX_PATH_LEN 128

struct event {
    uint32_t event_type;
    uint32_t pid;
    uint32_t ppid;
    uint32_t uid;
    uint32_t gid;
    char comm[MAX_COMM_LEN];
    char args[MAX_ARGS][MAX_ARG_LEN];
    uint32_t args_count;
    char path[MAX_PATH_LEN];
    uint16_t port;
    uint32_t flags;
};

// Per-CPU array for event storage (avoids stack limit)
struct {
    __uint(type, 1);  // BPF_MAP_TYPE_PERCPU_ARRAY
    __uint(max_entries, 1);
    __uint(key_size, sizeof(uint32_t));
    __uint(value_size, sizeof(struct event));
} event_storage SEC(".maps");

// Perf event array for sending events to userspace
struct {
    __uint(type, 4);  // BPF_MAP_TYPE_PERF_EVENT_ARRAY
    __uint(key_size, sizeof(uint32_t));
    __uint(value_size, sizeof(uint32_t));
} events SEC(".maps");

// Helper to get zeroed event from per-CPU map
static __always_inline struct event *get_event(void) {
    uint32_t key = 0;
    struct event *e = bpf_map_lookup_elem(&event_storage, &key);
    if (!e) return NULL;
    __builtin_memset(e, 0, sizeof(*e));
    return e;
}

// tracepoint context (simplified)
struct trace_event_raw_sys_enter {
    unsigned short common_type;
    unsigned char common_flags;
    unsigned char common_preempt_count;
    int common_pid;
    long unsigned int args[6];
};

// ── execve ──────────────────────────────────────────────────────────────────

SEC("tracepoint/syscalls/sys_enter_execve")
int trace_execve(struct trace_event_raw_sys_enter *ctx)
{
    struct event *e = get_event();
    if (!e) return 0;

    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    uint64_t uid_gid  = bpf_get_current_uid_gid();

    e->event_type = 0;
    e->pid = pid_tgid >> 32;
    e->uid = (uint32_t)uid_gid;
    e->gid = (uint32_t)(uid_gid >> 32);

    // Get ppid from task_struct
    void *task = bpf_get_current_task();
    if (task) {
        // real_parent is at a known offset in task_struct
        // We read task->real_parent->tgid
        void *real_parent;
        // real_parent is at offset 0x7a0 on x86_64 for 5.15 (approximate)
        // Safer: just use parent pointer offset - for now skip if uncertain
        bpf_probe_read_kernel(&real_parent, sizeof(real_parent), (char *)task + 0x7a0);
        bpf_probe_read_kernel(&e->ppid, sizeof(e->ppid), (char *)real_parent + 0x3c8);
    }

    const char *filename = (const char *)ctx->args[0];
    bpf_probe_read_user_str(&e->comm, sizeof(e->comm), filename);

    const char *const *argv = (const char *const *)ctx->args[1];
    #pragma unroll
    for (int i = 0; i < MAX_ARGS; i++) {
        const char *arg = NULL;
        bpf_probe_read_user(&arg, sizeof(arg), &argv[i]);
        if (!arg) break;
        bpf_probe_read_user_str(&e->args[i], MAX_ARG_LEN, arg);
        e->args_count++;
    }

    bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, e, sizeof(*e));
    return 0;
}

// ── openat (write only) ────────────────────────────────────────────────────

SEC("tracepoint/syscalls/sys_enter_openat")
int trace_openat(struct trace_event_raw_sys_enter *ctx)
{
    int flags = (int)ctx->args[2];
    if (!(flags & 3)) return 0;

    struct event *e = get_event();
    if (!e) return 0;

    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    e->event_type = 1;
    e->pid = pid_tgid >> 32;
    e->flags = (uint32_t)flags;

    const char *pathname = (const char *)ctx->args[1];
    bpf_probe_read_user_str(&e->path, sizeof(e->path), pathname);

    if (e->path[0] == '/' && e->path[1] != '\0') {
        char c = e->path[1];
        if (c == 'e' || c == 'v' || c == 'u' || c == 'b' ||
            c == 's' || c == 'p' || c == 'd' || c == 'r' || c == 'o') {
            bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, e, sizeof(*e));
        }
    }

    return 0;
}

// ── connect ────────────────────────────────────────────────────────────────

SEC("tracepoint/syscalls/sys_enter_connect")
int trace_connect(struct trace_event_raw_sys_enter *ctx)
{
    struct event *e = get_event();
    if (!e) return 0;

    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    e->event_type = 2;
    e->pid = pid_tgid >> 32;

    struct sockaddr { uint16_t sa_family; char sa_data[14]; };
    struct sockaddr_in { uint16_t sin_family; uint16_t sin_port; uint32_t sin_addr; char pad[8]; };
    struct sockaddr_in6 { uint16_t sin6_family; uint16_t sin6_port; uint32_t sin6_flowinfo; uint8_t sin6_addr[16]; uint32_t sin6_scope_id; };

    struct sockaddr *addr = (struct sockaddr *)ctx->args[1];
    uint16_t family;
    bpf_probe_read_user(&family, sizeof(family), &addr->sa_family);

    if (family == AF_INET) {
        struct sockaddr_in sin;
        bpf_probe_read_user(&sin, sizeof(sin), addr);
        e->port = sin.sin_port;
        uint32_t ip = sin.sin_addr;
        int off = 0;
        #pragma unroll
        for (int i = 0; i < 4; i++) {
            uint8_t octet = (ip >> (i * 8)) & 0xFF;
            if (octet >= 100) { e->path[off++] = '0' + octet/100; octet %= 100; }
            if (octet >= 10) { e->path[off++] = '0' + octet/10; octet %= 10; }
            e->path[off++] = '0' + octet;
            if (i < 3) e->path[off++] = '.';
        }
    } else if (family == AF_INET6) {
        struct sockaddr_in6 sin6;
        bpf_probe_read_user(&sin6, sizeof(sin6), addr);
        e->port = sin6.sin6_port;
        __builtin_memcpy(&e->path, &sin6.sin6_addr, 16);
    } else {
        return 0;
    }

    bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, e, sizeof(*e));
    return 0;
}

// ── bind ───────────────────────────────────────────────────────────────────

SEC("tracepoint/syscalls/sys_enter_bind")
int trace_bind(struct trace_event_raw_sys_enter *ctx)
{
    struct event *e = get_event();
    if (!e) return 0;

    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    e->event_type = 3;
    e->pid = pid_tgid >> 32;

    struct sockaddr { uint16_t sa_family; char sa_data[14]; };
    struct sockaddr_in { uint16_t sin_family; uint16_t sin_port; uint32_t sin_addr; char pad[8]; };

    struct sockaddr *addr = (struct sockaddr *)ctx->args[1];
    uint16_t family;
    bpf_probe_read_user(&family, sizeof(family), &addr->sa_family);

    if (family == AF_INET || family == AF_INET6) {
        struct sockaddr_in sin;
        bpf_probe_read_user(&sin, sizeof(sin), addr);
        e->port = sin.sin_port;
        bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, e, sizeof(*e));
    }

    return 0;
}

// ── unlinkat ───────────────────────────────────────────────────────────────

SEC("tracepoint/syscalls/sys_enter_unlinkat")
int trace_unlinkat(struct trace_event_raw_sys_enter *ctx)
{
    struct event *e = get_event();
    if (!e) return 0;

    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    e->event_type = 4;
    e->pid = pid_tgid >> 32;

    const char *pathname = (const char *)ctx->args[1];
    bpf_probe_read_user_str(&e->path, sizeof(e->path), pathname);

    if (e->path[0] == '/' && e->path[1] != '\0') {
        char c = e->path[1];
        if (c == 'e' || c == 'v' || c == 'u' || c == 'b' ||
            c == 's' || c == 'p' || c == 'd') {
            bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, e, sizeof(*e));
        }
    }

    return 0;
}

// ── mount ──────────────────────────────────────────────────────────────────

SEC("tracepoint/syscalls/sys_enter_mount")
int trace_mount(struct trace_event_raw_sys_enter *ctx)
{
    struct event *e = get_event();
    if (!e) return 0;

    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    e->event_type = 5;
    e->pid = pid_tgid >> 32;

    const char *dev_name = (const char *)ctx->args[0];
    bpf_probe_read_user_str(&e->path, sizeof(e->path), dev_name);

    bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, e, sizeof(*e));
    return 0;
}

char LICENSE[] SEC("license") = "GPL";

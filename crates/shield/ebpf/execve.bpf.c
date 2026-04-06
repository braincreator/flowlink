// FlowLink Shield — eBPF program for execve() interception
// Loaded into kernel via aya. Intercepts ALL execve calls.
// Sends command + args to userspace for rule matching.

#include <linux/types.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#define MAX_COMM_LEN 64
#define MAX_ARGS_LEN 512
#define MAX_ENV_LEN  128

struct exec_event {
    __u32 pid;
    __u32 ppid;
    __u32 uid;
    __u32 gid;
    char comm[MAX_COMM_LEN];
    char args[MAX_ARGS_LEN];
};

struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 1 << 24); // 16 MB ring buffer
} events SEC(".maps");

// Tracepoint: sched/sched_process_exec
// Fires AFTER execve succeeds — we check the command and can SIGSTOP
SEC("tracepoint/sched/sched_process_exec")
int trace_exec(struct trace_event_raw_sched_process_template *ctx)
{
    struct exec_event *e;
    
    e = bpf_ringbuf_reserve(&events, sizeof(*e), 0);
    if (!e)
        return 0;

    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u64 uid_gid = bpf_get_current_uid_gid();

    e->pid = pid_tgid >> 32;
    e->ppid = 0; // Will be filled by userspace via /proc
    e->uid = uid_gid;
    e->gid = uid_gid >> 32;

    __builtin_memset(e->comm, 0, sizeof(e->comm));
    __builtin_memset(e->args, 0, sizeof(e->args));

    bpf_get_current_comm(&e->comm, sizeof(e->comm));

    // Try to read args from the first arg of execve
    // Note: in tracepoint context, we read from task->mm->arg_start
    // For simplicity, comm is enough — userspace reads /proc/pid/cmdline
    
    bpf_ringbuf_submit(e, 0);

    return 0;
}

char _license[] SEC("license") = "GPL";

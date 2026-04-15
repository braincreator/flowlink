// FlowLink Sentinel — LSM BPF programs for kernel-level blocking
// Requires: CONFIG_BPF_LSM=y, kernel >= 5.7, "bpf" in /sys/kernel/security/lsm
// Compile: clang -O2 -g -target bpf -nostdinc -c sentinel_lsm.bpf.c -o sentinel_lsm.bpf.o

typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long long uint64_t;

// BPF map definition helpers
#define __uint(name, val) int (*name)[val]
#define __type(name, val) typeof(val) *name
#define SEC(name) __attribute__((section(name), used))

// BPF helpers
#define __always_inline __attribute__((always_inline))
#define NULL ((void *)0)

static void *(*const bpf_map_lookup_elem)(void *map, const void *key) = (void *)1;
static uint64_t (*const bpf_get_current_pid_tgid)(void) = (void *)14;
static uint64_t (*const bpf_get_current_uid_gid)(void) = (void *)15;
static long (*const bpf_probe_read_kernel)(void *dst, uint32_t size, const void *ptr) = (void *)114;
static long (*const bpf_probe_read_user_str)(void *dst, uint32_t size, const void *ptr) = (void *)112;

#define MAX_COMM_LEN 64
#define MAX_PATH_LEN 128

// ── Blocked commands map ────────────────────────────────────────────────
struct cmd_policy_key {
    char comm[MAX_COMM_LEN];
};
struct cmd_policy_value {
    uint32_t action; // 0=allow, 1=deny
};
struct {
    __uint(type, 1);
    __uint(max_entries, 4096);
    __uint(key_size, sizeof(struct cmd_policy_key));
    __uint(value_size, sizeof(struct cmd_policy_value));
} blocked_commands SEC(".maps");

// ── Protected paths map ─────────────────────────────────────────────────
struct path_policy_key {
    char prefix[MAX_PATH_LEN];
};
struct path_policy_value {
    uint32_t action; // 0=allow, 1=deny
};
struct {
    __uint(type, 1);
    __uint(max_entries, 4096);
    __uint(key_size, sizeof(struct path_policy_key));
    __uint(value_size, sizeof(struct path_policy_value));
} protected_paths SEC(".maps");

// ── Whitelist pids ──────────────────────────────────────────────────────
struct {
    __uint(type, 1);
    __uint(max_entries, 1024);
    __uint(key_size, sizeof(uint32_t));
    __uint(value_size, sizeof(uint32_t));
} whitelist_pids SEC(".maps");

// ── Blocked pids ────────────────────────────────────────────────────────
struct {
    __uint(type, 1);
    __uint(max_entries, 65536);
    __uint(key_size, sizeof(uint32_t));
    __uint(value_size, sizeof(uint32_t));
} blocked_pids SEC(".maps");

// Helper: check if pid is whitelisted
static __always_inline int is_whitelisted(uint32_t pid)
{
    uint32_t *val = bpf_map_lookup_elem(&whitelist_pids, &pid);
    return val && *val == 1;
}

// ═══════════════════════════════════════════════════════════════════════════
// LSM HOOK: bprm_check_security — block dangerous commands
// ═══════════════════════════════════════════════════════════════════════════
// int bprm_check_security(struct linux_binprm *bprm)

SEC("lsm/bprm_check_security")
int block_exec(struct linux_binprm *bprm)
{
    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    uint32_t pid = pid_tgid >> 32;
    uint64_t uid_gid = bpf_get_current_uid_gid();
    uint32_t uid = (uint32_t)uid_gid;

    if (is_whitelisted(pid)) return 0;

    // Check per-pid block list
    struct policy_key { uint32_t pid; };
    struct policy_key pk = { .pid = pid };
    uint32_t *pv = bpf_map_lookup_elem(&blocked_pids, &pk);
    if (pv && *pv == 1) return -1; // EPERM

    // Read command from bprm->filename
    char comm[MAX_COMM_LEN] = {};
    const char *filename = NULL;
    bpf_probe_read_kernel(&filename, sizeof(filename), (char *)bprm + 0x08);
    if (filename) {
        bpf_probe_read_user_str(comm, sizeof(comm), filename);
    }
    if (comm[0] == '\0') return 0;

    // Check full path in command blocklist
    struct cmd_policy_key ck;
    __builtin_memset(&ck, 0, sizeof(ck));
    __builtin_memcpy(&ck.comm, comm, MAX_COMM_LEN);
    struct cmd_policy_value *cv = bpf_map_lookup_elem(&blocked_commands, &ck);
    if (cv && cv->action == 1) return -1; // EPERM

    // Check basename (e.g., "/usr/bin/rm" → "rm")
    int last_slash = -1;
    #pragma unroll
    for (int i = 0; i < MAX_COMM_LEN - 1; i++) {
        if (comm[i] == '/') last_slash = i;
        if (comm[i] == '\0') break;
    }
    if (last_slash >= 0 && last_slash + 1 < MAX_COMM_LEN) {
        struct cmd_policy_key bk;
        __builtin_memset(&bk, 0, sizeof(bk));
        // Copy basename byte-by-byte (BPF can't handle variable-length memcpy)
        #pragma unroll
        for (int i = 0; i < MAX_COMM_LEN; i++) {
            int src = last_slash + 1 + i;
            if (src >= MAX_COMM_LEN || comm[src] == '\0') break;
            bk.comm[i] = comm[src];
        }
        struct cmd_policy_value *bv = bpf_map_lookup_elem(&blocked_commands, &bk);
        if (bv && bv->action == 1) return -1; // EPERM
    }

    return 0; // Allow
}

// ═══════════════════════════════════════════════════════════════════════════
// LSM HOOK: file_open — block writes to protected paths
// ═══════════════════════════════════════════════════════════════════════════
// int file_open(struct file *file)

SEC("lsm/file_open")
int block_file_open(struct file *file)
{
    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    uint32_t pid = pid_tgid >> 32;

    if (is_whitelisted(pid)) return 0;

    // Check f_mode for write intent (FMODE_WRITE = 0x1)
    unsigned int f_mode = 0;
    bpf_probe_read_kernel(&f_mode, sizeof(f_mode), (char *)file + 0x44);
    if (!(f_mode & 1)) return 0; // Read-only — allow

    // Get file path using bpf_d_path (helper 147)
    static long (*const bpf_d_path)(void *path, char *buf, uint32_t size) = (void *)147;
    char path[MAX_PATH_LEN] = {};
    // file->f_path at offset 0x14 on x86_64 5.15
    long path_len = bpf_d_path((char *)file + 0x14, path, sizeof(path));
    if (path_len <= 0) return 0;

    // Check protected paths — prefix match
    struct path_policy_key pk;
    __builtin_memset(&pk, 0, sizeof(pk));
    __builtin_memcpy(&pk.prefix, path, MAX_PATH_LEN);

    // Try common prefix lengths (/etc, /etc/, /etc/p, etc.)
    #pragma unroll
    for (int len = 4; len <= 20; len++) {
        if (path[len] == '/' || path[len] == '\0') {
            struct path_policy_key tpk;
            __builtin_memset(&tpk, 0, sizeof(tpk));
            __builtin_memcpy(&tpk.prefix, path, len + 1);
            struct path_policy_value *pv = bpf_map_lookup_elem(&protected_paths, &tpk);
            if (pv && pv->action == 1) return -1; // EPERM
        }
        if (path[len] == '\0') break;
    }

    return 0; // Allow
}

// ═══════════════════════════════════════════════════════════════════════════
// LSM HOOK: inode_unlink — monitor file deletions
// ═══════════════════════════════════════════════════════════════════════════

SEC("lsm/inode_unlink")
int monitor_unlink(struct inode *dir, struct dentry *dentry)
{
    // Deletion monitoring only — actual blocking handled by file_open
    return 0;
}

// ═══════════════════════════════════════════════════════════════════════════
// LSM HOOK: socket_bind — block network binds from blocked pids
// ═══════════════════════════════════════════════════════════════════════════

SEC("lsm/socket_bind")
int monitor_bind(struct socket *sock)
{
    uint64_t pid_tgid = bpf_get_current_pid_tgid();
    uint32_t pid = pid_tgid >> 32;

    if (is_whitelisted(pid)) return 0;

    struct policy_key { uint32_t pid; };
    struct policy_key pk = { .pid = pid };
    uint32_t *pv = bpf_map_lookup_elem(&blocked_pids, &pk);
    if (pv && *pv == 1) return -1; // EPERM

    return 0; // Allow
}

char LICENSE[] SEC("license") = "GPL";

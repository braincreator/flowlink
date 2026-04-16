//! Kernel event types captured by sentinel

/// A kernel-level event captured by syscall monitoring
#[derive(Debug, Clone, serde::Serialize)]
pub struct KernelEvent {
    /// Event type
    pub kind: EventKind,
    /// Process ID that triggered the event
    pub pid: u32,
    /// Parent process ID
    pub ppid: u32,
    /// User ID
    pub uid: u32,
    /// Command that was executed (for exec events)
    pub command: Option<String>,
    /// Arguments to the command
    pub args: Vec<String>,
    /// File path (for file events)
    pub path: Option<String>,
    /// Remote address (for network events)
    pub remote_addr: Option<String>,
    /// Timestamp (epoch ms)
    pub timestamp: u64,
}

/// Type of kernel event
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    /// Process execution (execve)
    Exec,
    /// File opened for writing (openat with O_WRONLY/O_RDWR)
    FileWrite,
    /// File deleted (unlinkat)
    FileDelete,
    /// Outgoing network connection (connect)
    NetworkConnect,
    /// Network listener started (bind)
    NetworkBind,
    /// Filesystem mounted (mount)
    Mount,
}

impl KernelEvent {
    /// Risk score 0-100 based on event type and context
    pub fn risk_score(&self) -> u32 {
        match self.kind {
            EventKind::Exec => {
                let cmd = self.command.as_deref().unwrap_or("");
                let critical =
                    ["rm", "mkfs", "dd", "shred", "shutdown", "reboot", "poweroff", "halt"];
                if critical.iter().any(|c| cmd.ends_with(c) || cmd == *c) {
                    90
                } else {
                    10
                }
            }
            EventKind::FileWrite => {
                let path = self.path.as_deref().unwrap_or("");
                let system_paths = ["/etc/", "/var/", "/usr/", "/boot/", "/dev/"];
                if system_paths.iter().any(|p| path.starts_with(p)) {
                    80
                } else {
                    20
                }
            }
            EventKind::FileDelete => {
                let path = self.path.as_deref().unwrap_or("");
                if path.starts_with("/etc/") || path.starts_with("/var/") {
                    75
                } else {
                    30
                }
            }
            EventKind::NetworkConnect => 25,
            EventKind::NetworkBind => 60,
            EventKind::Mount => 50,
        }
    }
}


mod tests {
    #![allow(dead_code)]
    use super::*;

    fn make_event(kind: EventKind) -> KernelEvent {
        KernelEvent {
            kind,
            pid: 1000,
            ppid: 999,
            uid: 0,
            command: None,
            args: vec![],
            path: None,
            remote_addr: None,
            timestamp: 0,
        }
    }

    fn exec_event(cmd: &str) -> KernelEvent {
        let mut e = make_event(EventKind::Exec);
        e.command = Some(cmd.into());
        e
    }

    fn write_event(path: &str) -> KernelEvent {
        let mut e = make_event(EventKind::FileWrite);
        e.path = if path.is_empty() { None } else { Some(path.into()) };
        e
    }

    fn delete_event(path: &str) -> KernelEvent {
        let mut e = make_event(EventKind::FileDelete);
        e.path = if path.is_empty() { None } else { Some(path.into()) };
        e
    }

    // ── EventKind variants ──
    #[test] fn kind_exec() { let e = make_event(EventKind::Exec); assert_eq!(e.kind, EventKind::Exec); }
    #[test] fn kind_file_write() { assert_eq!(make_event(EventKind::FileWrite).kind, EventKind::FileWrite); }
    #[test] fn kind_file_delete() { assert_eq!(make_event(EventKind::FileDelete).kind, EventKind::FileDelete); }
    #[test] fn kind_network_connect() { assert_eq!(make_event(EventKind::NetworkConnect).kind, EventKind::NetworkConnect); }
    #[test] fn kind_network_bind() { assert_eq!(make_event(EventKind::NetworkBind).kind, EventKind::NetworkBind); }
    #[test] fn kind_mount() { assert_eq!(make_event(EventKind::Mount).kind, EventKind::Mount); }

    // ── risk_score: Exec ──
    #[test] fn risk_exec_rm() { assert_eq!(exec_event("rm").risk_score(), 90); }
    #[test] fn risk_exec_mkfs() { assert_eq!(exec_event("mkfs").risk_score(), 90); }
    #[test] fn risk_exec_dd() { assert_eq!(exec_event("dd").risk_score(), 90); }
    #[test] fn risk_exec_shred() { assert_eq!(exec_event("shred").risk_score(), 90); }
    #[test] fn risk_exec_shutdown() { assert_eq!(exec_event("shutdown").risk_score(), 90); }
    #[test] fn risk_exec_reboot() { assert_eq!(exec_event("reboot").risk_score(), 90); }
    #[test] fn risk_exec_poweroff() { assert_eq!(exec_event("poweroff").risk_score(), 90); }
    #[test] fn risk_exec_halt() { assert_eq!(exec_event("halt").risk_score(), 90); }
    #[test] fn risk_exec_ls() { assert_eq!(exec_event("ls").risk_score(), 10); }
    #[test] fn risk_exec_cat() { assert_eq!(exec_event("cat").risk_score(), 10); }
    #[test] fn risk_exec_empty() { assert_eq!(exec_event("").risk_score(), 10); }
    #[test] fn risk_exec_none() { assert_eq!(make_event(EventKind::Exec).risk_score(), 10); }
    #[test] fn risk_exec_full_path_rm() { assert_eq!(exec_event("/usr/bin/rm").risk_score(), 90); }
    #[test] fn risk_exec_full_path_ls() { assert_eq!(exec_event("/usr/bin/ls").risk_score(), 10); }
    #[test] fn risk_exec_custom_binary() { assert_eq!(exec_event("myapp").risk_score(), 10); }
    #[test] fn risk_exec_make() { assert_eq!(exec_event("make").risk_score(), 10); }
    #[test] fn risk_exec_cargo() { assert_eq!(exec_event("cargo").risk_score(), 10); }

    // ── risk_score: FileWrite ──
    #[test] fn risk_write_etc_shadow() { assert_eq!(write_event("/etc/shadow").risk_score(), 80); }
    #[test] fn risk_write_etc_passwd() { assert_eq!(write_event("/etc/passwd").risk_score(), 80); }
    #[test] fn risk_write_var_log() { assert_eq!(write_event("/var/log/syslog").risk_score(), 80); }
    #[test] fn risk_write_usr() { assert_eq!(write_event("/usr/bin/custom").risk_score(), 80); }
    #[test] fn risk_write_boot() { assert_eq!(write_event("/boot/grub.cfg").risk_score(), 80); }
    #[test] fn risk_write_dev() { assert_eq!(write_event("/dev/sda").risk_score(), 80); }
    #[test] fn risk_write_home() { assert_eq!(write_event("/home/user/file").risk_score(), 20); }
    #[test] fn risk_write_tmp() { assert_eq!(write_event("/tmp/file").risk_score(), 20); }
    #[test] fn risk_write_opt() { assert_eq!(write_event("/opt/app/config").risk_score(), 20); }
    #[test] fn risk_write_none_path() { assert_eq!(KernelEvent{kind:EventKind::FileWrite,path:None,..make_event(EventKind::FileWrite)}.risk_score(), 20); }
    #[test] fn risk_write_empty() { assert_eq!(write_event("").risk_score(), 20); }
    #[test] fn risk_write_relative() { assert_eq!(write_event("file.txt").risk_score(), 20); }
    #[test] fn risk_write_root_file() { assert_eq!(write_event("/root/.bashrc").risk_score(), 20); }
    #[test] fn risk_write_etc_nested() { assert_eq!(write_event("/etc/nginx/sites-enabled/default").risk_score(), 80); }

    // ── risk_score: FileDelete ──
    #[test] fn risk_delete_etc() { assert_eq!(delete_event("/etc/passwd").risk_score(), 75); }
    #[test] fn risk_delete_var() { assert_eq!(delete_event("/var/lib/data").risk_score(), 75); }
    #[test] fn risk_delete_usr() { assert_eq!(delete_event("/usr/bin/tool").risk_score(), 30); }
    #[test] fn risk_delete_home() { assert_eq!(delete_event("/home/user/file").risk_score(), 30); }
    #[test] fn risk_delete_tmp() { assert_eq!(delete_event("/tmp/cache").risk_score(), 30); }
    #[test] fn risk_delete_none() { assert_eq!(KernelEvent{kind:EventKind::FileDelete,path:None,..make_event(EventKind::FileDelete)}.risk_score(), 30); }
    #[test] fn risk_delete_empty() { assert_eq!(delete_event("").risk_score(), 30); }

    // ── risk_score: Network ──
    #[test] fn risk_connect() { assert_eq!(make_event(EventKind::NetworkConnect).risk_score(), 25); }
    #[test] fn risk_bind() { assert_eq!(make_event(EventKind::NetworkBind).risk_score(), 60); }

    // ── risk_score: Mount ──
    #[test] fn risk_mount() { assert_eq!(make_event(EventKind::Mount).risk_score(), 50); }

    // ── Serialization ──
    #[test] fn serialize_exec() {
        let e = exec_event("rm");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"Exec\""));
        assert!(json.contains("\"rm\""));
    }
    #[test] fn serialize_file_write() {
        let e = write_event("/etc/passwd");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"FileWrite\""));
    }
    #[test] fn serialize_all_fields() {
        let e = KernelEvent {
            kind: EventKind::Exec,
            pid: 1234,
            ppid: 567,
            uid: 0,
            command: Some("rm".into()),
            args: vec!["-rf".into(), "/".into()],
            path: None,
            remote_addr: None,
            timestamp: 1234567890,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("1234"));
        assert!(json.contains("rm"));
    }

    // ── EventKind traits ──
    #[test] fn event_kind_copy() { let k = EventKind::Exec; let k2 = k; assert_eq!(k, k2); }
    #[test] fn event_kind_eq() { assert_eq!(EventKind::Exec, EventKind::Exec); assert_ne!(EventKind::Exec, EventKind::Mount); }
    #[test] fn event_kind_serde() {
        for (kind, name) in [
            (EventKind::Exec, "Exec"),
            (EventKind::FileWrite, "FileWrite"),
            (EventKind::FileDelete, "FileDelete"),
            (EventKind::NetworkConnect, "NetworkConnect"),
            (EventKind::NetworkBind, "NetworkBind"),
            (EventKind::Mount, "Mount"),
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert!(json.contains(name), "{} not in {}", name, json);
        }
    }
}

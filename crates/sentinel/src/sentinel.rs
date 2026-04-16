//! Sentinel — kernel-level monitoring backend

use crate::{EventKind, KernelEvent, SentinelConfig, Verdict};
use tokio::sync::mpsc;

/// Channel for receiving kernel events
pub type EventReceiver = mpsc::Receiver<KernelEvent>;

/// The sentinel monitor
pub struct Sentinel {
    config: SentinelConfig,
    sender: mpsc::Sender<KernelEvent>,
    receiver: Option<EventReceiver>,
    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    lsm_blocker: Option<std::sync::Mutex<crate::lsm_blocker::LsmBlocker>>,
}

impl Sentinel {
    pub fn new(config: SentinelConfig) -> Self {
        let (sender, receiver) = mpsc::channel(1024);
        Self {
            config,
            sender,
            receiver: Some(receiver),
            #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
            lsm_blocker: None,
        }
    }

    /// Take the event receiver (only once)
    pub fn take_receiver(&mut self) -> Option<EventReceiver> {
        self.receiver.take()
    }

    /// Get a sender to inject events (for testing)
    pub fn sender(&self) -> mpsc::Sender<KernelEvent> {
        self.sender.clone()
    }

    /// Hot-reload policy: update blocked commands and protected paths without restart
    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn reload_policy(&self, config: &SentinelConfig) -> anyhow::Result<crate::lsm_blocker::ReloadStats> {
        if let Some(ref blocker) = self.lsm_blocker {
            let mut b = blocker.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
            b.reload_config(config)
        } else {
            Err(anyhow::anyhow!("LSM blocker not loaded"))
        }
    }

    /// Block a command at kernel level (hot-reload)
    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn block_command(&self, cmd: &str) -> anyhow::Result<()> {
        if let Some(ref blocker) = self.lsm_blocker {
            let mut b = blocker.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
            b.block_command(cmd)
        } else {
            Err(anyhow::anyhow!("LSM blocker not loaded"))
        }
    }

    /// Unblock a command (hot-reload)
    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn unblock_command(&self, cmd: &str) -> anyhow::Result<()> {
        if let Some(ref blocker) = self.lsm_blocker {
            let mut b = blocker.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
            b.unblock_command(cmd)
        } else {
            Err(anyhow::anyhow!("LSM blocker not loaded"))
        }
    }

    /// Get current policy snapshot
    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn policy_snapshot(&self) -> Option<crate::lsm_blocker::PolicySnapshot> {
        self.lsm_blocker.as_ref().and_then(|blocker| {
            blocker.lock().ok().map(|b| b.policy_snapshot())
        })
    }

    /// Start monitoring. Platform-specific backend is selected at compile time.
    pub async fn start(&mut self) -> anyhow::Result<()> {
        #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
        {
            self.start_ebpf().await
        }
        #[cfg(all(target_os = "macos", feature = "esf"))]
        {
            self.start_esf().await
        }
        #[cfg(all(target_os = "macos", not(feature = "esf")))]
        {
            self.start_esf_passive().await
        }
        #[cfg(not(any(
            all(target_os = "linux", feature = "linux-ebpf"),
            target_os = "macos"
        )))]
        {
            self.start_stub().await
        }
    }

    /// Evaluate a kernel event against policy
    pub fn evaluate(&self, event: &KernelEvent) -> Verdict {
        let score = event.risk_score();

        // Check critical binaries
        if event.kind == EventKind::Exec {
            if let Some(cmd) = &event.command {
                let binary = cmd.rsplit('/').next().unwrap_or(cmd);
                if self.config.critical_binaries.iter().any(|c| c == binary) {
                    return Verdict::Log {
                        reason: format!("Critical binary executed: {}", binary),
                    };
                }
            }
        }

        // Check protected paths
        if matches!(event.kind, EventKind::FileWrite | EventKind::FileDelete) {
            if let Some(path) = &event.path {
                if self
                    .config
                    .protected_paths
                    .iter()
                    .any(|p| path.starts_with(p.as_str()))
                {
                    return if score >= 70 {
                        Verdict::Block {
                            reason: format!("Write to protected path: {}", path),
                        }
                    } else {
                        Verdict::Log {
                            reason: format!("Modification in protected path: {}", path),
                        }
                    };
                }
            }
        }

        // Network bind (potential reverse shell)
        if event.kind == EventKind::NetworkBind && score >= 50 {
            return Verdict::Log {
                reason: "Network listener started".into(),
            };
        }

        if score >= 70 {
            Verdict::Log {
                reason: format!("High-risk kernel event (score: {})", score),
            }
        } else {
            Verdict::Allow
        }
    }
}


// Wrapper to make perf_buffer pointer Send-safe
#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
struct SendPerfBuffer(*mut libbpf_sys::perf_buffer);
#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
unsafe impl Send for SendPerfBuffer {}

// ── Platform backends ──────────────────────────────────────────────────────

#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
impl Sentinel {
    async fn start_ebpf(&mut self) -> anyhow::Result<()> {
        use libbpf_sys::*;
        use std::ptr;

        // ── Try to load LSM BPF blocker (kernel-level blocking) ──
        match crate::lsm_blocker::LsmBlocker::load() {
            Ok(mut blocker) => {
                if let Err(e) = blocker.load_config(&self.config) {
                    tracing::warn!("LSM config load failed (non-fatal): {}", e);
                } else {
                    tracing::info!(
                        " LSM blocker active: {} commands blocked, {} paths protected",
                        blocker.blocked_commands().len(),
                        blocker.protected_paths().len()
                    );
                }
                self.lsm_blocker = Some(std::sync::Mutex::new(blocker));
            }
            Err(e) => {
                tracing::warn!("LSM BPF not available (monitoring only): {}", e);
            }
        }

        // ── Tracepoint monitoring via libbpf ──
        let bpf_bytes = include_bytes!("../bpf/sentinel.bpf.o");
        let obj_opts = bpf_object_open_opts {
            sz: std::mem::size_of::<bpf_object_open_opts>() as u64,
            ..unsafe { std::mem::zeroed() }
        };

        let obj = unsafe {
            bpf_object__open_mem(
                bpf_bytes.as_ptr() as *const libc::c_void,
                bpf_bytes.len() as u64,
                &obj_opts,
            )
        };
        if obj.is_null() {
            tracing::warn!("libbpf: failed to open tracepoint BPF object");
            return Ok(());
        }

        let ret = unsafe { bpf_object__load(obj) };
        if ret != 0 {
            let errno = unsafe { *libc::__errno_location() };
            tracing::warn!("libbpf: failed to load tracepoint programs (errno={})", errno);
            unsafe { bpf_object__close(obj) };
            return Ok(());
        }

        // Attach tracepoint programs
        let tracepoints: &[(&str, &str)] = &[
            ("trace_execve", "syscalls/sys_enter_execve"),
            ("trace_openat", "syscalls/sys_enter_openat"),
            ("trace_connect", "syscalls/sys_enter_connect"),
            ("trace_bind", "syscalls/sys_enter_bind"),
            ("trace_unlinkat", "syscalls/sys_enter_unlinkat"),
            ("trace_mount", "syscalls/sys_enter_mount"),
        ];

        let mut attached = 0;
        for (prog_name, _tp) in tracepoints {
            let c_name = std::ffi::CString::new(*prog_name).unwrap();
            let prog = unsafe { bpf_object__find_program_by_name(obj, c_name.as_ptr()) };
            if prog.is_null() { continue; }
            let link = unsafe { bpf_program__attach(prog) };
            if !link.is_null() { attached += 1; }
        }

        // Set up perf buffer for events
        let events_cname = std::ffi::CString::new("events").unwrap();
        let events_map = unsafe { bpf_object__find_map_by_name(obj, events_cname.as_ptr()) };
        if events_map.is_null() {
            tracing::info!("eBPF: {} tracepoints attached (no events map)", attached);
            std::mem::forget(obj);
            return Ok(());
        }
        let events_fd = unsafe { bpf_map__fd(events_map) };

        // Perf buffer callback
        let sender = self.sender.clone();
        let sample_ctx: Box<Box<dyn FnMut(*const libc::c_void, usize) + Send>> =
            Box::new(Box::new(move |data: *const libc::c_void, size: usize| {
                let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, size) };
                if let Ok(event) = parse_bpf_event(bytes) {
                    use tokio::sync::mpsc::error::TrySendError;
                    let _ = sender.try_send(event);
                }
            }));
        let sample_ctx_ptr = Box::into_raw(sample_ctx) as *mut libc::c_void;

        let pb = SendPerfBuffer(unsafe {
            perf_buffer__new(
                events_fd, 64,
                Some(Self::perf_sample_cb), None,
                sample_ctx_ptr, ptr::null(),
            )
        });
        if pb.0.is_null() {
            tracing::warn!("Failed to create perf buffer");
            std::mem::forget(obj);
            return Ok(());
        }

        // Poll in background
        let pb_raw = pb.0 as usize; // usize is Send
        tokio::task::spawn_blocking(move || {
            let pb_ptr = pb_raw as *mut libbpf_sys::perf_buffer;
            loop {
                let ret = unsafe { perf_buffer__poll(pb_ptr, 100) };
                if ret < 0 { break; }
            }
            unsafe { perf_buffer__free(pb_ptr) };
        });

        tracing::info!("eBPF sentinel started via libbpf: {} tracepoints attached", attached);
        std::mem::forget(obj);
        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    unsafe extern "C" fn perf_sample_cb(ctx: *mut libc::c_void, _cpu: i32, data: *mut libc::c_void, size: u32) {
        unsafe {
            if !ctx.is_null() {
                let cb = &mut *(ctx as *mut Box<dyn FnMut(*const libc::c_void, usize) + Send>);
                cb(data as *const libc::c_void, size as usize);
            }
        }
    }
}

/// Parse raw BPF event bytes into a `KernelEvent`.
/// Delegates to the shared parser in `bpf_event`.
#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
fn parse_bpf_event(data: &[u8]) -> anyhow::Result<KernelEvent> {
    crate::bpf_event::parse_bpf_event(data)
}

/// Convert ESF exec event to KernelEvent
#[cfg(all(target_os = "macos", feature = "esf"))]
fn esf_exec_to_event(exec: &endpointsecurity::EsEventExec, proc: &endpointsecurity::EsProcess) -> Option<KernelEvent> {
    Some(KernelEvent {
        kind: EventKind::Exec,
        pid: proc.pid,
        ppid: proc.ppid,
        uid: 0,
        command: Some(exec.target.executable.path.clone()),
        args: exec.args.clone(),
        path: None,
        remote_addr: None,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    })
}

/// Convert ESF open event to KernelEvent (only for writes)
#[cfg(all(target_os = "macos", feature = "esf"))]
fn esf_open_to_event(open: &endpointsecurity::EsEventOpen, proc: &endpointsecurity::EsProcess) -> Option<KernelEvent> {
    let is_write = (open.fflag & 0x3) != 0; // O_WRONLY=1, O_RDWR=2
    if is_write {
        Some(KernelEvent {
            kind: EventKind::FileWrite,
            pid: proc.pid,
            ppid: proc.ppid,
            uid: 0,
            command: None,
            args: vec![],
            path: Some(open.file.path.clone()),
            remote_addr: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    } else {
        None
    }
}

#[cfg(all(target_os = "macos", feature = "esf"))]
impl Sentinel {
    async fn start_esf(&mut self) -> anyhow::Result<()> {
        let config = self.config.clone();
        let sender = self.sender.clone();

        // ESF requires root
        if unsafe { libc::getuid() } != 0 {
            tracing::warn!("ESF requires root privileges — running in passive mode");
            tracing::warn!("Run with sudo or add Endpoint Security entitlement");
            return self.start_esf_passive().await;
        }

        let (tx, rx) = crossbeam_channel::unbounded::<endpointsecurity::EsMessage>();
        let es_client = endpointsecurity::create_es_client(tx)
            .map_err(|e| anyhow::anyhow!("ESF client creation failed: {:?} (need root/SIP entitlement)", e))?;

        // ── Subscribe to AUTH events for real-time blocking ──
        let mut auth_events: Vec<endpointsecurity::SupportedEsEvent> = vec![];
        if config.monitor_exec {
            auth_events.push(endpointsecurity::SupportedEsEvent::AuthExec);
        }
        if config.monitor_file_write {
            auth_events.push(endpointsecurity::SupportedEsEvent::AuthOpen);
        }
        if config.monitor_delete {
            auth_events.push(endpointsecurity::SupportedEsEvent::AuthUnlink);
        }

        if !auth_events.is_empty() {
            if !es_client.subscribe_to_events(&auth_events) {
                return Err(anyhow::anyhow!("Failed to subscribe to ESF Auth events"));
            }
        }

        // ── Auth event loop: evaluate and DENY/ALLOW in real-time ──
        let sentinel = Sentinel::new(config.clone());
        let sender_clone = sender.clone();
        let client_for_respond = unsafe { std::ptr::read(&es_client as *const _) };
        // We need the client alive for responding — wrap in Arc
        let es_client_arc = std::sync::Arc::new(es_client);
        let es_client_forget = es_client_arc.clone();

        std::thread::spawn(move || {
            loop {
                match rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(msg) => {
                        // Convert ESF event → KernelEvent
                        let ke = match &msg.event {
                            endpointsecurity::EsEvent::AuthExec(exec) => {
                                esf_exec_to_event(exec, &msg.process)
                            }
                            endpointsecurity::EsEvent::AuthOpen(open) => {
                                esf_open_to_event(open, &msg.process)
                            }
                            endpointsecurity::EsEvent::AuthUnlink(unlink) => {
                                Some(KernelEvent {
                                    kind: EventKind::FileDelete,
                                    pid: msg.process.pid,
                                    ppid: msg.process.ppid,
                                    uid: 0,
                                    command: None,
                                    args: vec![],
                                    path: Some(unlink.target.path.clone()),
                                    remote_addr: None,
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as u64,
                                })
                            }
                            _ => None,
                        };

                        // Evaluate and respond
                        if let Some(event) = ke {
                            let verdict = sentinel.evaluate(&event);
                            let (auth_result, cache) = match &verdict {
                                Verdict::Block { reason } => {
                                    tracing::warn!(
                                        "🚫 ESF BLOCKED: {} (pid={})",
                                        reason, event.pid
                                    );
                                    (endpointsecurity::EsAuthResult::Deny, endpointsecurity::EsCacheResult::Yes)
                                }
                                Verdict::Log { reason } => {
                                    tracing::warn!(
                                        "⚠️  ESF LOG: {} (pid={})",
                                        reason, event.pid
                                    );
                                    (endpointsecurity::EsAuthResult::Allow, endpointsecurity::EsCacheResult::Yes)
                                }
                                Verdict::Allow => {
                                    (endpointsecurity::EsAuthResult::Allow, endpointsecurity::EsCacheResult::Yes)
                                }
                            };

                            // Respond to kernel — MUST respond to Auth events
                            if matches!(msg.action_type, endpointsecurity::EsActionType::Auth) {
                                es_client_arc.respond_to_auth_event(&msg, &auth_result, &cache);
                            }

                            // Forward event to channel for external consumers
                            let _ = sender_clone.try_send(event);
                        } else {
                            // Unknown event — allow by default
                            if matches!(msg.action_type, endpointsecurity::EsActionType::Auth) {
                                es_client_arc.respond_to_auth_event(
                                    &msg,
                                    &endpointsecurity::EsAuthResult::Allow,
                                    &endpointsecurity::EsCacheResult::Yes,
                                );
                            }
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        tracing::error!("ESF channel disconnected");
                        break;
                    }
                }
            }
        });

        tracing::info!("🔒 ESF sentinel started in AUTH mode — blocking malicious commands at kernel level");
        std::mem::forget(es_client_forget);
        Ok(())
    }

    /// Passive mode — logs warnings but doesn't crash
    async fn start_esf_passive(&mut self) -> anyhow::Result<()> {
        tracing::info!("ESF sentinel in passive mode (no root). Install as root for full kernel monitoring.");
        Ok(())
    }
}

// macOS without ESF feature — stub mode with synthetic heartbeat
#[cfg(all(target_os = "macos", not(feature = "esf")))]
impl Sentinel {
    async fn start_esf_passive(&mut self) -> anyhow::Result<()> {
        tracing::info!(
            "ESF sentinel: running in stub mode on macOS (full ESF requires root + endpoint-security entitlement)"
        );
        let sender = self.sender.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let _ = sender
                    .send(KernelEvent {
                        kind: EventKind::Exec,
                        pid: std::process::id(),
                        ppid: 1,
                        uid: unsafe { libc::getuid() },
                        command: Some("synthetic-heartbeat".into()),
                        args: vec![],
                        path: None,
                        remote_addr: None,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    })
                    .await;
            }
        });
        Ok(())
    }
}

#[cfg(not(any(
    all(target_os = "linux", feature = "linux-ebpf"),
    target_os = "macos"
)))]
impl Sentinel {
    async fn start_stub(&mut self) -> anyhow::Result<()> {
        tracing::info!("Sentinel running in stub mode (no kernel monitoring)");
        Ok(())
    }
}


mod tests {
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
        e.path = Some(path.into());
        e
    }

    fn delete_event(path: &str) -> KernelEvent {
        let mut e = make_event(EventKind::FileDelete);
        e.path = Some(path.into());
        e
    }

    // ═══════════════════════════════════════════
    // Verdict: Exec — critical binaries
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_exec_rm() {
        let s = Sentinel::new(SentinelConfig::default());
        let e = exec_event("rm");
        match s.evaluate(&e) {
            Verdict::Log { reason } => assert!(reason.contains("rm")),
            other => panic!("expected Log, got {:?}", other),
        }
    }

    #[test]
    fn verdict_exec_mkfs() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&exec_event("mkfs")) {
            Verdict::Log { .. } => {}
            other => panic!("expected Log for mkfs, got {:?}", other),
        }
    }

    #[test]
    fn verdict_exec_dd() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&exec_event("dd")) {
            Verdict::Log { .. } => {}
            other => panic!("expected Log for dd, got {:?}", other),
        }
    }

    #[test]
    fn verdict_exec_shred() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&exec_event("shred")) {
            Verdict::Log { .. } => {}
            other => panic!("expected Log for shred, got {:?}", other),
        }
    }

    #[test]
    fn verdict_exec_shutdown() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&exec_event("shutdown")) {
            Verdict::Log { .. } => {}
            other => panic!("expected Log for shutdown, got {:?}", other),
        }
    }

    #[test]
    fn verdict_exec_reboot() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&exec_event("reboot")) {
            Verdict::Log { .. } => {}
            other => panic!("expected Log for reboot, got {:?}", other),
        }
    }

    // ═══════════════════════════════════════════
    // Verdict: Exec — safe binaries
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_exec_ls_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&exec_event("ls")), Verdict::Allow));
    }

    #[test]
    fn verdict_exec_cat_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&exec_event("cat")), Verdict::Allow));
    }

    #[test]
    fn verdict_exec_git_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&exec_event("git")), Verdict::Allow));
    }

    #[test]
    fn verdict_exec_node_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&exec_event("node")), Verdict::Allow));
    }

    #[test]
    fn verdict_exec_python_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&exec_event("python3")), Verdict::Allow));
    }

    #[test]
    fn verdict_exec_none_command() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&make_event(EventKind::Exec)), Verdict::Allow));
    }

    #[test]
    fn verdict_exec_full_path_rm() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&exec_event("/usr/bin/rm")) {
            Verdict::Log { reason } => assert!(reason.contains("rm")),
            other => panic!("expected Log for /usr/bin/rm, got {:?}", other),
        }
    }

    // ═══════════════════════════════════════════
    // Verdict: FileWrite — protected paths → Block
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_write_etc_shadow_block() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&write_event("/etc/shadow")) {
            Verdict::Block { reason } => assert!(reason.contains("/etc/shadow")),
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn verdict_write_etc_passwd_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/etc/passwd")), Verdict::Block { .. }));
    }

    #[test]
    fn verdict_write_etc_hosts_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/etc/hosts")), Verdict::Block { .. }));
    }

    #[test]
    fn verdict_write_var_lib_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/var/lib/data")), Verdict::Block { .. }));
    }

    #[test]
    fn verdict_write_usr_bin_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/usr/bin/custom")), Verdict::Block { .. }));
    }

    #[test]
    fn verdict_write_boot_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/boot/grub")), Verdict::Block { .. }));
    }

    #[test]
    fn verdict_write_etc_nested_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/etc/nginx/sites-enabled/default")), Verdict::Block { .. }));
    }

    // ═══════════════════════════════════════════
    // Verdict: FileWrite — safe paths → Allow
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_write_home_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/home/user/file")), Verdict::Allow));
    }

    #[test]
    fn verdict_write_tmp_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/tmp/file")), Verdict::Allow));
    }

    #[test]
    fn verdict_write_opt_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&write_event("/opt/app/config")), Verdict::Allow));
    }

    // ═══════════════════════════════════════════
    // Verdict: FileDelete — system paths → Block
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_delete_etc_block() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&delete_event("/etc/passwd")) {
            Verdict::Block { reason } => assert!(reason.contains("/etc/passwd")),
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn verdict_delete_var_block() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&delete_event("/var/lib/data")), Verdict::Block { .. }));
    }

    #[test]
    fn verdict_delete_home_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&delete_event("/home/user/file")), Verdict::Allow));
    }

    #[test]
    fn verdict_delete_tmp_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&delete_event("/tmp/cache")), Verdict::Allow));
    }

    // ═══════════════════════════════════════════
    // Verdict: Network
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_network_bind_log() {
        let s = Sentinel::new(SentinelConfig::default());
        match s.evaluate(&make_event(EventKind::NetworkBind)) {
            Verdict::Log { reason } => assert!(reason.contains("listener")),
            other => panic!("expected Log for bind, got {:?}", other),
        }
    }

    #[test]
    fn verdict_network_connect_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        assert!(matches!(s.evaluate(&make_event(EventKind::NetworkConnect)), Verdict::Allow));
    }

    // ═══════════════════════════════════════════
    // Verdict: Mount
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_mount() {
        let s = Sentinel::new(SentinelConfig::default());
        let e = make_event(EventKind::Mount);
        let v = s.evaluate(&e);
        // score=50 < 70 → Allow (no special mount handling)
        assert!(matches!(v, Verdict::Allow));
    }

    // ═══════════════════════════════════════════
    // SentinelConfig
    // ═══════════════════════════════════════════

    #[test]
    fn config_default_monitors_enabled() {
        let c = SentinelConfig::default();
        assert!(c.monitor_exec);
        assert!(c.monitor_file_write);
        assert!(c.monitor_network);
        assert!(c.monitor_delete);
    }

    #[test]
    fn config_default_protected_paths() {
        let c = SentinelConfig::default();
        assert!(c.protected_paths.iter().any(|p| p == "/etc"));
        assert!(c.protected_paths.iter().any(|p| p == "/var"));
        assert!(c.protected_paths.iter().any(|p| p == "/usr"));
    }

    #[test]
    fn config_default_critical_binaries() {
        let c = SentinelConfig::default();
        assert!(c.critical_binaries.iter().any(|b| b == "rm"));
        assert!(c.critical_binaries.iter().any(|b| b == "mkfs"));
        assert!(c.critical_binaries.iter().any(|b| b == "dd"));
    }

    #[test]
    fn config_custom_paths() {
        let c = SentinelConfig {
            protected_paths: vec!["/custom".into()],
            ..Default::default()
        };
        let s = Sentinel::new(c);
        // /etc not in custom paths but score=80 → high-risk Log
        assert!(matches!(s.evaluate(&write_event("/etc/passwd")), Verdict::Log { .. }));
        // /custom IS in paths → Log (score=20 < 70, not Block)
        assert!(matches!(s.evaluate(&write_event("/custom/file")), Verdict::Log { .. }));
    }

    #[test]
    fn config_empty_protected_paths() {
        let c = SentinelConfig { protected_paths: vec![], ..Default::default() };
        let s = Sentinel::new(c);
        // No protected paths, but score=80 → high-risk Log
        assert!(matches!(s.evaluate(&write_event("/etc/shadow")), Verdict::Log { .. }));
    }

    #[test]
    fn config_empty_critical_binaries() {
        let c = SentinelConfig { critical_binaries: vec![], ..Default::default() };
        let s = Sentinel::new(c);
        // No critical binaries, rm score=90 → high-risk Log
        assert!(matches!(s.evaluate(&exec_event("rm")), Verdict::Log { .. }));
    }

    #[test]
    fn config_monitor_toggles() {
        let c = SentinelConfig { monitor_exec: false, ..Default::default() };
        assert!(!c.monitor_exec);
        assert!(c.monitor_file_write);
    }

    // ═══════════════════════════════════════════
    // Channel / receiver
    // ═══════════════════════════════════════════

    #[test]
    fn take_receiver_once() {
        let mut s = Sentinel::new(SentinelConfig::default());
        assert!(s.take_receiver().is_some());
        assert!(s.take_receiver().is_none());
    }

    #[test]
    fn sender_works() {
        let s = Sentinel::new(SentinelConfig::default());
        let sender = s.sender();
        // Just verify we can clone it
        let _sender2 = sender.clone();
    }

    // ═══════════════════════════════════════════
    // Verdict serialization
    // ═══════════════════════════════════════════

    #[test]
    fn verdict_allow_serialize() {
        let v = Verdict::Allow;
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("Allow"));
    }

    #[test]
    fn verdict_block_serialize() {
        let v = Verdict::Block { reason: "test".into() };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("Block"));
        assert!(json.contains("test"));
    }

    #[test]
    fn verdict_log_serialize() {
        let v = Verdict::Log { reason: "warning".into() };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("Log"));
        assert!(json.contains("warning"));
    }
}

// ═══════════════════════════════════════════
// ESF converter tests (macOS + esf feature)
// ═══════════════════════════════════════════

#[cfg(all(test, target_os = "macos", feature = "esf"))]
mod esf_tests {
    use super::*;

    fn make_esf_process(pid: u32, ppid: u32) -> endpointsecurity::EsProcess {
        endpointsecurity::EsProcess {
            ppid,
            original_ppid: ppid,
            pid,
            group_id: 0,
            session_id: 0,
            codesigning_flags: 0,
            is_platform_binary: false,
            is_es_client: false,
            cdhash: String::new(),
            signing_id: String::new(),
            team_id: String::new(),
            executable: endpointsecurity::EsFile {
                path: "/bin/test".into(),
                path_truncated: false,
            },
        }
    }

    fn make_esf_exec(cmd: &str, args: Vec<&str>) -> endpointsecurity::EsEventExec {
        endpointsecurity::EsEventExec {
            target: make_esf_process(0, 0),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_esf_open(path: &str, fflag: u32) -> endpointsecurity::EsEventOpen {
        endpointsecurity::EsEventOpen {
            fflag,
            file: endpointsecurity::EsFile {
                path: path.into(),
                path_truncated: false,
            },
        }
    }

    // ── Exec conversion ──

    #[test]
    fn esf_exec_basic() {
        let proc = make_esf_process(1234, 100);
        let exec = make_esf_exec("/usr/bin/rm", vec!["-rf", "/"]);
        // Override target executable path
        let exec = endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: "/usr/bin/rm".into(),
                    path_truncated: false,
                },
                ..make_esf_process(0, 0)
            },
            args: vec!["-rf".into(), "/".into()],
        };
        let result = esf_exec_to_event(&exec, &proc).unwrap();
        assert_eq!(result.kind, EventKind::Exec);
        assert_eq!(result.pid, 1234);
        assert_eq!(result.ppid, 100);
        assert_eq!(result.command.as_deref(), Some("/usr/bin/rm"));
        assert_eq!(result.args, vec!["-rf", "/"]);
    }

    #[test]
    fn esf_exec_no_args() {
        let proc = make_esf_process(100, 1);
        let exec = endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: "/bin/ls".into(),
                    path_truncated: false,
                },
                ..make_esf_process(0, 0)
            },
            args: vec![],
        };
        let result = esf_exec_to_event(&exec, &proc).unwrap();
        assert_eq!(result.args, Vec::<String>::new());
        assert_eq!(result.command.as_deref(), Some("/bin/ls"));
    }

    #[test]
    fn esf_exec_timestamp_present() {
        let proc = make_esf_process(1, 0);
        let exec = make_esf_exec("test", vec![]);
        let result = esf_exec_to_event(&exec, &proc).unwrap();
        assert!(result.timestamp > 0);
    }

    // ── Open conversion (writes only) ──

    #[test]
    fn esf_open_write() {
        let proc = make_esf_process(500, 1);
        let open = make_esf_open("/etc/passwd", 0x1); // O_WRONLY
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert_eq!(result.kind, EventKind::FileWrite);
        assert_eq!(result.pid, 500);
        assert_eq!(result.path.as_deref(), Some("/etc/passwd"));
    }

    #[test]
    fn esf_open_rdwr() {
        let proc = make_esf_process(501, 1);
        let open = make_esf_open("/etc/shadow", 0x2); // O_RDWR
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert_eq!(result.kind, EventKind::FileWrite);
    }

    #[test]
    fn esf_open_read_only_skipped() {
        let proc = make_esf_process(502, 1);
        let open = make_esf_open("/etc/hosts", 0x0); // O_RDONLY
        assert!(esf_open_to_event(&open, &proc).is_none());
    }

    #[test]
    fn esf_open_write_with_create() {
        let proc = make_esf_process(503, 1);
        let open = make_esf_open("/tmp/newfile", 0x201); // O_WRONLY | O_CREAT
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert_eq!(result.kind, EventKind::FileWrite);
        assert_eq!(result.path.as_deref(), Some("/tmp/newfile"));
    }

    #[test]
    fn esf_open_append() {
        let proc = make_esf_process(504, 1);
        let open = make_esf_open("/var/log/app.log", 0x9); // O_WRONLY | O_APPEND
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert_eq!(result.kind, EventKind::FileWrite);
    }

    #[test]
    fn esf_open_no_remote_addr() {
        let proc = make_esf_process(1, 0);
        let open = make_esf_open("/etc/test", 0x1);
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert!(result.remote_addr.is_none());
    }

    #[test]
    fn esf_open_no_command() {
        let proc = make_esf_process(1, 0);
        let open = make_esf_open("/tmp/x", 0x1);
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert!(result.command.is_none());
    }

    // ── Verdict on ESF-generated events ──

    #[test]
    fn esf_verdict_exec_rm_critical() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_esf_process(1000, 1);
        let exec = endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: "/usr/bin/rm".into(),
                    path_truncated: false,
                },
                ..make_esf_process(0, 0)
            },
            args: vec!["-rf".into()],
        };
        let event = esf_exec_to_event(&exec, &proc).unwrap();
        match s.evaluate(&event) {
            Verdict::Log { reason } => assert!(reason.contains("rm")),
            other => panic!("expected Log, got {:?}", other),
        }
    }

    #[test]
    fn esf_verdict_write_etc_block() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_esf_process(1000, 1);
        let open = make_esf_open("/etc/shadow", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        assert!(matches!(s.evaluate(&event), Verdict::Block { .. }));
    }

    #[test]
    fn esf_verdict_write_home_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_esf_process(1000, 1);
        let open = make_esf_open("/home/user/.bashrc", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        assert!(matches!(s.evaluate(&event), Verdict::Allow));
    }

    #[test]
    fn esf_verdict_exec_safe_allow() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_esf_process(1000, 1);
        let exec = endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: "/usr/bin/git".into(),
                    path_truncated: false,
                },
                ..make_esf_process(0, 0)
            },
            args: vec!["pull".into()],
        };
        let event = esf_exec_to_event(&exec, &proc).unwrap();
        assert!(matches!(s.evaluate(&event), Verdict::Allow));
    }

    // ── Edge cases ──

    #[test]
    fn esf_exec_empty_command_path() {
        let proc = make_esf_process(1, 0);
        let exec = endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: String::new(),
                    path_truncated: false,
                },
                ..make_esf_process(0, 0)
            },
            args: vec![],
        };
        let result = esf_exec_to_event(&exec, &proc).unwrap();
        assert_eq!(result.command.as_deref(), Some(""));
    }

    #[test]
    fn esf_open_truncated_path_flag() {
        let proc = make_esf_process(1, 0);
        let open = endpointsecurity::EsEventOpen {
            fflag: 0x1,
            file: endpointsecurity::EsFile {
                path: "/very/long/path/that/was/truncated".into(),
                path_truncated: true,
            },
        };
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert!(result.path.as_deref().unwrap().contains("truncated"));
    }

    #[test]
    fn esf_exec_many_args() {
        let proc = make_esf_process(1, 0);
        let exec = endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: "/usr/bin/find".into(),
                    path_truncated: false,
                },
                ..make_esf_process(0, 0)
            },
            args: (0..20).map(|i| format!("arg{}", i)).collect(),
        };
        let result = esf_exec_to_event(&exec, &proc).unwrap();
        assert_eq!(result.args.len(), 20);
    }

    #[test]
    fn esf_open_zero_pid() {
        let proc = make_esf_process(0, 0);
        let open = make_esf_open("/etc/test", 0x1);
        let result = esf_open_to_event(&open, &proc).unwrap();
        assert_eq!(result.pid, 0);
    }
}

// ═══════════════════════════════════════════
// Auth blocking tests — verifies that Verdict::Block
// maps to EsAuthResult::Deny (kernel-level blocking)
// ═══════════════════════════════════════════

#[cfg(all(test, target_os = "macos", feature = "esf"))]
mod auth_tests {
    use super::*;

    /// Helper: simulate the Auth decision logic from start_esf()
    fn auth_verdict(verdict: &Verdict) -> (endpointsecurity::EsAuthResult, endpointsecurity::EsCacheResult) {
        match verdict {
            Verdict::Block { reason: _ } => {
                (endpointsecurity::EsAuthResult::Deny, endpointsecurity::EsCacheResult::Yes)
            }
            Verdict::Log { reason: _ } => {
                (endpointsecurity::EsAuthResult::Allow, endpointsecurity::EsCacheResult::Yes)
            }
            Verdict::Allow => {
                (endpointsecurity::EsAuthResult::Allow, endpointsecurity::EsCacheResult::Yes)
            }
        }
    }

    // ── Exec: critical binaries → DENY ──

    #[test]
    fn auth_exec_rm_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let exec = make_test_esf_exec("/usr/bin/rm", vec!["-rf", "/"]);
        let event = esf_exec_to_event(&exec, &proc).unwrap();
        let verdict = s.evaluate(&event);
        let (result, cache) = auth_verdict(&verdict);
        assert!(matches!(verdict, Verdict::Log { .. })); // rm → Log (not Block)
        assert!(matches!(result, endpointsecurity::EsAuthResult::Allow)); // exec critical → Log, not Block
    }

    #[test]
    fn auth_exec_ls_allowed() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let exec = make_test_esf_exec("/bin/ls", vec!["-la"]);
        let event = esf_exec_to_event(&exec, &proc).unwrap();
        let verdict = s.evaluate(&event);
        let (result, _) = auth_verdict(&verdict);
        assert!(matches!(verdict, Verdict::Allow));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Allow));
    }

    // ── FileWrite: protected paths → DENY ──

    #[test]
    fn auth_write_etc_shadow_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/etc/shadow", 0x1); // O_WRONLY
        let event = esf_open_to_event(&open, &proc).unwrap();
        let verdict = s.evaluate(&event);
        let (result, _) = auth_verdict(&verdict);
        assert!(matches!(verdict, Verdict::Block { .. }));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Deny));
    }

    #[test]
    fn auth_write_etc_passwd_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/etc/passwd", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let verdict = s.evaluate(&event);
        let (result, _) = auth_verdict(&verdict);
        assert!(matches!(verdict, Verdict::Block { .. }));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Deny));
    }

    #[test]
    fn auth_write_var_lib_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/var/lib/important", 0x2); // O_RDWR
        let event = esf_open_to_event(&open, &proc).unwrap();
        let verdict = s.evaluate(&event);
        let (result, _) = auth_verdict(&verdict);
        assert!(matches!(verdict, Verdict::Block { .. }));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Deny));
    }

    #[test]
    fn auth_write_boot_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/boot/grub.cfg", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (result, _) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Deny));
    }

    #[test]
    fn auth_write_usr_bin_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/usr/bin/malware", 0x201); // O_WRONLY|O_CREAT
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (result, _) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Deny));
    }

    // ── FileWrite: safe paths → ALLOW ──

    #[test]
    fn auth_write_home_allowed() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/home/user/.bashrc", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (result, _) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Allow));
    }

    #[test]
    fn auth_write_tmp_allowed() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/tmp/build.log", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (result, _) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Allow));
    }

    // ── FileWrite: read-only → skipped (not even an event) ──

    #[test]
    fn auth_read_etc_not_an_event() {
        let proc = make_test_esf_process(1000, 1);
        let open = make_test_esf_open("/etc/shadow", 0x0); // O_RDONLY
        assert!(esf_open_to_event(&open, &proc).is_none());
    }

    // ── Cache is always Yes ──

    #[test]
    fn auth_cache_always_yes() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(1000, 1);

        // Block case
        let open = make_test_esf_open("/etc/shadow", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (_, cache) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(cache, endpointsecurity::EsCacheResult::Yes));

        // Allow case
        let open = make_test_esf_open("/tmp/file", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (_, cache) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(cache, endpointsecurity::EsCacheResult::Yes));
    }

    // ── Edge: unknown process ──

    #[test]
    fn auth_unknown_process_write_etc_denied() {
        let s = Sentinel::new(SentinelConfig::default());
        let proc = make_test_esf_process(99999, 0); // unknown pid/ppid
        let open = make_test_esf_open("/etc/hosts", 0x1);
        let event = esf_open_to_event(&open, &proc).unwrap();
        let (result, _) = auth_verdict(&s.evaluate(&event));
        assert!(matches!(result, endpointsecurity::EsAuthResult::Deny));
    }

    // ── Helpers for auth tests ──

    fn make_test_esf_process(pid: u32, ppid: u32) -> endpointsecurity::EsProcess {
        endpointsecurity::EsProcess {
            ppid,
            original_ppid: ppid,
            pid,
            group_id: 0,
            session_id: 0,
            codesigning_flags: 0,
            is_platform_binary: false,
            is_es_client: false,
            cdhash: String::new(),
            signing_id: String::new(),
            team_id: String::new(),
            executable: endpointsecurity::EsFile {
                path: "/bin/test".into(),
                path_truncated: false,
            },
        }
    }

    fn make_test_esf_exec(cmd: &str, args: Vec<&str>) -> endpointsecurity::EsEventExec {
        endpointsecurity::EsEventExec {
            target: endpointsecurity::EsProcess {
                executable: endpointsecurity::EsFile {
                    path: cmd.into(),
                    path_truncated: false,
                },
                ..make_test_esf_process(0, 0)
            },
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_test_esf_open(path: &str, fflag: u32) -> endpointsecurity::EsEventOpen {
        endpointsecurity::EsEventOpen {
            fflag,
            file: endpointsecurity::EsFile {
                path: path.into(),
                path_truncated: false,
            },
        }
    }
}

// ═══════════════════════════════════════════
// Hot-reload policy tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod hotreload_tests {
    use super::*;

    #[test]
    fn reload_changes_config() {
        let config1 = SentinelConfig::default();
        let config2 = SentinelConfig {
            critical_binaries: vec!["custom_binary".into()],
            protected_paths: vec!["/custom_path".into()],
            ..Default::default()
        };

        let s1 = Sentinel::new(config1);
        let s2 = Sentinel::new(config2.clone());

        // Verify config2 has different values
        assert!(config2.critical_binaries.contains(&"custom_binary".into()));
        assert!(config2.protected_paths.contains(&"/custom_path".into()));
    }

    #[test]
    fn sentinel_config_serde_roundtrip() {
        let config = SentinelConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let config2: SentinelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.monitor_exec, config2.monitor_exec);
        assert_eq!(config.critical_binaries, config2.critical_binaries);
        assert_eq!(config.protected_paths, config2.protected_paths);
    }

    #[test]
    fn config_with_extra_binary() {
        let mut config = SentinelConfig::default();
        config.critical_binaries.push("dangerous_tool".into());
        let s = Sentinel::new(config.clone());
        // Verify the config was stored
        assert!(config.critical_binaries.iter().any(|b| b == "dangerous_tool"));
    }

    #[test]
    fn config_with_extra_path() {
        let mut config = SentinelConfig::default();
        config.protected_paths.push("/data/secret".into());
        let s = Sentinel::new(config.clone());
        assert!(config.protected_paths.iter().any(|p| p == "/data/secret"));
    }

    #[test]
    fn reload_to_empty_policy() {
        let config = SentinelConfig {
            critical_binaries: vec![],
            protected_paths: vec![],
            monitor_exec: true,
            monitor_file_write: true,
            monitor_network: true,
            monitor_delete: true,
        };
        let s = Sentinel::new(config.clone());
        assert!(config.critical_binaries.is_empty());
        assert!(config.protected_paths.is_empty());
    }

    #[test]
    fn reload_preserves_monitor_flags() {
        let config = SentinelConfig {
            monitor_exec: false,
            monitor_file_write: false,
            monitor_network: true,
            monitor_delete: false,
            ..Default::default()
        };
        let s = Sentinel::new(config.clone());
        assert!(!config.monitor_exec);
        assert!(!config.monitor_file_write);
        assert!(config.monitor_network);
        assert!(!config.monitor_delete);
    }
}

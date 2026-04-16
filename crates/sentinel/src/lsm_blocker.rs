//! LSM BPF loader — loads blocking programs into kernel LSM hooks
//!
//! Uses libbpf (via libbpf-sys) for reliable BPF object loading.
//! libbpf is the reference BPF loader — handles any clang-generated ELF.
//!
//! Requires: CONFIG_BPF_LSM=y, kernel >= 5.7, "bpf" in LSM list

#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
mod inner {
    use anyhow::{Context, Result};
    use libbpf_sys::*;
    use std::collections::HashSet;
    use std::ffi::CString;
    use std::mem::size_of;
    use std::ptr;

    const MAX_COMM_LEN: usize = 64;
    const MAX_PATH_LEN: usize = 128;

    // BPF map key/value structs — must match sentinel_lsm.bpf.c exactly
    #[derive(Debug, Copy, Clone)]
    #[repr(C)]
    struct CmdPolicyKey {
        comm: [u8; MAX_COMM_LEN],
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    struct CmdPolicyValue {
        action: u32,
    }

    #[derive(Debug, Copy, Clone)]
    #[repr(C)]
    struct PathPolicyKey {
        prefix: [u8; MAX_PATH_LEN],
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    struct PathPolicyValue {
        action: u32,
    }

    /// Manages LSM BPF programs and their policy maps via libbpf.
    pub struct LsmBlocker {
        obj: *mut bpf_object,
        // Keep links alive — if dropped, LSM programs detach!
        links: Vec<*mut bpf_link>,
        // FDs for maps — used for bpf_map_lookup/update/delete
        cmd_map_fd: i32,
        path_map_fd: i32,
        blocked_pids_fd: i32,
        whitelist_fd: i32,
        // Local tracking for introspection
        blocked_commands: HashSet<String>,
        protected_paths: HashSet<String>,
        whitelisted_pids: HashSet<u32>,
        blocked_pids: HashSet<u32>,
    }

    // libbpf object is thread-safe for map operations
    unsafe impl Send for LsmBlocker {}
    unsafe impl Sync for LsmBlocker {}

    impl LsmBlocker {
        /// Load LSM BPF programs from embedded object file
        pub fn load() -> Result<Self> {
            let bpf_bytes = include_bytes!("../bpf/sentinel_lsm.bpf.o");

            // Create libbpf object from embedded bytes
            let obj_opts = bpf_object_open_opts {
                sz: size_of::<bpf_object_open_opts>() as u64,
                object_name: ptr::null(),
                ..unsafe { std::mem::zeroed() }
            };

            let obj = unsafe {
                // libbpf needs a NUL-terminated path or memory buffer
                // Use bpf_object__open_mem for in-memory loading
                bpf_object__open_mem(
                    bpf_bytes.as_ptr() as *const libc::c_void,
                    bpf_bytes.len() as u64,
                    &obj_opts,
                )
            };

            if obj.is_null() {
                return Err(anyhow::anyhow!(
                    "libbpf failed to open BPF object"
                ));
            }

            // Load all programs into kernel
            let ret = unsafe { bpf_object__load(obj) };
            if ret != 0 {
                let err = unsafe { libc::__errno_location() };
                let errno = unsafe { *err };
                // Clean up
                unsafe { bpf_object__close(obj) };
                return Err(anyhow::anyhow!(
                    "libbpf failed to load BPF programs (errno={}: {})",
                    errno,
                    std::io::Error::from_raw_os_error(errno)
                ));
            }

            // Get map FDs
            let cmd_map_fd = Self::get_map_fd(obj, "blocked_commands")?;
            let path_map_fd = Self::get_map_fd(obj, "protected_paths")?;
            let blocked_pids_fd = Self::get_map_fd(obj, "blocked_pids")?;
            let whitelist_fd = Self::get_map_fd(obj, "whitelist_pids")?;


            // Attach LSM programs
            let lsm_progs = ["block_exec", "block_file_open", "monitor_unlink", "monitor_bind"];
            let mut links: Vec<*mut bpf_link> = Vec::new();
            let mut attached = 0;
            for prog_name in &lsm_progs {
                let c_name = CString::new(*prog_name).unwrap();
                let prog = unsafe { bpf_object__find_program_by_name(obj, c_name.as_ptr()) };
                if prog.is_null() {
                    tracing::warn!("LSM program '{}' not found", prog_name);
                    continue;
                }
                let link = unsafe { bpf_program__attach(prog) };
                if link.is_null() {
                    let errno = unsafe { *libc::__errno_location() };
                    tracing::warn!("Failed to attach LSM '{}': errno={}", prog_name, errno);
                } else {
                    links.push(link);
                    attached += 1;
                    tracing::info!("Attached LSM: {}", prog_name);
                }
            }
            if attached == 0 {
                unsafe { bpf_object__close(obj); }
                return Err(anyhow::anyhow!("No LSM programs attached"));
            }
            tracing::info!("🔒 LSM BPF blocker loaded via libbpf — kernel-level blocking active");

            Ok(Self {
                obj,
                links,
                cmd_map_fd,
                path_map_fd,
                blocked_pids_fd,
                whitelist_fd,
                blocked_commands: HashSet::new(),
                protected_paths: HashSet::new(),
                whitelisted_pids: HashSet::new(),
                blocked_pids: HashSet::new(),
            })
        }

        fn get_map_fd(obj: *mut bpf_object, name: &str) -> Result<i32> {
            let c_name = CString::new(name).context("map name")?;
            let map = unsafe { bpf_object__find_map_by_name(obj, c_name.as_ptr()) };
            if map.is_null() {
                // Clean up on error
                unsafe { bpf_object__close(obj) };
                return Err(anyhow::anyhow!("BPF map '{}' not found", name));
            }
            let fd = unsafe { bpf_map__fd(map) };
            if fd < 0 {
                unsafe { bpf_object__close(obj) };
                return Err(anyhow::anyhow!("Failed to get FD for map '{}'", name));
            }
            Ok(fd)
        }

        // ── BPF map helpers ──────────────────────────────────────────

        fn map_update<K, V>(&self, fd: i32, key: &K, value: &V) -> Result<()> {
            let ret = unsafe {
                bpf_map_update_elem(
                    fd as i32,
                    key as *const K as *const libc::c_void,
                    value as *const V as *const libc::c_void,
                    BPF_ANY as u64,
                )
            };
            if ret != 0 {
                let errno = unsafe { *libc::__errno_location() };
                return Err(anyhow::anyhow!(
                    "bpf_map_update_elem failed (errno={})", errno
                ));
            }
            Ok(())
        }

        fn map_delete<K>(&self, fd: i32, key: &K) -> Result<()> {
            let ret = unsafe {
                bpf_map_delete_elem(
                    fd as i32,
                    key as *const K as *const libc::c_void,
                )
            };
            if ret != 0 {
                let errno = unsafe { *libc::__errno_location() };
                return Err(anyhow::anyhow!(
                    "bpf_map_delete_elem failed (errno={})", errno
                ));
            }
            Ok(())
        }

        // ── Hot-reload policy methods ────────────────────────────────

        pub fn block_command(&mut self, cmd: &str) -> Result<()> {
            let key = CmdPolicyKey { comm: str_to_fixed(cmd) };
            let val = CmdPolicyValue { action: 1 };
            self.map_update(self.cmd_map_fd, &key, &val)?;
            self.blocked_commands.insert(cmd.to_string());
            tracing::info!(action = "block_command", command = cmd);
            Ok(())
        }

        pub fn unblock_command(&mut self, cmd: &str) -> Result<()> {
            let key = CmdPolicyKey { comm: str_to_fixed(cmd) };
            self.map_delete(self.cmd_map_fd, &key)?;
            self.blocked_commands.remove(cmd);
            tracing::info!(action = "unblock_command", command = cmd);
            Ok(())
        }

        pub fn protect_path(&mut self, path: &str) -> Result<()> {
            let key = PathPolicyKey { prefix: str_to_fixed(path) };
            let val = PathPolicyValue { action: 1 };
            self.map_update(self.path_map_fd, &key, &val)?;
            self.protected_paths.insert(path.to_string());
            tracing::info!(action = "protect_path", path = path);
            Ok(())
        }

        pub fn unprotect_path(&mut self, path: &str) -> Result<()> {
            let key = PathPolicyKey { prefix: str_to_fixed(path) };
            self.map_delete(self.path_map_fd, &key)?;
            self.protected_paths.remove(path);
            tracing::info!(action = "unprotect_path", path = path);
            Ok(())
        }

        pub fn whitelist_pid(&mut self, pid: u32) -> Result<()> {
            let val: u32 = 1;
            self.map_update(self.whitelist_fd, &pid, &val)?;
            self.whitelisted_pids.insert(pid);
            tracing::info!(action = "whitelist_pid", pid = pid);
            Ok(())
        }

        pub fn unwhitelist_pid(&mut self, pid: u32) -> Result<()> {
            self.map_delete(self.whitelist_fd, &pid)?;
            self.whitelisted_pids.remove(&pid);
            Ok(())
        }

        pub fn block_pid(&mut self, pid: u32) -> Result<()> {
            let val: u32 = 1;
            self.map_update(self.blocked_pids_fd, &pid, &val)?;
            self.blocked_pids.insert(pid);
            tracing::info!(action = "block_pid", pid = pid);
            Ok(())
        }

        pub fn unblock_pid(&mut self, pid: u32) -> Result<()> {
            self.map_delete(self.blocked_pids_fd, &pid)?;
            self.blocked_pids.remove(&pid);
            Ok(())
        }

        pub fn reload_config(&mut self, config: &crate::SentinelConfig) -> Result<ReloadStats> {
            let mut stats = ReloadStats::default();

            for cmd in &self.blocked_commands.clone() {
                self.unblock_command(cmd)?;
                stats.commands_removed += 1;
            }
            for cmd in &config.critical_binaries {
                self.block_command(cmd)?;
                stats.commands_added += 1;
            }

            for path in &self.protected_paths.clone() {
                self.unprotect_path(path)?;
                stats.paths_removed += 1;
            }
            for path in &config.protected_paths {
                self.protect_path(path)?;
                stats.paths_added += 1;
            }

            self.whitelist_pid(std::process::id())?;

            tracing::info!(
                "🔄 Policy hot-reloaded: +{}/{} commands, +{}/{} paths",
                stats.commands_added, stats.commands_removed,
                stats.paths_added, stats.paths_removed
            );
            Ok(stats)
        }

        pub fn load_config(&mut self, config: &crate::SentinelConfig) -> Result<()> {
            self.reload_config(config)?;
            Ok(())
        }

        // ── Introspection ──────────────────────────────────────────────

        pub fn blocked_commands(&self) -> &HashSet<String> { &self.blocked_commands }
        pub fn protected_paths(&self) -> &HashSet<String> { &self.protected_paths }
        pub fn whitelisted_pids(&self) -> &HashSet<u32> { &self.whitelisted_pids }
        pub fn blocked_pids(&self) -> &HashSet<u32> { &self.blocked_pids }

        pub fn policy_snapshot(&self) -> PolicySnapshot {
            PolicySnapshot {
                blocked_commands: self.blocked_commands.iter().cloned().collect(),
                protected_paths: self.protected_paths.iter().cloned().collect(),
                whitelisted_pids: self.whitelisted_pids.iter().copied().collect(),
                blocked_pids: self.blocked_pids.iter().copied().collect(),
            }
        }
    }

    impl Drop for LsmBlocker {
        fn drop(&mut self) {
            if !self.obj.is_null() {
                unsafe { bpf_object__close(self.obj) };
            }
        }
    }

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct PolicySnapshot {
        pub blocked_commands: Vec<String>,
        pub protected_paths: Vec<String>,
        pub whitelisted_pids: Vec<u32>,
        pub blocked_pids: Vec<u32>,
    }

    #[derive(Debug, Default)]
    pub struct ReloadStats {
        pub commands_added: usize,
        pub commands_removed: usize,
        pub paths_added: usize,
        pub paths_removed: usize,
    }

    fn str_to_fixed<const N: usize>(s: &str) -> [u8; N] {
        let mut buf = [0u8; N];
        let bytes = s.as_bytes();
        let len = bytes.len().min(N - 1);
        buf[..len].copy_from_slice(&bytes[..len]);
        buf
    }
}

#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
pub use inner::{LsmBlocker, PolicySnapshot, ReloadStats};

#[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
pub struct LsmBlocker;

#[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
impl LsmBlocker {
    pub fn load() -> anyhow::Result<Self> {
        Err(anyhow::anyhow!("LSM BPF requires Linux with linux-ebpf feature"))
    }
}

#[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
pub struct PolicySnapshot;

#[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
pub struct ReloadStats;

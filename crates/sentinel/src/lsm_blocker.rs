//! LSM BPF loader — loads blocking programs into kernel LSM hooks
//!
//! Requires: CONFIG_BPF_LSM=y, kernel >= 5.7, "bpf" in LSM list
//!
//! Hot-reload: all policy methods can be called at any time after `load()`.
//! No restart needed — maps are updated in-place in the kernel.

#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
mod inner {
    use anyhow::Result;
    use aya::maps::HashMap;
    use aya::programs::Lsm;
    use aya::Bpf;
    use std::collections::HashSet;

    const MAX_COMM_LEN: usize = 64;
    const MAX_PATH_LEN: usize = 128;

    #[derive(Debug)]
    #[repr(C)]
    struct CmdPolicyKey {
        comm: [u8; MAX_COMM_LEN],
    }

    #[derive(Debug, Clone)]
    #[repr(C)]
    struct CmdPolicyValue {
        action: u32,
    }

    #[derive(Debug)]
    #[repr(C)]
    struct PathPolicyKey {
        prefix: [u8; MAX_PATH_LEN],
    }

    #[derive(Debug, Clone)]
    #[repr(C)]
    struct PathPolicyValue {
        action: u32,
    }

    /// Manages LSM BPF programs and their policy maps.
    /// Thread-safe: wrap in `Arc<Mutex<LsmBlocker>>` for concurrent access.
    pub struct LsmBlocker {
        _bpf: Bpf,
        cmd_map: HashMap<CmdPolicyKey, CmdPolicyValue>,
        path_map: HashMap<PathPolicyKey, PathPolicyValue>,
        blocked_pids_map: HashMap<u32, u32>,
        whitelist_map: HashMap<u32, u32>,
        // Local tracking for introspection
        blocked_commands: HashSet<String>,
        protected_paths: HashSet<String>,
        whitelisted_pids: HashSet<u32>,
        blocked_pids: HashSet<u32>,
    }

    impl LsmBlocker {
        /// Load LSM BPF programs from embedded object
        pub fn load() -> Result<Self> {
            let bpf_bytes = include_bytes!("../../bpf/sentinel_lsm.bpf.o");
            let mut bpf = Bpf::load(bpf_bytes)
                .map_err(|e| anyhow::anyhow!("Failed to load LSM BPF: {}", e))?;

            // Attach LSM programs
            let programs = ["block_exec", "block_file_open", "monitor_unlink", "monitor_bind"];
            for prog_name in &programs {
                let program: &mut Lsm = bpf
                    .program_mut(prog_name)
                    .ok_or_else(|| anyhow::anyhow!("LSM program '{}' not found", prog_name))?
                    .try_into()
                    .map_err(|e: aya::programs::ProgramError| {
                        anyhow::anyhow!("'{}' is not an LSM program: {}", prog_name, e)
                    })?;
                program
                    .load("sentinel_lsm", &aya::programs::ProgramId(0))
                    .map_err(|e| anyhow::anyhow!("Failed to load LSM '{}': {}", prog_name, e))?;
                program
                    .attach()
                    .map_err(|e| anyhow::anyhow!("Failed to attach LSM '{}': {}", prog_name, e))?;
            }

            // Take maps from BPF object (we own them now)
            let cmd_map: HashMap<_, CmdPolicyKey, CmdPolicyValue> = bpf
                .take_map("blocked_commands")
                .ok_or_else(|| anyhow::anyhow!("blocked_commands map not found"))?
                .try_into()
                .map_err(|e| anyhow::anyhow!("blocked_commands: {}", e))?;
            let path_map: HashMap<_, PathPolicyKey, PathPolicyValue> = bpf
                .take_map("protected_paths")
                .ok_or_else(|| anyhow::anyhow!("protected_paths map not found"))?
                .try_into()
                .map_err(|e| anyhow::anyhow!("protected_paths: {}", e))?;
            let blocked_pids_map: HashMap<_, u32, u32> = bpf
                .take_map("blocked_pids")
                .ok_or_else(|| anyhow::anyhow!("blocked_pids map not found"))?
                .try_into()
                .map_err(|e| anyhow::anyhow!("blocked_pids: {}", e))?;
            let whitelist_map: HashMap<_, u32, u32> = bpf
                .take_map("whitelist_pids")
                .ok_or_else(|| anyhow::anyhow!("whitelist_pids map not found"))?
                .try_into()
                .map_err(|e| anyhow::anyhow!("whitelist_pids: {}", e))?;

            tracing::info!("🔒 LSM BPF blocker loaded — kernel-level blocking active");

            Ok(Self {
                _bpf: bpf,
                cmd_map,
                path_map,
                blocked_pids_map,
                whitelist_map,
                blocked_commands: HashSet::new(),
                protected_paths: HashSet::new(),
                whitelisted_pids: HashSet::new(),
                blocked_pids: HashSet::new(),
            })
        }

        // ── Hot-reload policy methods ──────────────────────────────────

        /// Block a command system-wide (e.g., "rm", "mkfs")
        pub fn block_command(&mut self, cmd: &str) -> Result<()> {
            let key = CmdPolicyKey {
                comm: str_to_fixed::<MAX_COMM_LEN>(cmd),
            };
            self.cmd_map
                .insert(key, CmdPolicyValue { action: 1 }, 0)
                .map_err(|e| anyhow::anyhow!("Failed to block command: {}", e))?;
            self.blocked_commands.insert(cmd.to_string());
            tracing::info!(action = "block_command", command = cmd);
            Ok(())
        }

        /// Unblock a previously blocked command
        pub fn unblock_command(&mut self, cmd: &str) -> Result<()> {
            let key = CmdPolicyKey {
                comm: str_to_fixed::<MAX_COMM_LEN>(cmd),
            };
            self.cmd_map
                .remove(&key)
                .map_err(|e| anyhow::anyhow!("Failed to unblock command: {}", e))?;
            self.blocked_commands.remove(cmd);
            tracing::info!(action = "unblock_command", command = cmd);
            Ok(())
        }

        /// Protect a path prefix (block writes)
        pub fn protect_path(&mut self, path: &str) -> Result<()> {
            let key = PathPolicyKey {
                prefix: str_to_fixed::<MAX_PATH_LEN>(path),
            };
            self.path_map
                .insert(key, PathPolicyValue { action: 1 }, 0)
                .map_err(|e| anyhow::anyhow!("Failed to protect path: {}", e))?;
            self.protected_paths.insert(path.to_string());
            tracing::info!(action = "protect_path", path = path);
            Ok(())
        }

        /// Unprotect a previously protected path
        pub fn unprotect_path(&mut self, path: &str) -> Result<()> {
            let key = PathPolicyKey {
                prefix: str_to_fixed::<MAX_PATH_LEN>(path),
            };
            self.path_map
                .remove(&key)
                .map_err(|e| anyhow::anyhow!("Failed to unprotect path: {}", e))?;
            self.protected_paths.remove(path);
            tracing::info!(action = "unprotect_path", path = path);
            Ok(())
        }

        /// Whitelist a PID (bypass all checks)
        pub fn whitelist_pid(&mut self, pid: u32) -> Result<()> {
            self.whitelist_map
                .insert(pid, 1, 0)
                .map_err(|e| anyhow::anyhow!("Failed to whitelist PID: {}", e))?;
            self.whitelisted_pids.insert(pid);
            tracing::info!(action = "whitelist_pid", pid = pid);
            Ok(())
        }

        /// Remove PID from whitelist
        pub fn unwhitelist_pid(&mut self, pid: u32) -> Result<()> {
            self.whitelist_map
                .remove(&pid)
                .map_err(|e| anyhow::anyhow!("Failed to unwhitelist PID: {}", e))?;
            self.whitelisted_pids.remove(&pid);
            Ok(())
        }

        /// Block a specific PID (deny all operations)
        pub fn block_pid(&mut self, pid: u32) -> Result<()> {
            self.blocked_pids_map
                .insert(pid, 1, 0)
                .map_err(|e| anyhow::anyhow!("Failed to block PID: {}", e))?;
            self.blocked_pids.insert(pid);
            tracing::info!(action = "block_pid", pid = pid);
            Ok(())
        }

        /// Unblock a previously blocked PID
        pub fn unblock_pid(&mut self, pid: u32) -> Result<()> {
            self.blocked_pids_map
                .remove(&pid)
                .map_err(|e| anyhow::anyhow!("Failed to unblock PID: {}", e))?;
            self.blocked_pids.remove(&pid);
            Ok(())
        }

        /// Hot-reload: replace entire policy from config
        pub fn reload_config(&mut self, config: &crate::SentinelConfig) -> Result<ReloadStats> {
            let mut stats = ReloadStats::default();

            // Clear and rebuild command blocklist
            for cmd in &self.blocked_commands.clone() {
                self.unblock_command(cmd)?;
                stats.commands_removed += 1;
            }
            for cmd in &config.critical_binaries {
                self.block_command(cmd)?;
                stats.commands_added += 1;
            }

            // Clear and rebuild protected paths
            for path in &self.protected_paths.clone() {
                self.unprotect_path(path)?;
                stats.paths_removed += 1;
            }
            for path in &config.protected_paths {
                self.protect_path(path)?;
                stats.paths_added += 1;
            }

            // Re-whitelist own PID
            self.whitelist_pid(std::process::id())?;

            tracing::info!(
                "🔄 Policy hot-reloaded: +{}/{} commands, +{}/{} paths",
                stats.commands_added, stats.commands_removed,
                stats.paths_added, stats.paths_removed
            );
            Ok(stats)
        }

        /// Load initial config (same as reload_config but for first load)
        pub fn load_config(&mut self, config: &crate::SentinelConfig) -> Result<()> {
            self.reload_config(config)?;
            Ok(())
        }

        // ── Introspection ──────────────────────────────────────────────

        pub fn blocked_commands(&self) -> &HashSet<String> { &self.blocked_commands }
        pub fn protected_paths(&self) -> &HashSet<String> { &self.protected_paths }
        pub fn whitelisted_pids(&self) -> &HashSet<u32> { &self.whitelisted_pids }
        pub fn blocked_pids(&self) -> &HashSet<u32> { &self.blocked_pids }

        /// Get full policy snapshot for API responses
        pub fn policy_snapshot(&self) -> PolicySnapshot {
            PolicySnapshot {
                blocked_commands: self.blocked_commands.iter().cloned().collect(),
                protected_paths: self.protected_paths.iter().cloned().collect(),
                whitelisted_pids: self.whitelisted_pids.iter().copied().collect(),
                blocked_pids: self.blocked_pids.iter().copied().collect(),
            }
        }
    }

    /// Snapshot of current policy state
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct PolicySnapshot {
        pub blocked_commands: Vec<String>,
        pub protected_paths: Vec<String>,
        pub whitelisted_pids: Vec<u32>,
        pub blocked_pids: Vec<u32>,
    }

    /// Stats from a hot-reload operation
    #[derive(Debug, Default)]
    pub struct ReloadStats {
        pub commands_added: usize,
        pub commands_removed: usize,
        pub paths_added: usize,
        pub paths_removed: usize,
    }

    /// Helper: convert &str to fixed-size byte array
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

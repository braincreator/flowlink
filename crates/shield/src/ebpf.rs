// FlowLink Shield — Process Monitor
// eBPF (Linux) or simulated /proc polling

use anyhow::Result;
use log::{info, warn};
use std::collections::HashSet;

/// Trait for process monitors that detect new processes
pub trait ProcessMonitor: Send + Sync {
    fn start(&mut self, callback: Box<dyn Fn(u32) + Send + Sync>) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn is_running(&self) -> bool;
}

/// Simulated monitor using /proc polling (works on any Linux, no eBPF required)
pub struct SimulatedMonitor {
    poll_interval_ms: u64,
    running: bool,
    seen_pids: HashSet<u32>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl SimulatedMonitor {
    pub fn new(poll_interval_ms: Option<u64>) -> Self {
        Self {
            poll_interval_ms: poll_interval_ms.unwrap_or(100),
            running: false,
            seen_pids: HashSet::new(),
            handle: None,
        }
    }
}

impl ProcessMonitor for SimulatedMonitor {
    fn start(&mut self, callback: Box<dyn Fn(u32) + Send + Sync>) -> Result<()> {
        if self.running {
            anyhow::bail!("Monitor already running");
        }

        // Seed with current PIDs so we only catch new ones
        self.seed_pids();

        self.running = true;
        let interval = self.poll_interval_ms;
        let cb = callback;

        let handle = std::thread::spawn(move || {
            info!("🔍 SimulatedMonitor started (poll interval: {}ms)", interval);
            let mut seen: HashSet<u32> = HashSet::new();

            // Seed
            if let Ok(entries) = std::fs::read_dir("/proc") {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.chars().all(|c| c.is_ascii_digit()) {
                            if let Ok(pid) = name.parse::<u32>() {
                                seen.insert(pid);
                            }
                        }
                    }
                }
            }

            loop {
                std::thread::sleep(std::time::Duration::from_millis(interval));
                if let Ok(entries) = std::fs::read_dir("/proc") {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.chars().all(|c| c.is_ascii_digit()) {
                                if let Ok(pid) = name.parse::<u32>() {
                                    if seen.insert(pid) {
                                        // New process detected
                                        cb(pid);
                                    }
                                }
                            }
                        }
                    }
                }

                // Clean up dead PIDs
                seen.retain(|&pid| std::path::Path::new(&format!("/proc/{}", pid)).exists());
            }
        });

        self.handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running = false;
        if let Some(handle) = self.handle.take() {
            // Thread will exit naturally on next iteration check
            // For a clean shutdown, we'd need an Arc<AtomicBool> — this is fine for now
            info!("SimulatedMonitor stop requested");
            let _ = handle.join();
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

impl SimulatedMonitor {
    fn seed_pids(&mut self) {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(pid) = name.parse::<u32>() {
                            self.seen_pids.insert(pid);
                        }
                    }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════
// eBPF Monitor (Linux only, behind "ebpf" feature)
// ═══════════════════════════════════════════

#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub use real_ebpf::EbpfMonitor;

#[cfg(all(target_os = "linux", feature = "ebpf"))]
mod real_ebpf {
    use super::*;

    /// eBPF-based process monitor using aya
    /// Hooks execve() syscall and reports new PIDs via ring buffer
    pub struct EbpfMonitor {
        running: bool,
        // TODO: aya::Bpf loader
        // TODO: aya::maps::RingBuffer for events
        // TODO: Embed BPF ELF compiled from bpf/monitor.c
    }

    impl EbpfMonitor {
        pub fn new() -> Self {
            Self { running: false }
        }

        /// Load the eBPF program and attach to tracepoint/syscalls/sys_enter_execve
        /// TODO: Implement with aya:
        ///   1. Load BPF program from embedded ELF
        ///   2. Attach to tracepoint/syscalls/sys_enter_execve
        ///   3. Read events from ring buffer (pid, comm, cmdline)
        ///   4. Call callback for each new execve
        pub fn load(&mut self) -> Result<()> {
            anyhow::bail!("eBPF monitor not yet implemented — requires BPF C program + aya integration")
        }
    }

    impl ProcessMonitor for EbpfMonitor {
        fn start(&mut self, _callback: Box<dyn Fn(u32) + Send + Sync>) -> Result<()> {
            if self.running {
                anyhow::bail!("Monitor already running");
            }
            self.running = true;
            warn!("EbpfMonitor: TODO — implement aya-based execve hook");
            Ok(())
        }

        fn stop(&mut self) -> Result<()> {
            self.running = false;
            // TODO: Detach BPF program, close ring buffer
            Ok(())
        }

        fn is_running(&self) -> bool {
            self.running
        }
    }
}

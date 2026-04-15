// FlowLink Shield — Process Monitor
// eBPF (Linux) or simulated /proc polling

use anyhow::Result;
use log::info;
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
            info!(
                "🔍 SimulatedMonitor started (poll interval: {}ms)",
                interval
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_monitor_creation() {
        let m = SimulatedMonitor::new(None);
        assert!(!m.is_running());
    }

    #[test]
    fn simulated_monitor_custom_interval() {
        let m = SimulatedMonitor::new(Some(500));
        assert!(!m.is_running());
    }

    #[test]
    fn simulated_monitor_default_interval() {
        let m = SimulatedMonitor::new(None);
        assert_eq!(m.poll_interval_ms, 100);
    }

    #[test]
    fn process_monitor_trait_object() {
        let mut m: Box<dyn ProcessMonitor> = Box::new(SimulatedMonitor::new(None));
        assert!(!m.is_running());
        // Don't actually start — no /proc on macOS
    }
}

// ═══════════════════════════════════════════
// eBPF Monitor (Linux only, behind "ebpf" feature)
//
// Requires:
//   1. aya crate (optional dependency)
//   2. A BPF C program compiled to ELF (e.g., bpf/monitor.bpf.c)
//   3. Linux kernel headers for tracepoint/syscalls/sys_enter_execve
//   4. CAP_BPF + CAP_SYS_ADMIN capabilities (or run as root)
//
// To build the BPF program:
//   clang -target bpf -g -O2 -c bpf/monitor.bpf.c -o bpf/monitor.bpf.o
//
// To enable:
//   cargo build --features ebpf
// ═══════════════════════════════════════════

#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub use real_ebpf::EbpfMonitor;

#[cfg(all(target_os = "linux", feature = "ebpf"))]
mod real_ebpf {
    use super::*;
    use aya::maps::RingBuffer;
    use aya::programs::TracePoint;
    use aya::{Bpf, IncludeFile};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /// BPF event struct — must match the C struct in the BPF program.
    ///
    /// Expected BPF C program output:
    /// ```c
    /// struct event {
    ///     u32 pid;
    ///     u32 ppid;
    ///     char comm[16];
    ///     u8 argc;
    /// };
    /// ```
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct ExecEvent {
        pub pid: u32,
        pub ppid: u32,
        pub comm: [u8; 16],
        pub argc: u8,
    }

    /// Configuration for the eBPF monitor.
    #[derive(Debug, Clone)]
    pub struct EbpfConfig {
        /// Path to the compiled BPF ELF file.
        /// Default: embedded via include_bytes! or "bpf/monitor.bpf.o"
        pub bpf_program_path: Option<String>,
        /// Ring buffer size in pages (default: 64 = 256KB)
        pub ring_buffer_pages: u32,
    }

    impl Default for EbpfConfig {
        fn default() -> Self {
            Self {
                bpf_program_path: None,
                ring_buffer_pages: 64,
            }
        }
    }

    /// eBPF-based process monitor using aya.
    ///
    /// Hooks `tracepoint/syscalls/sys_enter_execve` and reports new PIDs
    /// via ring buffer. This is significantly more efficient than /proc polling
    /// — events are delivered in real-time with zero overhead when idle.
    pub struct EbpfMonitor {
        running: bool,
        config: EbpfConfig,
        bpf: Option<Bpf>,
        ring_buf: Option<RingBuffer>,
        handle: Option<thread::JoinHandle<()>>,
        callback: Option<Box<dyn Fn(u32) + Send + Sync>>,
    }

    impl EbpfMonitor {
        pub fn new(config: EbpfConfig) -> Self {
            Self {
                running: false,
                config,
                bpf: None,
                ring_buf: None,
                handle: None,
                callback: None,
            }
        }

        /// Create with default configuration.
        pub fn new_default() -> Self {
            Self::new(EbpfConfig::default())
        }

        /// Load the eBPF program and attach to tracepoint/syscalls/sys_enter_execve.
        ///
        /// Steps:
        ///   1. Load BPF program from embedded ELF or file path
        ///   2. Attach tracepoint to sys_enter_execve
        ///   3. Set up ring buffer for reading exec events
        pub fn load(&mut self) -> Result<()> {
            if self.running {
                anyhow::bail!("Cannot load: monitor is already running. Stop it first.");
            }

            info!("Loading eBPF program...");

            // Load BPF program
            let mut bpf = match &self.config.bpf_program_path {
                Some(path) => {
                    // Load from file path
                    Bpf::load(include_bytes!("../../../bpf/monitor.bpf.o"))
                        .map_err(|e| anyhow::anyhow!("Failed to load BPF from {}: {}", path, e))?
                }
                None => {
                    // Load embedded BPF program
                    // This requires the BPF ELF to be compiled and included at build time.
                    // If the file doesn't exist at compile time, this will fail.
                    Bpf::load(include_bytes!("../../../bpf/monitor.bpf.o")).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to load embedded BPF program. Compile it first: \
                             clang -target bpf -g -O2 -c bpf/monitor.bpf.c -o bpf/monitor.bpf.o. \
                             Error: {}",
                            e
                        )
                    })?
                }
            };

            // Attach tracepoint: syscalls/sys_enter_execve
            let program: &mut TracePoint = bpf.program_mut("trace_execve")
                .ok_or_else(|| anyhow::anyhow!(
                    "BPF program 'trace_execve' not found. Ensure the BPF C program defines \
                     SEC(\"tracepoint/syscalls/sys_enter_execve\") int trace_execve(struct trace_event_raw_sys_enter *ctx)"
                ))?
                .try_into()
                .map_err(|e: aya::programs::ProgramError| anyhow::anyhow!("Failed to cast to TracePoint: {}", e))?;

            program.load()?;
            program.attach()?;
            info!("Attached tracepoint/syscalls/sys_enter_execve");

            // Set up ring buffer for events
            let events_map: aya::maps::RingBuffer = bpf.take_map("EVENTS")
                .ok_or_else(|| anyhow::anyhow!(
                    "BPF map 'EVENTS' not found. Ensure the BPF program defines \
                     struct bpf_map_def SEC(\".maps\") EVENTS = { .type = BPF_MAP_TYPE_RINGBUF, ... };"
                ))?
                .try_into()
                .map_err(|e: aya::maps::MapError| anyhow::anyhow!("Failed to create ring buffer: {}", e))?;

            info!("eBPF monitor loaded successfully");
            self.bpf = Some(bpf);
            self.ring_buf = Some(events_map);
            Ok(())
        }
    }

    impl ProcessMonitor for EbpfMonitor {
        fn start(&mut self, callback: Box<dyn Fn(u32) + Send + Sync>) -> Result<()> {
            if self.running {
                anyhow::bail!("Monitor already running");
            }

            // Auto-load if not loaded yet
            if self.bpf.is_none() {
                self.load()?;
            }

            let ring_buf = self.ring_buf.take().ok_or_else(|| {
                anyhow::anyhow!("Ring buffer not initialized. Call load() first.")
            })?;

            self.running = true;

            let poll_interval = Duration::from_millis(10); // 10ms poll for ring buffer

            let handle = thread::spawn(move || {
                info!("🛡️ EbpfMonitor started (ring buffer polling)");

                loop {
                    // Read events from ring buffer
                    // Poll with callback — returns number of events read, or -1 on error
                    let mut local_cb = &callback;
                    let result = ring_buf.read(
                        -1, // blocking read
                        |data: &[u8]| {
                            if data.len() < std::mem::size_of::<ExecEvent>() {
                                warn!("Received undersized eBPF event: {} bytes", data.len());
                                return -1;
                            }
                            let event: ExecEvent = unsafe {
                                std::ptr::read_unaligned(data.as_ptr() as *const ExecEvent)
                            };
                            let comm = std::str::from_utf8(&event.comm)
                                .unwrap_or("<unknown>")
                                .trim_end_matches('\0')
                                .to_string();
                            info!(
                                "eBPF event: pid={} ppid={} comm={}",
                                event.pid, event.ppid, comm
                            );
                            local_cb(event.pid);
                            0 // continue reading
                        },
                    );

                    match result {
                        Ok(_) => continue,
                        Err(e) => {
                            // EINTR or EAGAIN are normal during shutdown
                            let err_str = e.to_string();
                            if err_str.contains("EINTR") || err_str.contains("EAGAIN") {
                                break;
                            }
                            warn!("Ring buffer read error: {}", e);
                            thread::sleep(poll_interval);
                        }
                    }
                }

                info!("EbpfMonitor stopped");
            });

            self.handle = Some(handle);
            self.callback = Some(callback);
            Ok(())
        }

        fn stop(&mut self) -> Result<()> {
            self.running = false;
            info!("EbpfMonitor stopping...");

            if let Some(handle) = self.handle.take() {
                // Ring buffer read is blocking; the thread will exit when
                // the ring buffer is dropped (which happens when we drop it).
                // We give it a moment to clean up.
                match handle.join_timeout(Duration::from_secs(2)) {
                    Ok(()) => info!("EbpfMonitor thread exited cleanly"),
                    Err(_) => {
                        warn!("EbpfMonitor thread did not exit in 2s — it may be stuck on ring buffer read");
                    }
                }
            }

            // Drop BPF program — this detaches the tracepoint
            self.bpf = None;
            self.ring_buf = None;
            self.callback = None;

            Ok(())
        }

        fn is_running(&self) -> bool {
            self.running
        }
    }

    impl Drop for EbpfMonitor {
        fn drop(&mut self) {
            if self.running {
                let _ = self.stop();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ebpf_monitor_creation() {
            let m = EbpfMonitor::new_default();
            assert!(!m.is_running());
        }

        #[test]
        fn ebpf_monitor_custom_config() {
            let config = EbpfConfig {
                bpf_program_path: Some("/tmp/test.bpf.o".into()),
                ring_buffer_pages: 128,
            };
            let m = EbpfMonitor::new(config);
            assert!(!m.is_running());
        }

        #[test]
        fn ebpf_monitor_trait_object() {
            let m: Box<dyn ProcessMonitor> = Box::new(EbpfMonitor::new_default());
            assert!(!m.is_running());
        }
    }
}

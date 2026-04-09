// FlowLink Shield — eBPF Kernel Monitor
// Loads the BPF program, populates maps, consumes ring buffer events.

use anyhow::Result;
use tokio::sync::mpsc;

/// Event received from the eBPF ring buffer
#[derive(Debug, Clone)]
pub struct KernelEvent {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub comm: String,
    pub args: String,
    pub signal_sent: bool,
}

/// Dangerous pattern to load into the BPF map
#[derive(Debug, Clone)]
pub struct DangerousPattern {
    pub binary: String,
    pub check_args: bool,
    pub check_paths: bool,
}

/// Default dangerous patterns for L1 kernel matching
pub fn default_patterns() -> Vec<DangerousPattern> {
    vec![
        DangerousPattern { binary: "rm".into(), check_args: true, check_paths: true },
        DangerousPattern { binary: "shred".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "mkfs.".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "dd".into(), check_args: false, check_paths: true },
        DangerousPattern { binary: "shutdown".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "poweroff".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "halt".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "reboot".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "iptables".into(), check_args: true, check_paths: false },
        DangerousPattern { binary: "nft".into(), check_args: true, check_paths: false },
        DangerousPattern { binary: "docker".into(), check_args: true, check_paths: false },
        DangerousPattern { binary: "systemctl".into(), check_args: true, check_paths: false },
        DangerousPattern { binary: "killall".into(), check_args: false, check_paths: false },
        DangerousPattern { binary: "pkill".into(), check_args: false, check_paths: false },
    ]
}

// ═══════════════════════════════════════════
// Linux eBPF implementation (behind feature gate)
// ═══════════════════════════════════════════

#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub use real_kernel::*;

#[cfg(all(target_os = "linux", feature = "ebpf"))]
mod real_kernel {
    use super::*;
    use aya::{Ebpf, programs::TracePoint, maps::RingBuffer};
    use aya::maps::Map;

    /// eBPF kernel monitor — loads BPF program, manages maps + ring buffer
    pub struct EbpfKernelMonitor {
        _ebpf: Ebpf,
        _task: Option<tokio::task::JoinHandle<()>>,
    }

    impl EbpfKernelMonitor {
        /// Load the eBPF program and start consuming events.
        ///
        /// Returns (monitor, event_receiver) where events from the kernel
        /// ring buffer are delivered via the channel.
        pub async fn load(
            patterns: Vec<DangerousPattern>,
            allowed_uids: Vec<u32>,
        ) -> Result<(Self, mpsc::Receiver<KernelEvent>)> {
            let mut ebpf = Ebpf::load(include_bytes!("bpf/shield.bpf.o"))
                .context("Failed to load eBPF program")?;

            // Attach tracepoint
            let program: &mut TracePoint = ebpf.program_mut("on_execve")
                .context("BPF program 'on_execve' not found")?
                .try_into()
                .context("Not a tracepoint program")?;
            program.load()?;
            program.attach()?;
            info!("🛡 eBPF tracepoint attached: sys_enter_execve");

            // Populate allowed_uids map
            let allowed_map: aya::maps::HashMap<_, u32, u32> = ebpf.map_mut("allowed_uids")
                .context("allowed_uids map not found")?
                .try_into()?;
            for uid in &allowed_uids {
                let val: u32 = 1;
                allowed_map.insert(uid, val, 0)?;
            }
            info!("🛡 Allowed UIDs loaded: {:?}", allowed_uids);

            // Populate patterns map
            let patterns_map: aya::maps::Array<_, DangerousPatternBpf> = ebpf.map_mut("patterns")
                .context("patterns map not found")?
                .try_into()?;
            for (i, pat) in patterns.iter().enumerate().take(32) {
                let mut binary = [0u8; 32];
                let bytes = pat.binary.as_bytes();
                let len = bytes.len().min(32);
                binary[..len].copy_from_slice(&bytes[..len]);
                let bpf_pat = DangerousPatternBpf {
                    binary,
                    check_args: if pat.check_args { 1 } else { 0 },
                    check_paths: if pat.check_paths { 1 } else { 0 },
                };
                patterns_map.set(i as u32, bpf_pat, 0)?;
            }
            info!("🛡 {} dangerous patterns loaded", patterns.len().min(32));

            // Ring buffer consumer
            let events_map: RingBuffer = ebpf.map_mut("events")
                .context("events ringbuf not found")?
                .try_into()?;

            let (tx, rx) = mpsc::channel(256);

            // The ringbuf callback must be 'static + Send, so we move tx in
            let task = tokio::task::spawn_blocking(move || {
                let tx = tx;
                let _ = || -> Result<()> {
                    events_map.read(|data| {
                        if data.len() < std::mem::size_of::<KernelEventRaw>() {
                            return 0;
                        }
                        // Safe: we control the layout from BPF side
                        let raw: &KernelEventRaw = unsafe {
                            &*(data.as_ptr() as *const KernelEventRaw)
                        };
                        let event = KernelEvent {
                            pid: raw.pid,
                            ppid: raw.ppid,
                            uid: raw.uid,
                            comm: std::ffi::CStr::from_bytes_until_nul(&raw.comm)
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            args: std::ffi::CStr::from_bytes_until_nul(&raw.args)
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            signal_sent: raw.signal_sent != 0,
                        };
                        let _ = tx.blocking_send(event);
                        data.len()
                    })?;
                    Ok(())
                }();
            });

            Ok((Self {
                _ebpf: ebpf,
                _task: Some(task),
            }, rx))
        }
    }

    /// Raw event layout matching the BPF C struct (must be #[repr(C)])
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    struct KernelEventRaw {
        pid: u32,
        ppid: u32,
        uid: u32,
        comm: [u8; 64],
        args: [u8; 256],
        signal_sent: i32,
    }

    /// BPF-side pattern struct
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    struct DangerousPatternBpf {
        binary: [u8; 32],
        check_args: u8,
        check_paths: u8,
    }
}

// ═══════════════════════════════════════════
// Stub for non-Linux / non-ebpf builds
// ═══════════════════════════════════════════

#[cfg(not(all(target_os = "linux", feature = "ebpf")))]
#[allow(unused_imports)]
pub use stub::*;

#[cfg(not(all(target_os = "linux", feature = "ebpf")))]
mod stub {
    use super::*;

    /// Stub kernel monitor — no-op on non-Linux platforms
    #[allow(dead_code)]
    pub struct EbpfKernelMonitor;

    impl EbpfKernelMonitor {
        #[allow(dead_code)]
        pub async fn load(
            _patterns: Vec<DangerousPattern>,
            _allowed_uids: Vec<u32>,
        ) -> Result<(Self, mpsc::Receiver<KernelEvent>)> {
            log::warn!("eBPF kernel monitor not available (not Linux or ebpf feature disabled)");
            let (_tx, rx) = mpsc::channel(1);
            Ok((Self, rx))
        }
    }
}

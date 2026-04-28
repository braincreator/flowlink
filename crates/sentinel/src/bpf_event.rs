//! BPF event types matching kernel-space struct layout
//!
//! Used by the Linux eBPF backend. The layout MUST match `bpf/sentinel.bpf.c`.

use crate::event::{EventKind, KernelEvent};

/// BPF event struct — MUST match sentinel.bpf.c exactly
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BpfEvent {
    pub event_type: u32,
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub gid: u32,
    pub comm: [u8; 64],
    pub args: [[u8; 64]; 4],
    pub args_count: u32,
    pub path: [u8; 128],
    pub port: u16,
    pub flags: u32,
}

/// Parse raw BPF event bytes into a `KernelEvent`.
pub fn parse_bpf_event(data: &[u8]) -> anyhow::Result<KernelEvent> {
    if data.len() < std::mem::size_of::<BpfEvent>() {
        return Err(anyhow::anyhow!(
            "BPF event too short: {} < {}",
            data.len(),
            std::mem::size_of::<BpfEvent>()
        ));
    }

    let event = unsafe { &*(data.as_ptr() as *const BpfEvent) };

    let kind = match event.event_type {
        0 => EventKind::Exec,
        1 => EventKind::FileWrite,
        2 => EventKind::NetworkConnect,
        3 => EventKind::NetworkBind,
        4 => EventKind::FileDelete,
        5 => EventKind::Mount,
        _ => return Err(anyhow::anyhow!("unknown event type: {}", event.event_type)),
    };

    let command = null_terminated_str(&event.comm);
    let args: Vec<String> = (0..event.args_count as usize)
        .filter_map(|i| {
            let s = null_terminated_str(&event.args[i]);
            if s.is_empty() { None } else { Some(s) }
        })
        .collect();
    let path = null_terminated_str(&event.path);

    let remote_addr = if matches!(kind, EventKind::NetworkConnect | EventKind::NetworkBind) && !path.is_empty() {
        Some(path.clone())
    } else {
        None
    };

    Ok(KernelEvent {
        kind,
        pid: event.pid,
        ppid: event.ppid,
        uid: event.uid,
        command: if command.is_empty() { None } else { Some(command) },
        args,
        path: if path.is_empty() { None } else { Some(path) },
        remote_addr,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    })
}

fn null_terminated_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}


mod tests {
    #![allow(dead_code)]
    use super::*;

    fn make_bpf_bytes(event_type: u32, pid: u32) -> Vec<u8> {
        let mut buf = vec![0u8; std::mem::size_of::<BpfEvent>()];
        // event_type at 0
        buf[0..4].copy_from_slice(&event_type.to_ne_bytes());
        // pid at 4
        buf[4..8].copy_from_slice(&pid.to_ne_bytes());
        // ppid at 8
        buf[8..12].copy_from_slice(&100u32.to_ne_bytes());
        // uid at 12
        buf[12..16].copy_from_slice(&0u32.to_ne_bytes());
        // gid at 16
        buf[16..20].copy_from_slice(&0u32.to_ne_bytes());
        // comm at 20 (64 bytes)
        let cmd = b"/usr/bin/test_cmd\0";
        buf[20..20+cmd.len()].copy_from_slice(cmd);
        // args[0] at 84 (64 bytes)
        let arg1 = b"--arg1\0";
        buf[84..84+arg1.len()].copy_from_slice(arg1);
        // args_count at 340
        buf[340..344].copy_from_slice(&1u32.to_ne_bytes());
        // path at 344 (128 bytes)
        let path = b"/var/log/test\0";
        buf[344..344+path.len()].copy_from_slice(path);
        // port at 472
        buf[472..474].copy_from_slice(&8080u16.to_ne_bytes());
        // flags at 474
        buf[474..478].copy_from_slice(&1u32.to_ne_bytes());
        buf
    }

    fn set_str(buf: &mut [u8], offset: usize, s: &str) {
        let bytes = format!("{}\0", s).into_bytes();
        buf[offset..offset+bytes.len()].copy_from_slice(&bytes);
    }

    // ── All event types ──
    #[test] fn parse_exec() {
        let e = parse_bpf_event(&make_bpf_bytes(0, 1234)).unwrap();
        assert_eq!(e.kind, EventKind::Exec);
        assert_eq!(e.pid, 1234);
    }
    #[test] fn parse_file_write() {
        let e = parse_bpf_event(&make_bpf_bytes(1, 5678)).unwrap();
        assert_eq!(e.kind, EventKind::FileWrite);
        assert_eq!(e.pid, 5678);
    }
    #[test] fn parse_connect() {
        let e = parse_bpf_event(&make_bpf_bytes(2, 9999)).unwrap();
        assert_eq!(e.kind, EventKind::NetworkConnect);
        assert!(e.remote_addr.is_some()); // path used as addr for connect
    }
    #[test] fn parse_bind() {
        let e = parse_bpf_event(&make_bpf_bytes(3, 1111)).unwrap();
        assert_eq!(e.kind, EventKind::NetworkBind);
        assert!(e.remote_addr.is_some());
    }
    #[test] fn parse_delete() {
        let e = parse_bpf_event(&make_bpf_bytes(4, 2222)).unwrap();
        assert_eq!(e.kind, EventKind::FileDelete);
    }
    #[test] fn parse_mount() {
        let e = parse_bpf_event(&make_bpf_bytes(5, 3333)).unwrap();
        assert_eq!(e.kind, EventKind::Mount);
    }
    #[test] fn parse_unknown_error() {
        assert!(parse_bpf_event(&make_bpf_bytes(99, 100)).is_err());
    }

    // ── Error cases ──
    #[test] fn parse_empty_error() { assert!(parse_bpf_event(&[]).is_err()); }
    #[test] fn parse_too_short() { assert!(parse_bpf_event(&[0u8; 10]).is_err()); }
    #[test] fn parse_single_byte() { assert!(parse_bpf_event(&[0u8; 1]).is_err()); }
    #[test] fn parse_almost_enough() {
        let size = std::mem::size_of::<BpfEvent>();
        assert!(parse_bpf_event(&vec![0u8; size - 1]).is_err());
    }

    // ── Field extraction ──
    #[test] fn parse_pid() { assert_eq!(parse_bpf_event(&make_bpf_bytes(0, 12345)).unwrap().pid, 12345); }
    #[test] fn parse_ppid() { assert_eq!(parse_bpf_event(&make_bpf_bytes(0, 1)).unwrap().ppid, 100); }
    #[test] fn parse_uid() { assert_eq!(parse_bpf_event(&make_bpf_bytes(0, 1)).unwrap().uid, 0); }
    #[test] fn parse_command() {
        let e = parse_bpf_event(&make_bpf_bytes(0, 1)).unwrap();
        assert_eq!(e.command.as_deref(), Some("/usr/bin/test_cmd"));
    }
    #[test] fn parse_command_empty() {
        let mut buf = make_bpf_bytes(0, 1);
        for i in 20..84 { buf[i] = 0; }
        let e = parse_bpf_event(&buf).unwrap();
        assert!(e.command.is_none());
    }
    #[test] fn parse_args() {
        let e = parse_bpf_event(&make_bpf_bytes(0, 1)).unwrap();
        assert!(e.args.len() >= 1);
        assert_eq!(e.args[0], "--arg1");
    }
    #[test] fn parse_args_zero_count() {
        let mut buf = make_bpf_bytes(0, 1);
        buf[340..344].copy_from_slice(&0u32.to_ne_bytes());
        let e = parse_bpf_event(&buf).unwrap();
        assert!(e.args.is_empty());
    }
    #[test] fn parse_path() {
        let e = parse_bpf_event(&make_bpf_bytes(1, 1)).unwrap();
        assert_eq!(e.path.as_deref(), Some("/var/log/test"));
    }
    #[test] fn parse_path_empty() {
        let mut buf = make_bpf_bytes(1, 1);
        for i in 344..472 { buf[i] = 0; }
        let e = parse_bpf_event(&buf).unwrap();
        assert!(e.path.is_none());
    }

    // ── Struct size ──
    #[test] fn bpf_event_size() {
        let size = std::mem::size_of::<BpfEvent>();
        assert!(size < 512, "BpfEvent too large: {} bytes", size);
        // Expected: 4+4+4+4+4+64+256+4+128+2+4 = 478 (may vary with alignment)
    }

    // ── Multiple events ──
    #[test] fn parse_sequence() {
        for (etype, expected) in [(0,EventKind::Exec),(1,EventKind::FileWrite),(2,EventKind::NetworkConnect)] {
            let e = parse_bpf_event(&make_bpf_bytes(etype, 1)).unwrap();
            assert_eq!(e.kind, expected);
        }
    }

    // ── Connect with path as remote_addr ──
    #[test] fn parse_connect_remote_addr() {
        let mut buf = make_bpf_bytes(2, 1);
        set_str(&mut buf, 344, "10.0.0.1:443");
        let e = parse_bpf_event(&buf).unwrap();
        assert_eq!(e.kind, EventKind::NetworkConnect);
        assert!(e.remote_addr.is_some());
    }
}

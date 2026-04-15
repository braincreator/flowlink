// FlowLink Shield — macOS-specific forensic collectors using ps / sysctl
#![allow(dead_code)]

use anyhow::Result;
use std::process::Command;

use crate::forensic::{
    ContainerInfo, PlatformProcessInfo, ProcessOrigin, ProcessTreeNode, SshInfo,
};

pub fn collect_process_info(pid: u32) -> Result<PlatformProcessInfo> {
    // Use ps for process info on macOS
    let output = Command::new("ps")
        .args(["-o", "pid,ppid,uid,gid,lstart,comm", "-p", &pid.to_string()])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() < 2 {
        anyhow::bail!("ps returned no data for pid {}", pid);
    }

    let fields: Vec<&str> = lines[1].split_whitespace().collect();
    if fields.len() < 5 {
        anyhow::bail!("unexpected ps output format");
    }

    let ppid: u32 = fields.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let uid: u32 = fields.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let gid: u32 = fields.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);

    // Get executable path via proc_info or /proc (if mounted)
    let exe = get_executable_path(pid).unwrap_or_else(|_| fields[fields.len() - 1].to_string());
    let comm = fields[fields.len() - 1].to_string();

    // Session leader and tty via ps
    let tty = get_tty(pid);
    let session_leader = get_session_leader(pid).unwrap_or(pid);

    Ok(PlatformProcessInfo {
        pid,
        ppid,
        uid,
        gid,
        exe,
        comm,
        session_leader,
        controlling_terminal: tty,
    })
}

pub fn walk_process_tree(pid: u32, max_depth: usize) -> Vec<ProcessTreeNode> {
    let mut tree = Vec::new();
    let mut current_pid = pid;
    let mut seen = std::collections::HashSet::new();

    for _ in 0..max_depth {
        if seen.contains(&current_pid) || current_pid == 0 || current_pid == 1 {
            break;
        }
        seen.insert(current_pid);

        let info = match collect_process_info(current_pid) {
            Ok(i) => i,
            Err(_) => break,
        };

        tree.push(ProcessTreeNode {
            pid: current_pid,
            name: info.comm.clone(),
            exe: info.exe.clone(),
        });

        if info.ppid == 0 || info.ppid == current_pid {
            break;
        }
        current_pid = info.ppid;
    }

    tree
}

pub fn detect_origin(pid: u32, tree: &[ProcessTreeNode]) -> ProcessOrigin {
    // Check for Docker Desktop or Lima VM containers
    for node in tree.iter().rev() {
        let name = node.name.to_lowercase();
        match name.as_str() {
            "sshd-keygen-wrapper" | "sshd" => {
                return ProcessOrigin::Ssh {
                    remote_addr: "unknown".into(),
                    remote_port: 0,
                };
            }
            "com.docker.vpnkit" | "docker" | "com.docker.backend" => {
                return ProcessOrigin::Container {
                    id: "unknown".into(),
                    name: "docker-desktop".into(),
                    image: None,
                };
            }
            "launchd" => {
                return ProcessOrigin::Systemd {
                    unit: "launchd".into(),
                };
            }
            _ => {}
        }
    }

    // Check environment for agent
    if let Ok(output) = Command::new("ps")
        .args(["-E", "-p", &pid.to_string()])
        .output()
    {
        let env_str = String::from_utf8_lossy(&output.stdout);
        for pair in env_str.split_whitespace() {
            if pair.starts_with("FLOWLINK_AGENT_ID=") {
                let id = pair.trim_start_matches("FLOWLINK_AGENT_ID=").to_string();
                return ProcessOrigin::Agent { agent_id: id };
            }
            if pair.starts_with("SSH_CONNECTION=") {
                let val = pair.trim_start_matches("SSH_CONNECTION=");
                let parts: Vec<&str> = val.split_whitespace().collect();
                if parts.len() >= 2 {
                    return ProcessOrigin::Ssh {
                        remote_addr: parts[0].to_string(),
                        remote_port: parts[1].parse().unwrap_or(0),
                    };
                }
            }
        }
    }

    ProcessOrigin::Unknown
}

pub fn collect_ssh_info(_pid: u32) -> Option<SshInfo> {
    // macOS: check SSH_CONNECTION env via ps -E
    None // simplified; would need environ access
}

pub fn collect_container_info(_pid: u32) -> Option<ContainerInfo> {
    // macOS: Docker Desktop containers run in a VM, limited visibility
    None
}

pub fn get_cwd(pid: u32) -> Result<String> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("n/") {
            return Ok(line[1..].to_string());
        }
    }
    anyhow::bail!("could not determine cwd for pid {}", pid)
}

pub fn get_boot_offset_ms() -> Option<u64> {
    let output = Command::new("sysctl")
        .args(["-n", "kern.boottime"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Format: { sec = 1712400000, usec = 123456 } ...
    let sec: u64 = stdout
        .split("sec = ")
        .nth(1)?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()?;
    let now = chrono::Utc::now().timestamp() as u64;
    Some((now.saturating_sub(sec)) * 1000)
}

pub fn get_session_duration_ms(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let start_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let start = chrono::NaiveDateTime::parse_from_str(&start_str, "%a %b %e %H:%M:%S %Y").ok()?;
    let now = chrono::Utc::now().naive_utc();
    Some((now - start).num_milliseconds().max(0) as u64)
}

fn get_executable_path(pid: u32) -> Result<String> {
    // Try /proc first (may be mounted), fall back to ps
    let path = std::path::PathBuf::from(format!("/proc/{pid}/exe"));
    if path.exists() {
        return Ok(std::fs::read_link(path)?.display().to_string());
    }
    let output = Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_tty(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tty.is_empty() || tty == "??" {
        None
    } else {
        Some(tty)
    }
}

fn get_session_leader(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "sid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_own_process_info() {
        let pid = std::process::id();
        let result = collect_process_info(pid);
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.pid, pid);
    }

    #[test]
    fn test_walk_own_process_tree() {
        let tree = walk_process_tree(std::process::id(), 10);
        assert!(!tree.is_empty());
        // Should contain at least the current process
        assert_eq!(tree.first().unwrap().pid, std::process::id());
    }

    #[test]
    fn test_detect_origin_own() {
        let tree = walk_process_tree(std::process::id(), 10);
        let origin = detect_origin(std::process::id(), &tree);
        // Just verify it doesn't panic
        let _ = format!("{:?}", origin);
    }

    #[test]
    fn test_get_cwd_own() {
        let result = get_cwd(std::process::id());
        assert!(result.is_ok());
    }

    #[test]
    fn test_boot_offset() {
        let offset = get_boot_offset_ms();
        assert!(offset.is_some());
        // Should be > 0 on any running system
        assert!(offset.unwrap() > 0);
    }
}

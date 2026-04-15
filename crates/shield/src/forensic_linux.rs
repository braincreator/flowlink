// FlowLink Shield — Linux-specific forensic collectors using /proc

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::forensic::{
    ContainerInfo, PlatformProcessInfo, ProcessOrigin, ProcessTreeNode, SshInfo,
};

pub fn collect_process_info(pid: u32) -> Result<PlatformProcessInfo> {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));

    let stat = fs::read_to_string(proc_dir.join("stat")).unwrap_or_default();
    let status = fs::read_to_string(proc_dir.join("status")).unwrap_or_default();
    let comm = fs::read_to_string(proc_dir.join("comm"))
        .unwrap_or_default()
        .trim()
        .to_string();
    let exe = fs::read_link(proc_dir.join("exe"))
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let ppid = parse_ppid_from_stat(&stat);
    let uid = parse_field_from_status(&status, "Uid");
    let gid = parse_field_from_status(&status, "Gid");
    let session_leader = parse_session_from_stat(&stat);
    let controlling_terminal = parse_tty_from_stat(&stat);

    Ok(PlatformProcessInfo {
        pid,
        ppid,
        uid,
        gid,
        exe,
        comm,
        session_leader,
        controlling_terminal,
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

        let proc_dir = PathBuf::from(format!("/proc/{current_pid}"));
        let comm = fs::read_to_string(proc_dir.join("comm"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let exe = fs::read_link(proc_dir.join("exe"))
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let stat = fs::read_to_string(proc_dir.join("stat")).unwrap_or_default();
        let ppid = parse_ppid_from_stat(&stat);

        tree.push(ProcessTreeNode {
            pid: current_pid,
            name: comm,
            exe,
        });

        if ppid == 0 || ppid == current_pid {
            break;
        }
        current_pid = ppid;
    }

    tree
}

pub fn detect_origin(pid: u32, tree: &[ProcessTreeNode]) -> ProcessOrigin {
    // Check for container cgroup
    if let Ok(cgroup) = fs::read_to_string(format!("/proc/{pid}/cgroup")) {
        if let Some(info) = parse_cgroup_container(&cgroup) {
            return info;
        }
    }

    // Walk tree for known origins
    for node in tree.iter().rev() {
        let name = node.name.to_lowercase();
        match name.as_str() {
            "sshd" | "ssh" => {
                return ProcessOrigin::Ssh {
                    remote_addr: "unknown".into(),
                    remote_port: 0,
                };
            }
            "cron" | "crond" => {
                return ProcessOrigin::Cron { schedule: None };
            }
            "containerd" | "dockerd" | "podman" | "conmon" => {
                return ProcessOrigin::Container {
                    id: "unknown".into(),
                    name: "unknown".into(),
                    image: None,
                };
            }
            "systemd" | "init" => {
                return ProcessOrigin::Systemd {
                    unit: "unknown".into(),
                };
            }
            _ => {}
        }
    }

    // Check if FlowLink agent
    if let Ok(environ) = fs::read(format!("/proc/{pid}/environ")) {
        let env_str = String::from_utf8_lossy(&environ);
        for pair in env_str.split('\0') {
            if pair.starts_with("FLOWLINK_AGENT_ID=") {
                let id = pair.trim_start_matches("FLOWLINK_AGENT_ID=").to_string();
                return ProcessOrigin::Agent { agent_id: id };
            }
        }
    }

    ProcessOrigin::Unknown
}

pub fn collect_ssh_info(pid: u32) -> Option<SshInfo> {
    // Read SSH_CONNECTION from environ
    let Ok(environ) = fs::read(format!("/proc/{pid}/environ")) else {
        return None;
    };
    let env_str = String::from_utf8_lossy(&environ);
    for pair in env_str.split('\0') {
        if pair.starts_with("SSH_CONNECTION=") {
            let val = pair.trim_start_matches("SSH_CONNECTION=");
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() >= 4 {
                return Some(SshInfo {
                    remote_addr: parts[0].to_string(),
                    remote_port: parts[1].parse().unwrap_or(0),
                    local_port: parts[3].parse().unwrap_or(0),
                    session_id: None,
                });
            }
        }
    }
    None
}

pub fn collect_container_info(pid: u32) -> Option<ContainerInfo> {
    let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    parse_cgroup_container(&cgroup).and_then(|origin| match origin {
        ProcessOrigin::Container { id, name, image } => Some(ContainerInfo {
            id,
            name,
            image,
            runtime: detect_runtime(&cgroup),
        }),
        _ => None,
    })
}

pub fn get_cwd(pid: u32) -> Result<String> {
    let cwd = fs::read_link(format!("/proc/{pid}/cwd"))?;
    Ok(cwd.display().to_string())
}

pub fn get_boot_offset_ms() -> Option<u64> {
    let btime = fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find(|l| l.starts_with("btime"))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?;
    let now = chrono::Utc::now().timestamp() as u64;
    Some((now - btime) * 1000)
}

pub fn get_session_duration_ms(pid: u32) -> Option<u64> {
    // Use process start time from /proc/{pid}/stat
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let ticks = parse_process_starttime_ticks(&stat)?;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
    if hz == 0 {
        return None;
    }

    let btime = fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find(|l| l.starts_with("btime"))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?;

    let start_ms = (btime + ticks / hz) * 1000;
    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    Some(now_ms.saturating_sub(start_ms))
}

// Helpers

fn parse_ppid_from_stat(stat: &str) -> u32 {
    if let Some(close) = stat.rfind(')') {
        let rest = &stat[close + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() >= 2 {
            return fields[1].parse().unwrap_or(0);
        }
    }
    0
}

fn parse_field_from_status(status: &str, field: &str) -> u32 {
    for line in status.lines() {
        if line.starts_with(&format!("{field}:")) {
            return line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn parse_session_from_stat(stat: &str) -> u32 {
    // Field 6 (0-indexed after comm): session
    if let Some(close) = stat.rfind(')') {
        let rest = &stat[close + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() >= 6 {
            return fields[5].parse().unwrap_or(0);
        }
    }
    0
}

fn parse_tty_from_stat(stat: &str) -> Option<String> {
    // TTY number is field 7 (0-indexed after comm)
    if let Some(close) = stat.rfind(')') {
        let rest = &stat[close + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() >= 7 {
            let tty_nr: i32 = fields[6].parse().unwrap_or(0);
            if tty_nr > 0 {
                let major = (tty_nr >> 8) & 0xff;
                let minor = tty_nr & 0xff;
                if major == 136 {
                    return Some(format!("pts/{}", minor));
                }
                return Some(format!("tty{}", minor));
            }
        }
    }
    None
}

fn parse_process_starttime_ticks(stat: &str) -> Option<u64> {
    if let Some(close) = stat.rfind(')') {
        let rest = &stat[close + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // Field 22 (1-indexed) = starttime
        if fields.len() >= 22 {
            return fields[21].parse().ok();
        }
    }
    None
}

fn parse_cgroup_container(cgroup: &str) -> Option<ProcessOrigin> {
    // Docker: .../docker/<container_id>
    // kubernetes: .../kubepods/.../<container_id>
    // podman: .../libpod_parent/libpod-<container_id>
    for line in cgroup.lines() {
        if line.contains("/docker/") {
            let id = extract_hex_id(line, "/docker/")?;
            return Some(ProcessOrigin::Container {
                id: id.clone(),
                name: format!("docker-{}", &id[..id.len().min(12)]),
                image: None,
            });
        }
        if line.contains("/kubepods/") {
            let id = extract_hex_id(line, "/kubepods/")?;
            return Some(ProcessOrigin::Container {
                id: id.clone(),
                name: format!("k8s-{}", &id[..id.len().min(12)]),
                image: None,
            });
        }
        if line.contains("/libpod-") {
            let id = extract_hex_id(line, "/libpod-")?;
            return Some(ProcessOrigin::Container {
                id: id.clone(),
                name: format!("podman-{}", &id[..id.len().min(12)]),
                image: None,
            });
        }
    }
    None
}

fn extract_hex_id(line: &str, marker: &str) -> Option<String> {
    let pos = line.find(marker)?;
    let rest = &line[pos + marker.len()..];
    let id: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if id.len() >= 12 {
        Some(id)
    } else {
        None
    }
}

fn detect_runtime(cgroup: &str) -> String {
    if cgroup.contains("/docker/") {
        "docker".into()
    } else if cgroup.contains("/kubepods/") {
        "containerd".into()
    } else if cgroup.contains("/libpod") {
        "podman".into()
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ppid_from_stat() {
        let stat = "1234 (bash) S 5678 1234 5678 0 -1 4194304 1234 0 0 0 0";
        assert_eq!(parse_ppid_from_stat(stat), 5678);
    }

    #[test]
    fn test_parse_field_from_status() {
        let status = "Uid:\t1000\t1000\t1000\t1000\nGid:\t100\t100\t100\t100";
        assert_eq!(parse_field_from_status(status, "Uid"), 1000);
        assert_eq!(parse_field_from_status(status, "Gid"), 100);
    }

    #[test]
    fn test_parse_session_from_stat() {
        let stat = "1234 (bash) S 5678 1234 5678 1234 9999 0 -1";
        assert_eq!(parse_session_from_stat(stat), 9999);
    }

    #[test]
    fn test_parse_tty_from_stat() {
        // pts/0 → major 136, minor 0 → tty_nr = 34816
        let stat = "1234 (bash) S 1 1234 1234 1234 0 34816 -1";
        assert_eq!(parse_tty_from_stat(stat), Some("pts/0".into()));
    }

    #[test]
    fn test_parse_tty_none() {
        let stat = "1234 (bash) S 1 1234 1234 1234 0 0 -1";
        assert_eq!(parse_tty_from_stat(stat), None);
    }

    #[test]
    fn test_parse_process_starttime_ticks() {
        // 22 fields after comm
        let stat = "1234 (bash) S 1 1234 1234 1234 0 0 -1 0 0 0 0 0 0 0 0 0 0 0 0 99999 0";
        assert_eq!(parse_process_starttime_ticks(stat), Some(99999));
    }

    #[test]
    fn test_parse_cgroup_docker() {
        let cgroup = "12:pids:/docker/abc123def456789\n1:name=systemd:/docker/abc123def456789";
        let origin = parse_cgroup_container(cgroup);
        assert!(matches!(origin, Some(ProcessOrigin::Container { .. })));
        if let Some(ProcessOrigin::Container { ref id, .. }) = origin {
            assert!(id.starts_with("abc123def"));
        }
    }

    #[test]
    fn test_parse_cgroup_kubernetes() {
        let cgroup = "0::/kubepods/besteffort/pod1234/abc123def456";
        let origin = parse_cgroup_container(cgroup);
        assert!(matches!(origin, Some(ProcessOrigin::Container { .. })));
    }

    #[test]
    fn test_parse_cgroup_podman() {
        let cgroup = "0::/libpod_parent/libpod-abc123def456";
        let origin = parse_cgroup_container(cgroup);
        assert!(matches!(origin, Some(ProcessOrigin::Container { .. })));
    }

    #[test]
    fn test_parse_cgroup_none() {
        let cgroup = "0::/user.slice";
        assert!(parse_cgroup_container(cgroup).is_none());
    }

    #[test]
    fn test_detect_runtime() {
        assert_eq!(detect_runtime("12:pids:/docker/abc"), "docker");
        assert_eq!(detect_runtime("0::/kubepods/pod1/abc"), "containerd");
        assert_eq!(detect_runtime("0::/libpod_parent/libpod-abc"), "podman");
        assert_eq!(detect_runtime("0::/user.slice"), "unknown");
    }

    #[test]
    fn test_extract_hex_id() {
        assert_eq!(
            extract_hex_id("some/path/docker/abc123def456789abc/rest", "/docker/"),
            Some("abc123def456789abc".into())
        );
        assert_eq!(extract_hex_id("no marker here", "/docker/"), None);
    }

    #[test]
    fn test_origin_detection_ssh() {
        let tree = vec![
            ProcessTreeNode {
                pid: 1,
                name: "sshd".into(),
                exe: "/usr/sbin/sshd".into(),
            },
            ProcessTreeNode {
                pid: 100,
                name: "bash".into(),
                exe: "/usr/bin/bash".into(),
            },
        ];
        let origin = detect_origin(100, &tree);
        assert!(matches!(origin, ProcessOrigin::Ssh { .. }));
    }

    #[test]
    fn test_origin_detection_cron() {
        let tree = vec![ProcessTreeNode {
            pid: 1,
            name: "cron".into(),
            exe: "/usr/sbin/cron".into(),
        }];
        let origin = detect_origin(1, &tree);
        assert!(matches!(origin, ProcessOrigin::Cron { .. }));
    }
}

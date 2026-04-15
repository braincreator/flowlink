// FlowLink Shield — Forensic metadata collection
// WHO executed it, WHERE it came from, WHEN it happened
#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
use crate::forensic_linux;
#[cfg(target_os = "macos")]
use crate::forensic_macos;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicContext {
    // WHO
    pub uid: u32,
    pub gid: u32,
    pub username: String,
    pub groups: Vec<String>,
    pub is_root: bool,
    pub is_service: bool,

    // PROCESS TREE
    pub pid: u32,
    pub ppid: u32,
    pub process_tree: Vec<ProcessTreeNode>,
    pub session_leader_pid: u32,
    pub controlling_terminal: Option<String>,

    // ORIGIN
    pub origin: ProcessOrigin,
    pub ssh_connection: Option<SshInfo>,
    pub container_info: Option<ContainerInfo>,
    pub agent_id: Option<String>,

    // WHAT
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub shell: Option<String>,
    pub executable_path: String,
    pub executable_hash: Option<String>,

    // WHEN
    pub timestamp_nanos: u64,
    pub timestamp_iso: String,
    pub boot_offset_ms: Option<u64>,
    pub session_duration_ms: Option<u64>,

    // CONTEXT
    pub threat_level: String,
    pub risk_score: u8,
    pub matched_pattern: Option<String>,
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTreeNode {
    pub pid: u32,
    pub name: String,
    pub exe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessOrigin {
    Ssh {
        remote_addr: String,
        remote_port: u16,
    },
    Cron {
        schedule: Option<String>,
    },
    Agent {
        agent_id: String,
    },
    Container {
        id: String,
        name: String,
        image: Option<String>,
    },
    Systemd {
        unit: String,
    },
    Direct,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshInfo {
    pub remote_addr: String,
    pub remote_port: u16,
    pub local_port: u16,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub runtime: String,
}

impl ForensicContext {
    /// Collect forensic context for a process
    pub fn collect(pid: u32, command: &str, args: &[String]) -> Result<Self> {
        #[cfg(target_os = "linux")]
        let info = forensic_linux::collect_process_info(pid)?;
        #[cfg(target_os = "macos")]
        let info = forensic_macos::collect_process_info(pid)?;

        let now = chrono::Utc::now();
        let timestamp_nanos = (now.timestamp_nanos_opt().unwrap_or(0)) as u64;
        let timestamp_iso = now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);

        let is_root = info.uid == 0;
        let username = resolve_username(info.uid);
        let groups = resolve_groups(info.gid);
        let is_service = is_service_account(&username);

        let process_tree = {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::walk_process_tree(pid, 20)
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::walk_process_tree(pid, 20)
            }
        };
        let origin = {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::detect_origin(pid, &process_tree)
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::detect_origin(pid, &process_tree)
            }
        };

        let ssh_connection = if matches!(origin, ProcessOrigin::Ssh { .. }) {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::collect_ssh_info(pid)
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::collect_ssh_info(pid)
            }
        } else {
            None
        };

        let container_info = if matches!(origin, ProcessOrigin::Container { .. }) {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::collect_container_info(pid)
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::collect_container_info(pid)
            }
        } else {
            None
        };

        let agent_id = match &origin {
            ProcessOrigin::Agent { agent_id } => Some(agent_id.clone()),
            _ => None,
        };

        let shell = detect_shell(&process_tree);
        let cwd = {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::get_cwd(pid).unwrap_or_else(|_| "/unknown".to_string())
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::get_cwd(pid).unwrap_or_else(|_| "/unknown".to_string())
            }
        };
        let executable_path = info.exe.clone();
        let executable_hash = None; // expensive, opt-in

        let boot_offset_ms = {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::get_boot_offset_ms()
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::get_boot_offset_ms()
            }
        };
        let session_duration_ms = {
            #[cfg(target_os = "linux")]
            {
                forensic_linux::get_session_duration_ms(pid)
            }
            #[cfg(target_os = "macos")]
            {
                forensic_macos::get_session_duration_ms(pid)
            }
        };

        Ok(Self {
            uid: info.uid,
            gid: info.gid,
            username,
            groups,
            is_root,
            is_service,
            pid: info.pid,
            ppid: info.ppid,
            process_tree,
            session_leader_pid: info.session_leader,
            controlling_terminal: info.controlling_terminal,
            origin,
            ssh_connection,
            container_info,
            agent_id,
            command: command.to_string(),
            args: args.to_vec(),
            cwd,
            shell,
            executable_path,
            executable_hash,
            timestamp_nanos,
            timestamp_iso,
            boot_offset_ms,
            session_duration_ms,
            threat_level: String::new(),
            risk_score: 0,
            matched_pattern: None,
            snapshot_id: None,
        })
    }

    /// Compute risk score (0-100) based on available context
    pub fn compute_risk_score(&mut self) {
        let mut score: u8 = 0;

        // Root user
        if self.is_root {
            score += 30;
        }

        // Service account
        if self.is_service {
            score += 15;
        }

        // Origin risk
        match &self.origin {
            ProcessOrigin::Ssh { .. } => score += 25,
            ProcessOrigin::Cron { .. } => score += 10,
            ProcessOrigin::Container { .. } => score += 20,
            ProcessOrigin::Agent { .. } => score += 5,
            ProcessOrigin::Direct => score += 0,
            ProcessOrigin::Systemd { .. } => score += 10,
            ProcessOrigin::Unknown => score += 15,
        }

        // Threat level
        match self.threat_level.as_str() {
            "L1" | "Critical" => score += 35,
            "L2" | "High" => score += 20,
            "L3" | "Medium" => score += 10,
            _ => {}
        }

        // Dangerous commands
        let cmd_lower = self.command.to_lowercase();
        let dangerous_patterns = [
            "rm -rf",
            "mkfs",
            "dd if=",
            "chmod 777",
            "> /dev/sd",
            ":(){ :|:& };:",
            "wget|sh",
            "curl|sh",
            "nc -l",
            "python -c",
        ];
        for pat in &dangerous_patterns {
            if cmd_lower.contains(pat) {
                score += 10;
                break;
            }
        }

        self.risk_score = score.min(100);
    }

    /// Set threat level and compute risk
    pub fn with_threat(mut self, level: &str, pattern: Option<&str>) -> Self {
        self.threat_level = level.to_string();
        self.matched_pattern = pattern.map(String::from);
        self.compute_risk_score();
        self
    }

    /// Format a one-line origin description
    pub fn origin_description(&self) -> String {
        match &self.origin {
            ProcessOrigin::Ssh {
                remote_addr,
                remote_port,
            } => {
                format!("SSH from {}:{}", remote_addr, remote_port)
            }
            ProcessOrigin::Cron { schedule } => {
                format!(
                    "Cron{}",
                    schedule
                        .as_ref()
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default()
                )
            }
            ProcessOrigin::Agent { agent_id } => format!("Agent {}", agent_id),
            ProcessOrigin::Container { id, name, .. } => {
                format!("Container {} ({})", name, &id[..id.len().min(12)])
            }
            ProcessOrigin::Systemd { unit } => format!("systemd unit {}", unit),
            ProcessOrigin::Direct => "Direct login".to_string(),
            ProcessOrigin::Unknown => "Unknown origin".to_string(),
        }
    }

    /// Format process tree as arrow chain
    pub fn format_process_tree(&self) -> String {
        self.process_tree
            .iter()
            .rev()
            .map(|n| n.name.clone())
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

fn resolve_username(uid: u32) -> String {
    unsafe {
        let pwd = libc::getpwuid(uid);
        if !pwd.is_null() {
            std::ffi::CStr::from_ptr((*pwd).pw_name)
                .to_string_lossy()
                .to_string()
        } else {
            format!("uid={}", uid)
        }
    }
}

fn resolve_groups(gid: u32) -> Vec<String> {
    unsafe {
        let grp = libc::getgrgid(gid);
        if !grp.is_null() {
            vec![std::ffi::CStr::from_ptr((*grp).gr_name)
                .to_string_lossy()
                .to_string()]
        } else {
            vec![format!("gid={}", gid)]
        }
    }
}

fn is_service_account(username: &str) -> bool {
    let service_names = [
        "nobody",
        "daemon",
        "bin",
        "sys",
        "ftp",
        "mail",
        "www",
        "nginx",
        "apache",
        "mysql",
        "postgres",
        "redis",
        "docker",
        "systemd-network",
        "systemd-resolve",
        "polkitd",
        "sshd",
        "cron",
        "at",
        "lp",
        "uucp",
        "games",
    ];
    service_names.iter().any(|&s| s == username) || username.starts_with("_")
}

fn detect_shell(tree: &[ProcessTreeNode]) -> Option<String> {
    for node in tree.iter().rev() {
        let name = node.name.to_lowercase();
        if matches!(
            name.as_str(),
            "bash" | "zsh" | "sh" | "dash" | "fish" | "ksh" | "tcsh" | "csh"
        ) {
            return Some(node.name.clone());
        }
    }
    // Check if the process itself is a shell
    if !tree.is_empty() {
        let name = tree.last()?.name.to_lowercase();
        if matches!(
            name.as_str(),
            "bash"
                | "zsh"
                | "sh"
                | "dash"
                | "fish"
                | "ksh"
                | "tcsh"
                | "csh"
                | "python"
                | "python3"
                | "perl"
                | "ruby"
                | "node"
        ) {
            return Some(tree.last()?.name.clone());
        }
    }
    None
}

/// Platform-specific process info collected during forensic
pub(crate) struct PlatformProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub gid: u32,
    pub exe: String,
    pub comm: String,
    pub session_leader: u32,
    pub controlling_terminal: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree(names: &[&str]) -> Vec<ProcessTreeNode> {
        names
            .iter()
            .enumerate()
            .map(|(i, &name)| ProcessTreeNode {
                pid: 1000 + i as u32,
                name: name.to_string(),
                exe: format!("/usr/bin/{}", name),
            })
            .collect()
    }

    #[test]
    fn test_process_tree_node_serialization() {
        let node = ProcessTreeNode {
            pid: 1234,
            name: "bash".into(),
            exe: "/usr/bin/bash".into(),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: ProcessTreeNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pid, 1234);
        assert_eq!(back.name, "bash");
    }

    #[test]
    fn test_forensic_context_serialization_roundtrip() {
        let ctx = ForensicContext {
            uid: 0,
            gid: 0,
            username: "root".into(),
            groups: vec!["root".into()],
            is_root: true,
            is_service: false,
            pid: 1234,
            ppid: 5678,
            process_tree: make_tree(&["sshd", "bash", "rm"]),
            session_leader_pid: 5678,
            controlling_terminal: Some("pts/0".into()),
            origin: ProcessOrigin::Ssh {
                remote_addr: "192.168.1.50".into(),
                remote_port: 52342,
            },
            ssh_connection: Some(SshInfo {
                remote_addr: "192.168.1.50".into(),
                remote_port: 52342,
                local_port: 22,
                session_id: None,
            }),
            container_info: None,
            agent_id: None,
            command: "rm -rf /etc".into(),
            args: vec!["-rf".into(), "/etc".into()],
            cwd: "/home/alice".into(),
            shell: Some("bash".into()),
            executable_path: "/usr/bin/rm".into(),
            executable_hash: None,
            timestamp_nanos: 1712453732123456789,
            timestamp_iso: "2024-04-06T20:15:32.123456789Z".into(),
            boot_offset_ms: Some(86400000),
            session_duration_ms: Some(3600000),
            threat_level: "L1".into(),
            risk_score: 95,
            matched_pattern: Some("rm_rf".into()),
            snapshot_id: Some("snap-123".into()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: ForensicContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.uid, 0);
        assert_eq!(back.risk_score, 95);
        assert!(matches!(back.origin, ProcessOrigin::Ssh { .. }));
    }

    #[test]
    fn test_risk_score_root_ssh() {
        let mut ctx = ForensicContext {
            uid: 0,
            gid: 0,
            username: "root".into(),
            groups: vec![],
            is_root: true,
            is_service: false,
            pid: 1,
            ppid: 1,
            process_tree: vec![],
            session_leader_pid: 1,
            controlling_terminal: None,
            origin: ProcessOrigin::Ssh {
                remote_addr: "1.2.3.4".into(),
                remote_port: 22,
            },
            ssh_connection: None,
            container_info: None,
            agent_id: None,
            command: "rm -rf /".into(),
            args: vec![],
            cwd: "/".into(),
            shell: None,
            executable_path: "/usr/bin/rm".into(),
            executable_hash: None,
            timestamp_nanos: 0,
            timestamp_iso: String::new(),
            boot_offset_ms: None,
            session_duration_ms: None,
            threat_level: "L1".into(),
            risk_score: 0,
            matched_pattern: None,
            snapshot_id: None,
        };
        ctx.compute_risk_score();
        // root(30) + ssh(25) + L1(35) + dangerous cmd(10) = 100
        assert_eq!(ctx.risk_score, 100);
    }

    #[test]
    fn test_risk_score_normal_user() {
        let mut ctx = ForensicContext {
            uid: 1000,
            gid: 1000,
            username: "alice".into(),
            groups: vec![],
            is_root: false,
            is_service: false,
            pid: 1,
            ppid: 1,
            process_tree: vec![],
            session_leader_pid: 1,
            controlling_terminal: None,
            origin: ProcessOrigin::Direct,
            ssh_connection: None,
            container_info: None,
            agent_id: None,
            command: "ls".into(),
            args: vec![],
            cwd: "/home/alice".into(),
            shell: None,
            executable_path: "/usr/bin/ls".into(),
            executable_hash: None,
            timestamp_nanos: 0,
            timestamp_iso: String::new(),
            boot_offset_ms: None,
            session_duration_ms: None,
            threat_level: "L3".into(),
            risk_score: 0,
            matched_pattern: None,
            snapshot_id: None,
        };
        ctx.compute_risk_score();
        // L3(10) only
        assert_eq!(ctx.risk_score, 10);
    }

    #[test]
    fn test_risk_score_capped_at_100() {
        let mut ctx = ForensicContext {
            uid: 0,
            gid: 0,
            username: "root".into(),
            groups: vec![],
            is_root: true,
            is_service: true,
            pid: 1,
            ppid: 1,
            process_tree: vec![],
            session_leader_pid: 1,
            controlling_terminal: None,
            origin: ProcessOrigin::Unknown,
            ssh_connection: None,
            container_info: None,
            agent_id: None,
            command: "rm -rf /".into(),
            args: vec![],
            cwd: "/".into(),
            shell: None,
            executable_path: "/bin/rm".into(),
            executable_hash: None,
            timestamp_nanos: 0,
            timestamp_iso: String::new(),
            boot_offset_ms: None,
            session_duration_ms: None,
            threat_level: "L1".into(),
            risk_score: 0,
            matched_pattern: None,
            snapshot_id: None,
        };
        ctx.compute_risk_score();
        assert!(ctx.risk_score <= 100);
    }

    #[test]
    fn test_origin_description() {
        let ctx = ForensicContext {
            origin: ProcessOrigin::Ssh {
                remote_addr: "10.0.0.1".into(),
                remote_port: 22,
            },
            ..empty_ctx()
        };
        assert_eq!(ctx.origin_description(), "SSH from 10.0.0.1:22");

        let ctx2 = ForensicContext {
            origin: ProcessOrigin::Direct,
            ..empty_ctx()
        };
        assert_eq!(ctx2.origin_description(), "Direct login");
    }

    #[test]
    fn test_format_process_tree() {
        // Tree is built leaf-first (current pid first), so reverse gives root→leaf
        let ctx = ForensicContext {
            process_tree: make_tree(&["rm", "bash", "sshd"]),
            ..empty_ctx()
        };
        assert_eq!(ctx.format_process_tree(), "sshd → bash → rm");
    }

    #[test]
    fn test_detect_shell_in_tree() {
        let tree = make_tree(&["sshd", "bash", "rm"]);
        assert_eq!(detect_shell(&tree), Some("bash".into()));

        let tree2 = make_tree(&["systemd", "service", "node"]);
        assert_eq!(detect_shell(&tree2), Some("node".into()));
    }

    #[test]
    fn test_is_service_account() {
        assert!(is_service_account("nginx"));
        assert!(is_service_account("_www"));
        assert!(!is_service_account("alice"));
        assert!(!is_service_account("bob"));
    }

    #[test]
    fn test_with_threat() {
        let ctx = ForensicContext {
            command: "rm -rf /".into(),
            ..empty_ctx()
        }
        .with_threat("L1", Some("rm_rf"));
        assert_eq!(ctx.threat_level, "L1");
        assert_eq!(ctx.matched_pattern, Some("rm_rf".into()));
        assert!(ctx.risk_score > 0);
    }

    #[test]
    fn test_ssh_info_serialization() {
        let info = SshInfo {
            remote_addr: "1.2.3.4".into(),
            remote_port: 22,
            local_port: 22,
            session_id: Some("sess-1".into()),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("1.2.3.4"));
    }

    #[test]
    fn test_container_info_serialization() {
        let info = ContainerInfo {
            id: "abc123".into(),
            name: "web".into(),
            image: Some("nginx:latest".into()),
            runtime: "docker".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("nginx"));
    }

    #[test]
    fn test_process_origin_variants_serialization() {
        let origins = vec![
            ProcessOrigin::Ssh {
                remote_addr: "1.2.3.4".into(),
                remote_port: 22,
            },
            ProcessOrigin::Cron {
                schedule: Some("*/5 * * * *".into()),
            },
            ProcessOrigin::Agent {
                agent_id: "agent-1".into(),
            },
            ProcessOrigin::Container {
                id: "abc".into(),
                name: "web".into(),
                image: None,
            },
            ProcessOrigin::Systemd {
                unit: "nginx.service".into(),
            },
            ProcessOrigin::Direct,
            ProcessOrigin::Unknown,
        ];
        for origin in origins {
            let json = serde_json::to_string(&origin).unwrap();
            let back: ProcessOrigin = serde_json::from_str(&json).unwrap();
            // Just verify roundtrip doesn't panic
            let _ = format!("{:?}", back);
        }
    }

    fn empty_ctx() -> ForensicContext {
        ForensicContext {
            uid: 0,
            gid: 0,
            username: String::new(),
            groups: vec![],
            is_root: false,
            is_service: false,
            pid: 0,
            ppid: 0,
            process_tree: vec![],
            session_leader_pid: 0,
            controlling_terminal: None,
            origin: ProcessOrigin::Unknown,
            ssh_connection: None,
            container_info: None,
            agent_id: None,
            command: String::new(),
            args: vec![],
            cwd: String::new(),
            shell: None,
            executable_path: String::new(),
            executable_hash: None,
            timestamp_nanos: 0,
            timestamp_iso: String::new(),
            boot_offset_ms: None,
            session_duration_ms: None,
            threat_level: String::new(),
            risk_score: 0,
            matched_pattern: None,
            snapshot_id: None,
        }
    }
}

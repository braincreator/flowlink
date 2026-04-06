// FlowLink Shield — Process interceptor
// SIGSTOP/SIGKILL/SIGCONT management + /proc reader

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
#[derive(Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub comm: String,
    pub cmdline: String,
    pub exe: String,
}

impl ProcessInfo {
    /// Read full process info from /proc/{pid}
    pub fn from_pid(pid: u32) -> Result<Self> {
        let proc_dir = PathBuf::from(format!("/proc/{pid}"));

        let cmdline = fs::read_to_string(proc_dir.join("cmdline"))
            .unwrap_or_default()
            .replace('\0', " ")
            .trim()
            .to_string();

        let stat = fs::read_to_string(proc_dir.join("stat"))
            .unwrap_or_default();
        let ppid = parse_ppid_from_stat(&stat);

        let status = fs::read_to_string(proc_dir.join("status"))
            .unwrap_or_default();
        let uid = parse_uid_from_status(&status);

        let comm = fs::read_to_string(proc_dir.join("comm"))
            .unwrap_or_default()
            .trim()
            .to_string();

        let exe = fs::read_link(proc_dir.join("exe"))
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        Ok(Self {
            pid,
            ppid,
            uid,
            comm,
            cmdline,
            exe,
        })
    }

    pub fn full_command(&self) -> String {
        if self.cmdline.is_empty() {
            self.comm.clone()
        } else {
            self.cmdline.clone()
        }
    }

    pub fn username(&self) -> String {
        // Try to resolve UID to username
        unsafe {
            let pwd = libc::getpwuid(self.uid);
            if !pwd.is_null() {
                let name = std::ffi::CStr::from_ptr((*pwd).pw_name);
                name.to_string_lossy().to_string()
            } else {
                format!("uid={}", self.uid)
            }
        }
    }
}

fn parse_ppid_from_stat(stat: &str) -> u32 {
    // Format: pid (comm) state ppid ...
    // Find closing ')', then parse ppid (3rd field after)
    if let Some(close) = stat.rfind(')') {
        let rest = &stat[close + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() >= 2 {
            return fields[1].parse().unwrap_or(0);
        }
    }
    0
}

fn parse_uid_from_status(status: &str) -> u32 {
    for line in status.lines() {
        if line.starts_with("Uid:") {
            return line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Send signal to a process
pub fn send_signal(pid: u32, sig: i32) -> Result<()> {
    let ret = unsafe { libc::kill(pid as i32, sig) };
    if ret != 0 {
        anyhow::bail!("Failed to send signal {} to pid {}: errno {}", sig, pid, std::io::Error::last_os_error());
    }
    Ok(())
}

pub fn sigstop(pid: u32) -> Result<()> {
    send_signal(pid, libc::SIGSTOP)
}

pub fn sigcont(pid: u32) -> Result<()> {
    send_signal(pid, libc::SIGCONT)
}

pub fn sigkill(pid: u32) -> Result<()> {
    send_signal(pid, libc::SIGKILL)
}

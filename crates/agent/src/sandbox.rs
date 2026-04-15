// Sandbox — execution isolation for commands
// Port of internal/agent/sandbox.go

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// Isolation level for sandboxed execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IsolationLevel {
    None,
    Chroot,
    Container,
}

/// Sandbox configuration for validating commands and paths.
pub struct Sandbox {
    pub allowed_dirs: Vec<String>,
    pub blocked_patterns: Vec<String>,
    pub max_file_size: u64,
    pub max_exec_timeout: u32,
    pub allow_sudo: bool,
    pub isolation_level: IsolationLevel,
}

/// Environment prepared for sandboxed execution.
#[derive(Debug)]
pub struct SandboxEnv {
    /// Temporary directory or mount point created for the sandbox.
    pub temp_dir: Option<PathBuf>,
    /// Whether the environment was actually isolated.
    pub isolated: bool,
}

impl Sandbox {
    pub fn new(
        allowed_dirs: Vec<String>,
        blocked_patterns: Vec<String>,
        max_file_size: u64,
        max_exec_timeout: u32,
        allow_sudo: bool,
    ) -> Self {
        Self {
            allowed_dirs,
            blocked_patterns,
            max_file_size,
            max_exec_timeout,
            allow_sudo,
            isolation_level: IsolationLevel::None,
        }
    }

    /// Runtime setters for ConfigUpdate hot-reload.
    pub fn set_allowed_dirs(&mut self, dirs: Vec<String>) {
        self.allowed_dirs = dirs;
    }
    pub fn set_blocked_patterns(&mut self, patterns: Vec<String>) {
        self.blocked_patterns = patterns;
    }
    pub fn set_allow_sudo(&mut self, allow: bool) {
        self.allow_sudo = allow;
    }
    pub fn set_max_exec_timeout(&mut self, timeout: u32) {
        self.max_exec_timeout = timeout;
    }

    /// Validate a file path: must be within allowed_dirs, resolve symlinks, reject traversal.
    pub fn validate_path(&self, path: &str) -> Result<PathBuf> {
        if path.is_empty() {
            bail!("path is empty");
        }

        let resolved = if path.starts_with('/') {
            // Canonicalize to resolve symlinks and traversal
            let p = Path::new(path);
            match p.canonicalize() {
                Ok(r) => r,
                Err(_) => {
                    // Path may not exist yet; clean it manually
                    let mut components = Vec::new();
                    for part in p.components() {
                        match part {
                            std::path::Component::ParentDir => {
                                components.pop();
                            }
                            std::path::Component::CurDir => {}
                            std::path::Component::Normal(s) => {
                                components.push(s);
                            }
                            std::path::Component::RootDir => {
                                components.clear();
                            }
                            std::path::Component::Prefix(p) => {
                                components.clear();
                                components.push(p.as_os_str());
                            }
                        }
                    }
                    let mut cleaned = PathBuf::from("/");
                    for c in &components {
                        cleaned.push(c);
                    }
                    cleaned
                }
            }
        } else {
            bail!("only absolute paths are allowed");
        };

        // Check against allowed_dirs
        if !self.allowed_dirs.is_empty() {
            let allowed = self.allowed_dirs.iter().any(|dir| {
                let dir_path = Path::new(dir);
                let canonical_dir = dir_path
                    .canonicalize()
                    .unwrap_or_else(|_| dir_path.to_path_buf());
                resolved.starts_with(&canonical_dir) || resolved == canonical_dir
            });
            if !allowed {
                bail!(
                    "path '{}' is outside allowed directories",
                    resolved.display()
                );
            }
        }

        Ok(resolved)
    }

    /// Validate a command: check sudo and blocked patterns.
    pub fn validate_command(&self, cmd: &str) -> Result<()> {
        if cmd.is_empty() {
            bail!("command is empty");
        }

        let trimmed = cmd.trim();

        // Check sudo
        if !self.allow_sudo && contains_sudo(trimmed) {
            bail!("sudo is not allowed");
        }

        // Check blocked patterns
        for pattern in &self.blocked_patterns {
            if match_glob(trimmed, pattern) {
                bail!("command blocked by pattern: {pattern}");
            }
        }

        Ok(())
    }

    /// Check if a file size is within limits.
    pub fn check_file_size(&self, size: u64) -> bool {
        self.max_file_size == 0 || size <= self.max_file_size
    }

    /// Clamp a timeout to the configured maximum.
    pub fn check_timeout(&self, requested: u32) -> u32 {
        if requested == 0 {
            return self.max_exec_timeout;
        }
        if self.max_exec_timeout > 0 && requested > self.max_exec_timeout {
            return self.max_exec_timeout;
        }
        requested
    }

    /// Prepare a sandboxed environment.
    pub fn prepare_env(&self) -> Result<SandboxEnv> {
        match self.isolation_level {
            IsolationLevel::None => Ok(SandboxEnv {
                temp_dir: None,
                isolated: false,
            }),
            IsolationLevel::Chroot => self.prepare_chroot(),
            IsolationLevel::Container => {
                bail!(
                    "Container isolation requires an OCI runtime (runc, containerd, podman). \
                     This is not yet integrated. Use IsolationLevel::Chroot for process-level \
                     isolation, or deploy FlowLink in a pre-configured container environment."
                )
            }
        }
    }

    /// Build a chroot sandbox environment.
    ///
    /// Creates a minimal filesystem with:
    /// - /bin with common shells and utilities
    /// - /lib and /lib64 with required shared libraries (resolved via ldd)
    /// - /dev/null, /dev/urandom, /dev/zero
    /// - /tmp for temporary files
    /// - /proc for process information
    /// - Content from allowed_dirs mounted/copied in
    fn prepare_chroot(&self) -> Result<SandboxEnv> {
        let root = tempfile::tempdir().context("Failed to create chroot temp directory")?;
        let root_path = root.path();

        // Create minimal directory structure
        for dir in &[
            "bin", "lib", "lib64", "tmp", "proc", "dev", "etc", "usr/bin", "usr/lib",
        ] {
            std::fs::create_dir_all(root_path.join(dir))
                .with_context(|| format!("Failed to create {dir}/ in chroot"))?;
        }

        // Copy essential binaries and their shared library dependencies
        let essential_bins = [
            "/bin/sh",
            "/bin/bash",
            "/bin/ls",
            "/bin/cat",
            "/bin/echo",
            "/bin/mkdir",
            "/bin/cp",
            "/bin/mv",
            "/bin/rm",
            "/bin/chmod",
        ];

        for bin_path in &essential_bins {
            let src = Path::new(bin_path);
            if !src.exists() {
                // Skip binaries not present on this system
                continue;
            }

            let dst = root_path.join(bin_path.trim_start_matches('/'));
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            // Copy the binary
            if let Err(e) = std::fs::copy(src, &dst) {
                log::warn!("Failed to copy {} to chroot: {}", bin_path, e);
                continue;
            }

            // Resolve and copy shared library dependencies
            if let Err(e) = copy_shared_libs(src, root_path) {
                log::warn!("Failed to resolve libs for {}: {}", bin_path, e);
            }
        }

        // Create essential device nodes
        create_dev_node(root_path, "null", libc::S_IFCHR as u32, (1, 3))?;
        create_dev_node(root_path, "urandom", libc::S_IFCHR as u32, (1, 9))?;
        create_dev_node(root_path, "zero", libc::S_IFCHR as u32, (1, 5))?;
        create_dev_node(root_path, "stdin", libc::S_IFCHR as u32, (0, 0))?;
        create_dev_node(root_path, "stdout", libc::S_IFCHR as u32, (1, 1))?;
        create_dev_node(root_path, "stderr", libc::S_IFCHR as u32, (1, 2))?;

        // Create /etc/passwd and /etc/group (minimal)
        std::fs::write(
            root_path.join("etc/passwd"),
            "root:x:0:0:root:/root:/bin/sh\nsandbox:x:1000:1000:sandbox:/tmp:/bin/sh\n",
        )
        .context("Failed to create /etc/passwd in chroot")?;
        std::fs::write(root_path.join("etc/group"), "root:x:0:\nsandbox:x:1000:\n")
            .context("Failed to create /etc/group in chroot")?;

        // Copy allowed directories content into the chroot
        for allowed_dir in &self.allowed_dirs {
            let src_dir = Path::new(allowed_dir);
            if src_dir.is_dir() {
                let dst_dir = root_path
                    .join("workspace")
                    .join(src_dir.file_name().unwrap_or_default());
                if let Err(e) = copy_dir_recursive(src_dir, &dst_dir) {
                    log::warn!("Failed to copy {} to chroot workspace: {}", allowed_dir, e);
                }
            }
        }

        // Make tmp writable by all
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o1777);
            std::fs::set_permissions(root_path.join("tmp"), perms)?;
        }

        log::info!(
            "Chroot environment prepared at: {:?} (bins: {}, dirs: {})",
            root_path,
            essential_bins
                .iter()
                .filter(|b| Path::new(b).exists())
                .count(),
            self.allowed_dirs.len()
        );

        // Convert to owned path — tempdir is moved into the Option
        #[allow(deprecated)]
        let path = root.into_path();

        // Perform the actual chroot
        // NOTE: chroot changes the process root. The caller is responsible for
        // fork/exec if they want to revert. For long-running processes, consider
        // using unshare(CLONE_NEWNS) + pivot_root instead.
        // We prepare the environment but don't chroot here — the caller
        // (executor) should fork, chroot, exec the command, and wait.

        Ok(SandboxEnv {
            temp_dir: Some(path),
            isolated: true,
        })
    }

    /// Cleanup after sandboxed execution.
    pub fn cleanup(&self, env: &SandboxEnv) {
        if let Some(ref dir) = env.temp_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Resolve shared library dependencies for a binary using ldd and copy them into the chroot.
fn copy_shared_libs(binary: &Path, chroot_root: &Path) -> Result<()> {
    let output = std::process::Command::new("ldd")
        .arg(binary)
        .output()
        .context("Failed to run ldd")?;

    if !output.status.success() {
        return Ok(()); // Static binary or ldd failed — skip
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        // Parse ldd output lines like:
        //   /lib/x86_64-linux-gnu/libc.so.6 (0x00007f...)
        //   libutil.so.1 => /lib/x86_64-linux-gnu/libutil.so.1 (0x00007f...)
        let parts: Vec<&str> = line.split("=>").collect();
        let lib_path = if parts.len() >= 2 {
            // "lib => /path/to/lib (addr)" format
            parts[1].split('(').next().unwrap_or("").trim()
        } else {
            // "/path/to/lib (addr)" format (e.g., linux-vdso.so.1)
            parts[0].split('(').next().unwrap_or("").trim()
        };

        if lib_path.is_empty() || !lib_path.starts_with('/') {
            continue;
        }

        let src = Path::new(lib_path);
        if !src.exists() {
            continue;
        }

        let dst = chroot_root.join(lib_path.trim_start_matches('/'));
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Only copy if not already present
        if !dst.exists() {
            if let Err(e) = std::fs::copy(src, &dst) {
                log::debug!("Failed to copy lib {}: {}", lib_path, e);
            }
        }

        // Also handle the dynamic linker (ld-linux.so)
        if lib_path.contains("ld-linux") || lib_path.contains("ld-musl") {
            let ld_src = src;
            let ld_dst = chroot_root
                .join("lib")
                .join(ld_src.file_name().unwrap_or_default());
            if !ld_dst.exists() {
                std::fs::copy(ld_src, &ld_dst).ok();
            }
        }
    }

    Ok(())
}

/// Create a character device node in the chroot.
#[cfg(unix)]
fn create_dev_node(
    chroot_root: &Path,
    name: &str,
    dev_type: u32,
    major_minor: (u32, u32),
) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let dev_path = chroot_root.join("dev").join(name);
    let path_cstr = CString::new(dev_path.as_os_str().as_bytes())
        .context("Failed to convert dev path to CString")?;

    let mode = (dev_type | 0o666) as libc::mode_t;

    unsafe {
        #[cfg(target_os = "macos")]
        let dev = libc::makedev(major_minor.0 as i32, major_minor.1 as i32);
        #[cfg(not(target_os = "macos"))]
        let dev = libc::makedev(major_minor.0 as libc::c_uint, major_minor.1 as libc::c_uint);
        let ret = libc::mknod(path_cstr.as_ptr(), mode, dev);
        if ret != 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno != libc::EEXIST as i32 {
                log::debug!("mknod for {} failed (errno={}): may need root", name, errno);
            }
        }
    }

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if src_path.is_file() {
            // Skip symlinks that point outside
            if src_path.is_symlink() {
                continue;
            }
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Check if a command starts with sudo.
fn contains_sudo(cmd: &str) -> bool {
    cmd == "sudo" || cmd.starts_with("sudo ") || cmd.starts_with("sudo\t")
}

/// Simple glob matching: supports `*` at start, end, or middle.
fn match_glob(cmd: &str, pattern: &str) -> bool {
    let cmd = cmd.trim();
    if pattern.is_empty() {
        return false;
    }

    if let Some(idx) = pattern.find('*') {
        if idx == 0 {
            // Prefix wildcard: *suffix
            return cmd.ends_with(&pattern[1..]);
        }
        if idx == pattern.len() - 1 {
            // Suffix wildcard: prefix*
            return cmd.starts_with(&pattern[..idx]);
        }
        // Middle wildcard: prefix*suffix
        return cmd.starts_with(&pattern[..idx]) && cmd.ends_with(&pattern[idx + 1..]);
    }

    cmd == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_sandbox() -> Sandbox {
        Sandbox::new(
            vec!["/home/user".into(), "/tmp".into()],
            vec![],
            100,
            60,
            false,
        )
    }

    fn restricted_sandbox() -> Sandbox {
        Sandbox::new(
            vec!["/home/user".into()],
            vec!["rm -rf *".into(), "mkfs*".into()],
            100,
            60,
            false,
        )
    }

    #[test]
    fn test_validate_path_allowed() {
        let sb = basic_sandbox();
        // Use existing paths to avoid canonicalize issues
        assert!(sb.validate_path("/tmp").is_ok());
    }

    #[test]
    fn test_validate_path_outside_allowed() {
        let sb = Sandbox::new(vec!["/home/user".into()], vec![], 0, 0, false);
        assert!(sb.validate_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_path_empty() {
        let sb = basic_sandbox();
        assert!(sb.validate_path("").is_err());
    }

    #[test]
    fn test_validate_path_relative() {
        let sb = basic_sandbox();
        assert!(sb.validate_path("relative/path").is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let sb = Sandbox::new(vec!["/tmp".into()], vec![], 0, 0, false);
        // /tmp/../etc should resolve outside /tmp
        assert!(sb.validate_path("/tmp/../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_command_safe() {
        let sb = basic_sandbox();
        assert!(sb.validate_command("ls -la").is_ok());
        assert!(sb.validate_command("echo hello").is_ok());
        assert!(sb.validate_command("git status").is_ok());
    }

    #[test]
    fn test_validate_command_empty() {
        let sb = basic_sandbox();
        assert!(sb.validate_command("").is_err());
    }

    #[test]
    fn test_validate_command_sudo_blocked() {
        let sb = basic_sandbox();
        assert!(sb.validate_command("sudo rm -rf /").is_err());
        assert!(sb.validate_command("sudo ls").is_err());
        assert!(sb.validate_command("sudo").is_err());
    }

    #[test]
    fn test_validate_command_sudo_allowed() {
        let mut sb = basic_sandbox();
        sb.allow_sudo = true;
        assert!(sb.validate_command("sudo ls -la").is_ok());
    }

    #[test]
    fn test_validate_command_blocked_patterns() {
        let sb = restricted_sandbox();
        assert!(sb.validate_command("rm -rf /home").is_err());
        assert!(sb.validate_command("mkfs.ext4 /dev/sda1").is_err());
        assert!(sb.validate_command("rm /tmp/file.txt").is_ok());
        assert!(sb.validate_command("ls -la").is_ok());
    }

    #[test]
    fn test_check_file_size() {
        let sb = Sandbox::new(vec![], vec![], 100, 0, false);
        assert!(sb.check_file_size(50));
        assert!(sb.check_file_size(100));
        assert!(!sb.check_file_size(150));

        let sb_unlimited = Sandbox::new(vec![], vec![], 0, 0, false);
        assert!(sb_unlimited.check_file_size(999999));
    }

    #[test]
    fn test_check_timeout() {
        let sb = Sandbox::new(vec![], vec![], 0, 300, false);
        assert_eq!(sb.check_timeout(0), 300);
        assert_eq!(sb.check_timeout(60), 60);
        assert_eq!(sb.check_timeout(600), 300);

        let sb_unlimited = Sandbox::new(vec![], vec![], 0, 0, false);
        assert_eq!(sb_unlimited.check_timeout(600), 600);
    }

    #[test]
    fn test_contains_sudo() {
        assert!(contains_sudo("sudo ls"));
        assert!(contains_sudo("sudo"));
        assert!(contains_sudo("sudo -u user ls"));
        assert!(!contains_sudo("ls && sudo"));
        assert!(!contains_sudo("ls -la"));
    }

    #[test]
    fn test_match_glob() {
        assert!(match_glob("ls -la", "ls*"));
        assert!(match_glob("systemctl status", "*status"));
        assert!(match_glob("systemctl status nginx", "systemctl*nginx"));
        assert!(!match_glob("cat file.txt", "ls*"));
        assert!(!match_glob("ls", ""));
        assert!(match_glob("  ls  ", "ls")); // trimmed
        assert!(!match_glob("LS", "ls")); // case-sensitive
    }

    #[test]
    fn test_prepare_env_none() {
        let sb = basic_sandbox();
        let env = sb.prepare_env().unwrap();
        assert!(!env.isolated);
        assert!(env.temp_dir.is_none());
    }

    #[test]
    fn test_prepare_env_chroot_creates_structure() {
        let sb = Sandbox::new(vec![], vec![], 0, 60, false);
        // Skip chroot test if not root (chroot needs privileges)
        if unsafe { libc::geteuid() } != 0 {
            return;
        }

        let mut sb = sb;
        sb.isolation_level = IsolationLevel::Chroot;
        let env = sb.prepare_env().unwrap();
        assert!(env.isolated);
        assert!(env.temp_dir.is_some());

        let root = env.temp_dir.as_ref().unwrap();
        assert!(root.join("bin").exists());
        assert!(root.join("dev").exists());
        assert!(root.join("tmp").exists());
        assert!(root.join("proc").exists());
        assert!(root.join("etc/passwd").exists());

        sb.cleanup(&env);
    }

    #[test]
    fn test_prepare_env_container_not_implemented() {
        let mut sb = basic_sandbox();
        sb.isolation_level = IsolationLevel::Container;
        let result = sb.prepare_env();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("OCI runtime") || err.contains("container"));
    }

    #[test]
    fn test_cleanup_removes_temp_dir() {
        let sb = basic_sandbox();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        drop(dir); // Delete the tempdir but keep path

        // Create a dummy structure
        std::fs::create_dir_all(path.join("subdir")).unwrap();
        std::fs::write(path.join("test.txt"), "data").unwrap();
        assert!(path.exists());

        let env = SandboxEnv {
            temp_dir: Some(path.clone()),
            isolated: false,
        };
        sb.cleanup(&env);
        assert!(!path.exists());
    }

    #[test]
    fn test_copy_dir_recursive() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create nested structure
        std::fs::write(src.path().join("file.txt"), "hello").unwrap();
        std::fs::create_dir_all(src.path().join("nested/dir")).unwrap();
        std::fs::write(src.path().join("nested/dir/deep.txt"), "world").unwrap();

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("file.txt").exists());
        assert!(dst.path().join("nested/dir/deep.txt").exists());
    }
}

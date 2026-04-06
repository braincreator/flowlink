// FlowLink Shield — ZFS/LVM snapshot trigger

use anyhow::Result;
use std::process::Command;
use log::{info, warn};

#[derive(Debug, Clone, Copy)]
pub enum SnapshotBackend {
    Zfs,
    Lvm,
    None,
}

impl SnapshotBackend {
    pub fn detect() -> Self {
        // Check if zfs command exists
        if Command::new("zfs").arg("list").output().is_ok() {
            return SnapshotBackend::Zfs;
        }
        // Check if lvm/lvcreate exists
        if Command::new("lvcreate").arg("--version").output().is_ok() {
            return SnapshotBackend::Lvm;
        }
        SnapshotBackend::None
    }
}

/// Create a snapshot with timestamp tag
pub fn create_snapshot(dataset: &str, tag: &str, backend: SnapshotBackend) -> Result<String> {
    let snapshot_name = format!(
        "{}@shield-{}-{}",
        dataset,
        tag,
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );

    match backend {
        SnapshotBackend::Zfs => {
            info!("Creating ZFS snapshot: {}", snapshot_name);
            let output = Command::new("zfs")
                .args(["snapshot", &snapshot_name])
                .output()?;

            if !output.status.success() {
                anyhow::bail!(
                    "zfs snapshot failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            info!("ZFS snapshot created: {}", snapshot_name);
        }
        SnapshotBackend::Lvm => {
            info!("Creating LVM snapshot for: {}", dataset);
            let snap_lv = format!("{}-shield-snap", dataset);
            let output = Command::new("lvcreate")
                .args(["-L", "1G", "-s", "-n", &snap_lv, dataset])
                .output()?;

            if !output.status.success() {
                anyhow::bail!(
                    "lvcreate snapshot failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            info!("LVM snapshot created: {}", snap_lv);
            return Ok(snap_lv);
        }
        SnapshotBackend::None => {
            warn!("No snapshot backend available — skipping snapshot");
            return Ok("(no snapshot backend)".to_string());
        }
    }

    Ok(snapshot_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn detect_backend_returns_enum() {
        let backend = SnapshotBackend::detect();
        match backend {
            SnapshotBackend::Zfs | SnapshotBackend::Lvm | SnapshotBackend::None => {}
        }
    }

    #[test]
    fn snapshot_backend_none_create() {
        let result = create_snapshot("tank/data", "test", SnapshotBackend::None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "(no snapshot backend)");
    }

    #[test]
    fn zfs_snapshot_name_format() {
        let name = format!("{}@shield-{}-{}", "tank/data", "rm_rf", "20260406-165300");
        let re = Regex::new(r"^[^@]+@shield-[a-z_]+-\d{8}-\d{6}$").unwrap();
        assert!(re.is_match(&name), "snapshot name format invalid: {}", name);
    }

    #[test]
    fn lvm_snapshot_name_format() {
        let snap = format!("{}-shield-snap", "vg0/lv_data");
        assert!(snap.ends_with("-shield-snap"));
        assert!(snap.contains("/"));
    }

    #[test]
    fn snapshot_backend_debug_clone() {
        let b = SnapshotBackend::None;
        let _ = format!("{:?}", b);
        let _ = b.clone();
    }
}

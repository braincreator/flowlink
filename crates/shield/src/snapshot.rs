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

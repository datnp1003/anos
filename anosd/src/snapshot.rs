//! Snapshot System — btrfs/LVM snapshot creation and rollback.
//!
//! Phase 4: automatic snapshots before dangerous tool executions,
//! with rollback capability when things go wrong.

use serde::{Deserialize, Serialize};
use std::process::Command;

/// A system snapshot entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub reason: String,
    pub size_bytes: u64,
}

/// Snapshot manager — creates and manages system snapshots
pub struct SnapshotManager {
    /// Max snapshots to retain
    #[allow(dead_code)]
    max_snapshots: usize,
}

impl SnapshotManager {
    #[allow(dead_code)]
    pub fn new(max_snapshots: usize) -> Self {
        Self { max_snapshots }
    }

    /// Check if btrfs is available on the system
    pub fn btrfs_available() -> bool {
        Command::new("which")
            .arg("btrfs")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if any btrfs subvolume exists for root
    pub fn has_btrfs_root() -> bool {
        if !Self::btrfs_available() {
            return false;
        }
        Command::new("btrfs")
            .args(["subvolume", "list", "/"])
            .output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    }

    /// Create a snapshot with a reason tag
    pub fn create(reason: &str) -> Option<Snapshot> {
        if !Self::btrfs_available() {
            tracing::warn!("Snapshot: btrfs not available, skipping");
            return None;
        }

        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let safe_reason = reason
            .chars()
            .take(30)
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let snapshot_name = format!("anos-{}-{}", ts, safe_reason);
        let snapshot_path = format!("/mnt/snapshots/{}", snapshot_name);

        // Ensure snapshot dir exists
        let _ = std::fs::create_dir_all("/mnt/snapshots");

        // Create btrfs snapshot of root
        let output = Command::new("sudo")
            .args(["btrfs", "subvolume", "snapshot", "-r", "/", &snapshot_path])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let size = Self::estimate_size(&snapshot_path);
                tracing::info!(
                    "Snapshot created: {} ({} bytes) — {}",
                    snapshot_name,
                    size,
                    reason
                );
                Some(Snapshot {
                    id: snapshot_name,
                    path: snapshot_path,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    reason: reason.to_string(),
                    size_bytes: size,
                })
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                tracing::warn!("Snapshot failed: {}", err.trim());
                // Fallback: try without sudo
                let output2 = Command::new("btrfs")
                    .args(["subvolume", "snapshot", "-r", "/", &snapshot_path])
                    .output();
                match output2 {
                    Ok(o2) if o2.status.success() => {
                        let size = Self::estimate_size(&snapshot_path);
                        Some(Snapshot {
                            id: snapshot_name,
                            path: snapshot_path,
                            created_at: chrono::Utc::now().to_rfc3339(),
                            reason: reason.to_string(),
                            size_bytes: size,
                        })
                    }
                    _ => None,
                }
            }
            Err(e) => {
                tracing::error!("Snapshot command error: {}", e);
                None
            }
        }
    }

    /// List all Anos-created snapshots
    pub fn list() -> Vec<Snapshot> {
        let mut snapshots = Vec::new();

        if !Self::btrfs_available() {
            return snapshots;
        }

        if let Ok(output) = Command::new("btrfs")
            .args(["subvolume", "list", "/mnt/snapshots"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("anos-") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 9 {
                        let name = parts.last().unwrap_or(&"unknown");
                        let path = format!("/mnt/snapshots/{}", name);
                        snapshots.push(Snapshot {
                            id: name.to_string(),
                            path: path.clone(),
                            created_at: String::new(),
                            reason: String::new(),
                            size_bytes: Self::estimate_size(&path),
                        });
                    }
                }
            }
        }
        snapshots
    }

    /// Rollback to a specific snapshot
    /// WARNING: This replaces the current root subvolume
    #[allow(dead_code)]
    pub fn rollback(snapshot_id: &str) -> Result<String, String> {
        if !Self::btrfs_available() {
            return Err("btrfs not available".into());
        }

        let snapshot_path = format!("/mnt/snapshots/{}", snapshot_id);

        // Create a pre-rollback snapshot first (safety net)
        let _ = Self::create("pre-rollback");

        // btrfs send/receive to restore
        let send = Command::new("btrfs")
            .args(["send", &snapshot_path])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("btrfs send failed: {}", e))?;

        let receive = Command::new("sudo")
            .args(["btrfs", "receive", "/"])
            .stdin(send.stdout.unwrap())
            .output()
            .map_err(|e| format!("btrfs receive failed: {}", e))?;

        if receive.status.success() {
            let msg = format!(
                "Rolled back to snapshot: {}. A reboot is recommended.",
                snapshot_id
            );
            tracing::info!("{}", msg);
            Ok(msg)
        } else {
            let err = String::from_utf8_lossy(&receive.stderr);
            Err(format!("Rollback failed: {}", err))
        }
    }

    /// Delete old snapshots, keeping only the most recent N
    #[allow(dead_code)]
    pub fn prune(max_keep: usize) -> usize {
        let mut snapshots = Self::list();
        if snapshots.len() <= max_keep {
            return 0;
        }

        // Sort by name (timestamp-based) — oldest first
        snapshots.sort_by(|a, b| a.id.cmp(&b.id));

        let to_delete = snapshots.len() - max_keep;
        let mut deleted = 0;

        for snap in snapshots.iter().take(to_delete) {
            let output = Command::new("btrfs")
                .args(["subvolume", "delete", &snap.path])
                .output();

            match output {
                Ok(o) if o.status.success() => {
                    tracing::info!("Pruned old snapshot: {}", snap.id);
                    deleted += 1;
                }
                _ => {
                    // Try with sudo
                    if let Ok(o2) = Command::new("sudo")
                        .args(["btrfs", "subvolume", "delete", &snap.path])
                        .output()
                    {
                        if o2.status.success() {
                            deleted += 1;
                        }
                    }
                }
            }
        }
        deleted
    }

    /// Check if snapshot support is active
    pub fn status() -> String {
        let has_btrfs = Self::btrfs_available();
        let has_root = Self::has_btrfs_root();
        let count = Self::list().len();

        if !has_btrfs {
            "Snapshot: ❌ btrfs tools not installed".into()
        } else if !has_root {
            "Snapshot: ⚠️ btrfs available but root is not a subvolume".into()
        } else {
            format!("Snapshot: ✅ Active (btrfs) — {} snapshots", count)
        }
    }

    fn estimate_size(path: &str) -> u64 {
        Command::new("du")
            .args(["-sb", path])
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(0)
    }
}

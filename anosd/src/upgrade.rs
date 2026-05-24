//! Self-Upgrade Tool — Anos can upgrade itself.
//!
//! Phase 4: git pull latest, cargo build --release, restart daemon.
//! Auto-rollback on build failure.

use serde::{Deserialize, Serialize};
use std::process::Command;

/// Upgrade source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum UpgradeSource {
    /// Pull from git + build
    Git,
    /// Download pre-built binary from GitHub releases
    Release(String),
}

/// Upgrade result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct UpgradeResult {
    pub success: bool,
    pub from_version: String,
    pub to_version: String,
    pub message: String,
    pub duration_secs: f64,
}

/// Self-upgrade manager
pub struct SelfUpgrade {
    anos_dir: String,
    current_version: String,
}

impl SelfUpgrade {
    pub fn new(anos_dir: &str) -> Self {
        Self {
            anos_dir: anos_dir.to_string(),
            current_version: Self::detect_current_version(anos_dir),
        }
    }

    /// Detect current installed version from Cargo.toml
    fn detect_current_version(anos_dir: &str) -> String {
        let cargo_toml = format!("{}/anosd/Cargo.toml", anos_dir);
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            for line in content.lines() {
                if let Some(v) = line.trim().strip_prefix("version = ") {
                    return v.trim_matches('"').to_string();
                }
            }
        }
        "unknown".into()
    }

    /// Check for available updates from GitHub releases
    pub fn check_updates() -> Option<(String, String)> {
        let output = Command::new("gh")
            .args([
                "release",
                "list",
                "--repo",
                "datnp1003/anos",
                "--limit",
                "1",
                "--exclude-drafts",
                "--exclude-pre-releases",
            ])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                let parts: Vec<&str> = text.split_whitespace().collect();
                if parts.len() >= 3 {
                    let tag = parts[1].to_string();
                    let title = parts[2..].join(" ");
                    Some((tag, title))
                } else {
                    None
                }
            }
            _ => {
                tracing::info!("gh CLI not available, checking git tags");
                if let Ok(o2) = Command::new("git")
                    .args([
                        "-C",
                        "~/.openclaw/workspace/anos",
                        "tag",
                        "--sort=-creatordate",
                    ])
                    .output()
                {
                    let text = String::from_utf8_lossy(&o2.stdout);
                    let latest = text.lines().next().unwrap_or("").to_string();
                    if !latest.is_empty() {
                        return Some((latest, "git tag".into()));
                    }
                }
                None
            }
        }
    }

    /// Try binary upgrade first (download from GitHub releases)
    #[allow(dead_code)]
    pub fn upgrade_binary(version: &str) -> Result<String, String> {
        tracing::info!("Upgrading binary to {}", version);

        // Download release
        let url = format!(
            "https://github.com/datnp1003/anos/releases/download/{}/anosd",
            version
        );
        let output = Command::new("curl")
            .args(["-fsSL", &url, "-o", "/tmp/anosd-new"])
            .output()
            .map_err(|e| format!("download failed: {}", e))?;

        if !output.status.success() {
            return Err("no binary release for this version, try source build".into());
        }

        // Verify it's executable
        let _ = Command::new("chmod")
            .args(["+x", "/tmp/anosd-new"])
            .output();

        // Test that it runs
        let test = Command::new("/tmp/anosd-new").arg("--version").output();

        match test {
            Ok(o) if o.status.success() => {
                // Replace current binary
                let current_bin = format!(
                    "{}/anosd/target/release/anosd",
                    std::env::var("HOME").unwrap_or_default()
                );
                let _ = std::fs::copy("/tmp/anosd-new", &current_bin);
                Ok(format!(
                    "Binary upgraded to {} — restart anosd to apply. Binary at: {}",
                    version, current_bin
                ))
            }
            _ => {
                let _ = std::fs::remove_file("/tmp/anosd-new");
                Err("downloaded binary failed verification".into())
            }
        }
    }

    /// Source build upgrade: git pull + cargo build --release
    #[allow(dead_code)]
    pub fn upgrade_source(&self) -> Result<UpgradeResult, String> {
        let start = std::time::Instant::now();
        let from_ver = self.current_version.clone();

        // Step 1: git pull
        let pull = Command::new("git")
            .args(["-C", &self.anos_dir, "pull", "origin", "main"])
            .output()
            .map_err(|e| format!("git pull failed: {}", e))?;

        if !pull.status.success() {
            return Err(format!(
                "git pull failed: {}",
                String::from_utf8_lossy(&pull.stderr).trim()
            ));
        }

        // Step 2: cargo build --release
        let build = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(format!("{}/anosd", self.anos_dir))
            .output()
            .map_err(|e| format!("cargo build failed: {}", e))?;

        if !build.status.success() {
            // ROLLBACK: git reset to previous commit
            let _ = Command::new("git")
                .args(["-C", &self.anos_dir, "reset", "--hard", "HEAD~1"])
                .output();

            return Err(format!(
                "Build failed, rolled back. Error: {}",
                String::from_utf8_lossy(&build.stderr)
                    .lines()
                    .last()
                    .unwrap_or("unknown")
            ));
        }

        // Step 3: build CLI too
        let _ = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(format!("{}/anos-cli", self.anos_dir))
            .output();

        let to_ver = Self::detect_current_version(&self.anos_dir);
        let duration = start.elapsed().as_secs_f64();

        Ok(UpgradeResult {
            success: true,
            from_version: from_ver,
            to_version: to_ver.clone(),
            message: format!(
                "Upgraded from {} → {}. Build took {:.1}s. Restart anosd to apply.",
                self.current_version, to_ver, duration
            ),
            duration_secs: duration,
        })
    }

    /// Restart the anosd daemon
    #[allow(dead_code)]
    pub fn restart_daemon() -> Result<String, String> {
        // Kill existing anosd
        let _ = Command::new("pkill")
            .args(["-f", "target/release/anosd"])
            .output();

        // Start new one
        let anos_dir = std::env::var("ANOS_DIR").unwrap_or_else(|_| {
            format!(
                "{}/.openclaw/workspace/anos",
                std::env::var("HOME").unwrap_or_default()
            )
        });

        let output = Command::new(format!("{}/anosd/target/release/anosd", anos_dir))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        match output {
            Ok(_child) => Ok("anosd restarted successfully".into()),
            Err(e) => Err(format!("Failed to start new anosd: {}", e)),
        }
    }

    /// Full upgrade: binary first, fallback to source
    #[allow(dead_code)]
    pub async fn upgrade(&self) -> Result<UpgradeResult, String> {
        // Try binary first
        if let Some((version, _)) = Self::check_updates() {
            if version != format!("v{}", self.current_version) {
                match Self::upgrade_binary(&version) {
                    Ok(msg) => {
                        return Ok(UpgradeResult {
                            success: true,
                            from_version: self.current_version.clone(),
                            to_version: version.clone(),
                            message: msg,
                            duration_secs: 0.0,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Binary upgrade failed ({}), falling back to source build",
                            e
                        );
                    }
                }
            }
        }

        // Fallback: source build
        self.upgrade_source()
    }

    /// Get upgrade status
    pub fn status(&self) -> String {
        format!(
            "Current: {} | Dir: {} | {}",
            self.current_version,
            self.anos_dir,
            if Self::btrfs_available() {
                "✅ Snapshot safety available"
            } else {
                "⚠️ No snapshot safety"
            }
        )
    }

    fn btrfs_available() -> bool {
        Command::new("which")
            .arg("btrfs")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_version() {
        let v = SelfUpgrade::detect_current_version(&format!(
            "{}/.openclaw/workspace/anos",
            std::env::var("HOME").unwrap_or_default()
        ));
        assert!(!v.is_empty());
        assert_ne!(v, "unknown");
    }
}

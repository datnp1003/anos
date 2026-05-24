//! Proactive Scheduler — autonomous periodic system health checks with alerts.
//!
//! Phase 6: Anos tự chạy checks định kỳ, cảnh báo khi vượt ngưỡng, không cần user trigger.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// A scheduled health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchCheck {
    pub id: String,
    pub name: String,
    pub description: String,
    pub interval_secs: u64,
    pub enabled: bool,
    pub threshold: Option<f64>,
    pub threshold_unit: Option<String>,
    pub last_run: Option<String>,
    pub last_value: Option<String>,
    pub alert_count: u64,
}

/// Result of a single check run
#[derive(Debug, Clone)]
struct CheckResult {
    #[allow(dead_code)]
    check_id: String,
    value: String,
    #[allow(dead_code)]
    numeric_value: Option<f64>,
    exceeded_threshold: bool,
    message: String,
}

/// Scheduler that manages all watches
pub struct Watcher {
    checks: Arc<RwLock<Vec<WatchCheck>>>,
}

impl Watcher {
    pub fn new() -> Self {
        let default_checks = vec![
            WatchCheck {
                id: "disk".into(),
                name: "Disk Usage".into(),
                description: "Alert when root disk exceeds threshold %".into(),
                interval_secs: 1800,
                enabled: true,
                threshold: Some(85.0),
                threshold_unit: Some("%".into()),
                last_run: None,
                last_value: None,
                alert_count: 0,
            },
            WatchCheck {
                id: "ram".into(),
                name: "Memory Usage".into(),
                description: "Alert when RAM usage exceeds threshold %".into(),
                interval_secs: 900,
                enabled: true,
                threshold: Some(90.0),
                threshold_unit: Some("%".into()),
                last_run: None,
                last_value: None,
                alert_count: 0,
            },
            WatchCheck {
                id: "updates".into(),
                name: "Package Updates".into(),
                description: "Alert when security updates are available".into(),
                interval_secs: 21600,
                enabled: true,
                threshold: None,
                threshold_unit: None,
                last_run: None,
                last_value: None,
                alert_count: 0,
            },
            WatchCheck {
                id: "load".into(),
                name: "System Load".into(),
                description: "Alert when load average exceeds CPU core count * 2".into(),
                interval_secs: 600,
                enabled: false,
                threshold: None,
                threshold_unit: None,
                last_run: None,
                last_value: None,
                alert_count: 0,
            },
            WatchCheck {
                id: "services".into(),
                name: "Critical Services".into(),
                description: "Alert when critical services are down".into(),
                interval_secs: 1800,
                enabled: false,
                threshold: None,
                threshold_unit: None,
                last_run: None,
                last_value: None,
                alert_count: 0,
            },
            WatchCheck {
                id: "security".into(),
                name: "Security Audit".into(),
                description: "Alert on failed SSH logins and firewall issues".into(),
                interval_secs: 3600,
                enabled: false,
                threshold: None,
                threshold_unit: None,
                last_run: None,
                last_value: None,
                alert_count: 0,
            },
        ];

        Self {
            checks: Arc::new(RwLock::new(default_checks)),
        }
    }

    /// Start the background scheduler
    pub async fn start(&self) {
        let checks = Arc::clone(&self.checks);

        tokio::spawn(async move {
            tracing::info!("👁️ Watcher started — monitoring system health");
            let mut tick = interval(Duration::from_secs(5));

            loop {
                tick.tick().await;
                let now = chrono::Utc::now();
                let list = checks.read().await;

                for check in list.iter() {
                    if !check.enabled {
                        continue;
                    }
                    if let Some(last) = &check.last_run {
                        if let Ok(last_ts) = chrono::DateTime::parse_from_rfc3339(last) {
                            let elapsed = (now - last_ts.with_timezone(&chrono::Utc)).num_seconds();
                            if elapsed < check.interval_secs as i64 {
                                continue;
                            }
                        }
                    }
                }

                // Clone enabled checks to check
                let enabled: Vec<WatchCheck> = list.iter().filter(|c| c.enabled).cloned().collect();
                drop(list);

                for check in &enabled {
                    let result = execute_check(check);
                    let exceeded = result.exceeded_threshold;
                    let mut list = checks.write().await;
                    if let Some(c) = list.iter_mut().find(|c| c.id == check.id) {
                        c.last_run = Some(now.to_rfc3339());
                        c.last_value = Some(result.value.clone());
                        if exceeded {
                            c.alert_count += 1;
                        }
                    }
                    drop(list);

                    if exceeded {
                        tracing::warn!(
                            "⚠️ Watcher: {} exceeded threshold — {}",
                            check.name,
                            result.message
                        );
                    } else {
                        tracing::debug!("✅ Watcher: {} OK — {}", check.name, result.message);
                    }
                }
            }
        });

        tracing::info!("Proactive scheduler started with {} checks", self.checks.read().await.len());
    }

    /// List all checks
    pub async fn list(&self) -> Vec<WatchCheck> {
        self.checks.read().await.clone()
    }

    /// Enable a check
    pub async fn enable(&self, id: &str) -> String {
        let mut list = self.checks.write().await;
        if let Some(c) = list.iter_mut().find(|c| c.id == id) {
            c.enabled = true;
            format!("✅ Watch '{}' enabled", c.name)
        } else {
            format!("❌ Check '{}' not found", id)
        }
    }

    /// Disable a check
    pub async fn disable(&self, id: &str) -> String {
        let mut list = self.checks.write().await;
        if let Some(c) = list.iter_mut().find(|c| c.id == id) {
            c.enabled = false;
            format!("🔕 Watch '{}' disabled", c.name)
        } else {
            format!("❌ Check '{}' not found", id)
        }
    }

    /// Get check summary
    pub async fn summary(&self) -> String {
        let list = self.checks.read().await;
        let enabled = list.iter().filter(|c| c.enabled).count();
        let total = list.len();
        let alerted = list.iter().filter(|c| c.alert_count > 0).count();
        format!(
            "👁️ Watcher: {} enabled / {} total ({}) | {} recent alerts",
            enabled, total,
            list.iter().filter(|c| c.enabled).map(|c| c.id.clone()).collect::<Vec<_>>().join(", "),
            alerted
        )
    }
}

fn execute_check(check: &WatchCheck) -> CheckResult {
    match check.id.as_str() {
        "disk" => check_disk(check),
        "ram" => check_ram(check),
        "updates" => check_updates(check),
        "load" => check_load(check),
        "services" => check_services(check),
        "security" => check_security(check),
        _ => CheckResult {
            check_id: check.id.clone(),
            value: "unknown check".into(),
            numeric_value: None,
            exceeded_threshold: false,
            message: "Unknown check type".into(),
        },
    }
}

fn check_disk(check: &WatchCheck) -> CheckResult {
    let threshold = check.threshold.unwrap_or(85.0);
    if let Ok(out) = Command::new("df")
        .args(["-h", "--output=pcent,target"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.ends_with(" /") && !line.contains("snap") {
                let pct: f64 = line
                    .split_whitespace()
                    .next()
                    .unwrap_or("0%")
                    .trim_end_matches('%')
                    .parse()
                    .unwrap_or(0.0);
                let exceeded = pct > threshold;
                return CheckResult {
                    check_id: check.id.clone(),
                    value: format!("{:.0}%", pct),
                    numeric_value: Some(pct),
                    exceeded_threshold: exceeded,
                    message: if exceeded {
                        format!("Disk {}% full (threshold {}%)", pct, threshold)
                    } else {
                        format!("Disk {:.0}% — OK", pct)
                    },
                };
            }
        }
    }
    CheckResult {
        check_id: check.id.clone(),
        value: "N/A".into(),
        numeric_value: None,
        exceeded_threshold: false,
        message: "Could not read disk".into(),
    }
}

fn check_ram(check: &WatchCheck) -> CheckResult {
    let threshold = check.threshold.unwrap_or(90.0);
    if let Ok(mem) = std::fs::read_to_string("/proc/meminfo") {
        let mut map: HashMap<&str, f64> = HashMap::new();
        for line in mem.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() >= 2 {
                if let Ok(v) = p[1].parse::<f64>() {
                    map.insert(p[0].trim_end_matches(':'), v);
                }
            }
        }
        let total = map.get("MemTotal").copied().unwrap_or(1.0);
        let available = map.get("MemAvailable").copied().unwrap_or(0.0);
        let used_pct = ((total - available) / total) * 100.0;
        let exceeded = used_pct > threshold;
        return CheckResult {
            check_id: check.id.clone(),
            value: format!("{:.1}%", used_pct),
            numeric_value: Some(used_pct),
            exceeded_threshold: exceeded,
            message: if exceeded {
                format!("RAM {:.1}% used (threshold {}%)", used_pct, threshold)
            } else {
                format!("RAM {:.1}% — OK", used_pct)
            },
        };
    }
    CheckResult {
        check_id: check.id.clone(),
        value: "N/A".into(),
        numeric_value: None,
        exceeded_threshold: false,
        message: "Could not read memory".into(),
    }
}

fn check_updates(check: &WatchCheck) -> CheckResult {
    if let Ok(out) = Command::new("apt")
        .args(["list", "--upgradable"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        let count = text.lines().filter(|l| !l.starts_with("Listing") && !l.is_empty()).count();
        let sec = text.lines().filter(|l| l.contains("-security")).count();
        let exceeded = sec > 0;
        return CheckResult {
            check_id: check.id.clone(),
            value: format!("{} total, {} security", count, sec),
            numeric_value: Some(count as f64),
            exceeded_threshold: exceeded,
            message: if exceeded {
                format!("{} security updates available", sec)
            } else {
                format!("{} updates (0 security) — OK", count)
            },
        };
    }
    CheckResult {
        check_id: check.id.clone(),
        value: "N/A".into(),
        numeric_value: None,
        exceeded_threshold: false,
        message: "Could not check updates".into(),
    }
}

fn check_load(check: &WatchCheck) -> CheckResult {
    if let Ok(raw) = std::fs::read_to_string("/proc/loadavg") {
        let load: f64 = raw
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0.0);

        let cpus: f64 = std::fs::read_to_string("/proc/cpuinfo")
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with("processor"))
            .count() as f64;

        let threshold = cpus * 2.0;
        let exceeded = load > threshold;
        return CheckResult {
            check_id: check.id.clone(),
            value: format!("{:.2}", load),
            numeric_value: Some(load),
            exceeded_threshold: exceeded,
            message: if exceeded {
                format!("Load {:.2} exceeds {} CPU cores ×2 = {:.1}", load, cpus, threshold)
            } else {
                format!("Load {:.2} ({} cores) — OK", load, cpus)
            },
        };
    }
    CheckResult {
        check_id: check.id.clone(),
        value: "N/A".into(),
        numeric_value: None,
        exceeded_threshold: false,
        message: "Could not read load".into(),
    }
}

fn check_services(check: &WatchCheck) -> CheckResult {
    let critical = vec!["sshd", "nginx", "docker"];
    let mut down = vec![];

    for svc in &critical {
        if let Ok(out) = Command::new("systemctl")
            .args(["is-active", "--quiet", &format!("{}.service", svc)])
            .output()
        {
            if !out.status.success() {
                down.push(svc.to_string());
            }
        }
    }

    let exceeded = !down.is_empty();
    CheckResult {
        check_id: check.id.clone(),
        value: if exceeded { down.join(", ") } else { "all OK".into() },
        numeric_value: Some(down.len() as f64),
        exceeded_threshold: exceeded,
        message: if exceeded {
            format!("Services down: {}", down.join(", "))
        } else {
            "All critical services running".into()
        },
    }
}

fn check_security(check: &WatchCheck) -> CheckResult {
    let mut failed = 0usize;
    let mut bans = 0usize;

    // Check failed SSH logins
    if let Ok(out) = Command::new("grep")
        .args(["-c", "Failed password", "/var/log/auth.log"])
        .output()
    {
        if let Ok(s) = String::from_utf8(out.stdout) {
            failed = s.trim().parse().unwrap_or(0);
        }
    }

    // Check fail2ban
    if let Ok(out) = Command::new("fail2ban-client")
        .args(["status", "sshd"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("Currently banned:") {
                bans = line
                    .split_whitespace()
                    .last()
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
            }
        }
    }

    let exceeded = failed > 10 || bans > 0;
    CheckResult {
        check_id: check.id.clone(),
        value: format!("{} failed, {} banned", failed, bans),
        numeric_value: Some(failed as f64),
        exceeded_threshold: exceeded,
        message: if exceeded {
            format!("Security alert: {} failed logins, {} banned IPs", failed, bans)
        } else {
            format!("Security OK ({} failed logins)", failed)
        },
    }
}

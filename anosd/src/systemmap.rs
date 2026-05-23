use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct SystemSection {
    pub name: &'static str,
    pub priority: u8,
    pub content: String,
}

pub struct SystemMap;

impl SystemMap {
    pub fn snapshot() -> Result<String> {
        let mut s = String::from("# System State\n\n");
        s.push_str(&format!("Host: {} | Kernel: {} | Uptime: {}\n",
            std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default().trim(),
            std::fs::read_to_string("/proc/version").unwrap_or_default().split_whitespace().take(3).collect::<Vec<_>>().join(" "),
            Self::uptime()));
        s.push_str(&format!("**CPU**: {:.1}% | **RAM**: {} | **Disk**: {}\n",
            Self::cpu_usage(), Self::memory(), Self::disk()));
        s.push_str(&format!("**Top**: {}\n", Self::top_processes()));
        s.push_str(&format!("**Pkgs**: {} installed, {} upgradable | **Svcs**: {}\n",
            Self::pkg_count(), Self::upgradable_count(), Self::svc_count()));
        Ok(s)
    }

    pub fn build(hint: Option<&str>, _: usize) -> Result<String> {
        let _hint = hint.unwrap_or("general").to_lowercase();
        Self::snapshot()
    }

    fn uptime() -> String {
        let raw = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
        let secs: f64 = raw.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let d = secs as u32 / 86400; let h = (secs as u32 % 86400) / 3600; let m = (secs as u32 % 3600) / 60;
        if d > 0 { format!("{}d {}h {}m", d, h, m) } else { format!("{}h {}m", h, m) }
    }

    fn cpu_usage() -> f32 {
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        stat.lines().find(|l| l.starts_with("cpu ")).map(|l| {
            let fields: Vec<u64> = l.split_whitespace().skip(1).take(4).filter_map(|s| s.parse().ok()).collect();
            if fields.len() >= 4 { let total: u64 = fields.iter().sum(); let idle = fields[3];
                if total > 0 { ((total-idle) as f32 / total as f32) * 100.0 } else { 0.0 } } else { 0.0 }
        }).unwrap_or(0.0)
    }

    fn memory() -> String {
        let m = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mut map: HashMap<&str, f64> = HashMap::new();
        for l in m.lines() { let p: Vec<&str> = l.split_whitespace().collect();
            if p.len() >= 2 { if let Ok(v) = p[1].parse::<f64>() { map.insert(p[0].trim_end_matches(':'), v); } } }
        let gb = |kb: f64| kb / 1024.0 / 1024.0;
        let total = map.get("MemTotal").copied().unwrap_or(0.0);
        let avail = map.get("MemAvailable").copied().unwrap_or(0.0);
        format!("{:.1}G/{:.1}G used ({:.1}G free)", gb(total-avail), gb(total), gb(avail))
    }

    fn disk() -> String {
        Command::new("df").args(["-h","--output=size,used,avail,pcent,target"]).output().ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout).lines()
                    .find(|l| l.ends_with(" /") && !l.contains("/snap")).map(|l| l.to_string())
            })
            .map(|l| { let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() >= 5 { format!("{}/{} used ({} free, {}% full)", p[1], p[0], p[2], p[3].trim_end_matches('%')) } else { l }
            }).unwrap_or_else(|| "unknown".into())
    }

    fn top_processes() -> String {
        Command::new("ps").args(["aux","--sort=-%cpu","--no-headers"]).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().take(5)
                .filter_map(|l| { let p: Vec<&str> = l.split_whitespace().collect();
                    if p.len() >= 11 { Some(format!("{}(PID:{},{}%CPU,{}M)", p[10].rsplit('/').next().unwrap_or(p[10]), p[1], p[2],
                        (p[5].parse::<f64>().unwrap_or(0.0)/1024.0) as u32))
                    } else { None }
                }).collect::<Vec<_>>().join(" | "))
            .unwrap_or_default()
    }

    fn pkg_count() -> usize {
        Command::new("dpkg").args(["--list"]).output().map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.starts_with("ii")).count()).unwrap_or(0)
    }

    fn upgradable_count() -> usize {
        Command::new("apt").args(["list","--upgradable"]).output().map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.starts_with("Listing") && !l.is_empty()).count()).unwrap_or(0)
    }

    fn svc_count() -> usize {
        Command::new("systemctl").args(["list-units","--type=service","--state=running","--no-legend"]).output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().count()).unwrap_or(0)
    }
}

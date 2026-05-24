//! Intent Classifier — classifies user messages into system intents with confidence scoring.
//!
//! Phase 2: production-grade classifier with keyword + pattern matching and confidence scoring.

use serde::Serialize;

/// Recognized intent categories
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Intent {
    /// Package management: install, remove, update, search
    Package,
    /// System diagnostics: CPU, RAM, performance, errors
    System,
    /// Network: interfaces, ports, DNS, ping
    Network,
    /// Filesystem: disk, files, directories
    Filesystem,
    /// Process management: kill, list, info
    Process,
    /// Kernel tuning
    Kernel,
    /// Security: firewall, permissions
    Security,
    /// Desktop / GUI
    Gui,
    /// Self-upgrade
    SelfUpgrade,
    /// General chat / no specific intent
    Chat,
}

/// Classification result with confidence
#[derive(Debug, Clone, Serialize)]
pub struct Classification {
    pub intent: Intent,
    pub confidence: f32,
    /// Skill name to load
    pub skill_name: Option<String>,
    /// Short description for the audit log
    pub summary: String,
}

pub struct IntentClassifier;

impl IntentClassifier {
    pub fn classify(msg: &str) -> Classification {
        let lower = msg.to_lowercase();

        // Ordered by specificity — first match wins
        let rules: &[(&[&str], Intent, &str, f32)] = &[
            // Package
            (
                &[
                    "cài",
                    "install",
                    "setup",
                    "gỡ",
                    "xóa",
                    "remove",
                    "update",
                    "upgrade",
                    "nâng cấp",
                    "package",
                    "apt",
                    "dpkg",
                    "snap",
                    "flatpak",
                ],
                Intent::Package,
                "package",
                0.92,
            ),
            // Kernel
            (
                &["kernel", "sysctl", "modprobe", "module", "boot"],
                Intent::Kernel,
                "kernel",
                0.90,
            ),
            // Security
            (
                &[
                    "bảo mật",
                    "security",
                    "firewall",
                    "ufw",
                    "iptables",
                    "fail2ban",
                    "apparmor",
                    "selinux",
                    "audit",
                ],
                Intent::Security,
                "security",
                0.90,
            ),
            // Process
            (
                &[
                    "process",
                    "kill",
                    "tiến trình",
                    "pid",
                    "nice",
                    "renice",
                    "top",
                    "htop",
                ],
                Intent::Process,
                "process",
                0.88,
            ),
            // Network
            (
                &[
                    "mạng",
                    "network",
                    "port",
                    "dns",
                    "internet",
                    "ping",
                    "route",
                    "interface",
                    "ip addr",
                    "listen",
                    "socket",
                ],
                Intent::Network,
                "network",
                0.88,
            ),
            // Filesystem
            (
                &[
                    "disk",
                    "ổ cứng",
                    "dọn",
                    "clean",
                    "btrfs",
                    "file",
                    "folder",
                    "thư mục",
                    "đọc file",
                    "ghi file",
                    "mkdir",
                    "ls ",
                    "du ",
                    "df ",
                    "mount",
                    "inode",
                ],
                Intent::Filesystem,
                "filesystem",
                0.88,
            ),
            // Gui
            (
                &[
                    "gui", "desktop", "hyprland", "sway", "gnome", "kde", "wayland", "x11",
                ],
                Intent::Gui,
                "gui",
                0.85,
            ),
            // Self-upgrade
            (
                &[
                    "nâng cấp os",
                    "self upgrade",
                    "self-upgrade",
                    "upgrade anos",
                    "nâng cấp anos",
                ],
                Intent::SelfUpgrade,
                "self-upgrade",
                0.95,
            ),
            // System diagnostics — must come AFTER specific intents
            (
                &[
                    "chậm",
                    "lag",
                    "cpu",
                    "ram",
                    "memory",
                    "lỗi",
                    "error",
                    "crash",
                    "nặng",
                    "nóng",
                    "temp",
                    "sao máy",
                    "kiểm tra",
                    "check",
                    "status",
                    "health",
                    "tình trạng",
                    "trạng thái",
                    "system",
                    "uptime",
                    "load",
                ],
                Intent::System,
                "system",
                0.80,
            ),
        ];

        for (keywords, intent, skill, base_conf) in rules {
            let matches: usize = keywords.iter().filter(|kw| lower.contains(**kw)).count();
            if matches > 0 {
                let boost = (matches as f32 * 0.03).min(0.08);
                let conf = (base_conf + boost).min(0.99);
                return Classification {
                    intent: intent.clone(),
                    confidence: conf,
                    skill_name: Some(skill.to_string()),
                    summary: format!("{:?} intent (confidence: {:.0}%)", intent, conf * 100.0),
                };
            }
        }

        Classification {
            intent: Intent::Chat,
            confidence: 0.5,
            skill_name: None,
            summary: "Chat (no specific intent)".to_string(),
        }
    }

    /// Return the skill name to inject for this intent
    #[allow(dead_code)]
    pub fn skill_for_intent(intent: &Intent) -> Option<&'static str> {
        match intent {
            Intent::Package => Some("package"),
            Intent::System => Some("system"),
            Intent::Network => Some("network"),
            Intent::Filesystem => Some("filesystem"),
            Intent::Process => Some("process"),
            Intent::Kernel => Some("kernel"),
            Intent::Security => Some("security"),
            Intent::Gui => Some("gui"),
            Intent::SelfUpgrade => Some("self-upgrade"),
            Intent::Chat => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_intent() {
        let c = IntentClassifier::classify("Install neovim");
        assert_eq!(c.intent, Intent::Package);
        assert!(c.confidence > 0.85);
        assert_eq!(c.skill_name, Some("package".into()));
    }

    #[test]
    fn test_system_diag() {
        let c = IntentClassifier::classify("Check system health");
        assert_eq!(c.intent, Intent::System);
        assert!(c.confidence > 0.75);
    }

    #[test]
    fn test_network() {
        let c = IntentClassifier::classify("Which ports are open?");
        assert_eq!(c.intent, Intent::Network);
        assert!(c.confidence > 0.80);
    }

    #[test]
    fn test_chat() {
        let c = IntentClassifier::classify("Hello there");
        assert_eq!(c.intent, Intent::Chat);
        assert!(c.confidence < 0.8);
    }

    #[test]
    fn test_kill_process() {
        let c = IntentClassifier::classify("Kill process node");
        assert_eq!(c.intent, Intent::Process);
    }

    #[test]
    fn test_filesystem() {
        let c = IntentClassifier::classify("How much disk space is free?");
        assert_eq!(c.intent, Intent::Filesystem);
    }
}

use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
pub enum Permission {
    ReadOnly = 0,
    Safe = 1,
    Confirm = 2,
    Dangerous = 3,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

#[async_trait]
pub trait SystemTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    #[allow(dead_code)]
    fn permission(&self) -> Permission;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, params: &serde_json::Value, require_confirm: bool) -> ToolResult;
}

pub fn run_cmd(cmd: &str, args: &[&str]) -> (i32, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            if o.status.code().is_none() {
                // Killed by signal
                return (-1, err);
            }
            (
                o.status.code().unwrap_or(-1),
                if out.is_empty() { err } else { out },
            )
        }
        Err(e) => {
            // Graceful message for "command not found" (os error 2)
            if e.raw_os_error() == Some(2) {
                (
                    -1,
                    format!(
                        "⚠️ {}: command not found on this system. Try: sudo apt install {}",
                        cmd, cmd
                    ),
                )
            } else {
                (-1, e.to_string())
            }
        }
    }
}

// ── Package ──
pub struct PackageTool;

pub fn detect_pkg_manager() -> &'static str {
    // Detect by trying to run each manager with --version
    // Avoids 'which' dependency (missing on minimal/sublinux systems)
    for (cmd, args) in &[
        // Debian/Ubuntu — try apt-get first (more reliably installed)
        ("apt-get", &["--version"][..]),
        ("apt", &["--version"][..]),
        // Alpine
        ("apk", &["--version"][..]),
        // Arch
        ("pacman", &["--version"][..]),
        // Fedora/RHEL 8+
        ("dnf", &["--version"][..]),
        // RHEL 7/CentOS 7
        ("yum", &["--version"][..]),
        // openSUSE
        ("zypper", &["--version"][..]),
    ] {
        if let Ok(o) = Command::new(cmd).args(*args).output() {
            if o.status.success() {
                match *cmd {
                    "apt-get" => return "apt",
                    other => return other,
                }
            }
        }
    }
    "apt" // fallback — will fail gracefully via run_cmd
}
impl PackageTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for PackageTool {
    fn name(&self) -> &str {
        "package"
    }
    fn description(&self) -> &str {
        "Manage packages: search, install, remove, update, info, list_upgradable"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "package".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["search","install","remove","update","info","list_upgradable"]},"package":{"type":"string"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let pm = detect_pkg_manager();
        let action = params["action"].as_str().unwrap_or("list");
        let pkg = params["package"].as_str().unwrap_or("");
        match action {
            "search" => {
                let (_, o) = match pm {
                    "apk" => run_cmd("apk", &["search", pkg]),
                    "pacman" => run_cmd("pacman", &["-Ss", pkg]),
                    "dnf" | "yum" => run_cmd(pm, &["search", pkg]),
                    "zypper" => run_cmd("zypper", &["search", pkg]),
                    _ => run_cmd("apt", &["search", pkg]),
                };
                ToolResult {
                    success: true,
                    output: o.lines().take(15).collect::<Vec<_>>().join("\n"),
                    error: None,
                }
            }
            "info" => {
                let (c, o) = match pm {
                    "apk" => run_cmd("apk", &["info", "-a", pkg]),
                    "pacman" => run_cmd("pacman", &["-Qi", pkg]),
                    "dnf" | "yum" => run_cmd("rpm", &["-qi", pkg]),
                    _ => run_cmd("dpkg", &["-s", pkg]),
                };
                ToolResult {
                    success: c == 0,
                    output: o,
                    error: if c != 0 {
                        Some(format!("Not installed: {}", pkg))
                    } else {
                        None
                    },
                }
            }
            "list_upgradable" => {
                let (_, o) = match pm {
                    "apk" => run_cmd("apk", &["list", "-u"]),
                    "pacman" => run_cmd("checkupdates", &[]),
                    "dnf" => run_cmd("dnf", &["check-update", "-q"]),
                    "yum" => run_cmd("yum", &["check-update", "-q"]),
                    "zypper" => run_cmd("zypper", &["list-updates"]),
                    _ => run_cmd("apt", &["list", "--upgradable"]),
                };
                let n = o.lines().filter(|l| !l.starts_with("Listing")).count();
                ToolResult {
                    success: true,
                    output: format!("{} upgradable packages", n),
                    error: None,
                }
            }
            "install" if pkg.is_empty() => ToolResult {
                success: false,
                output: String::new(),
                error: Some("Package name required".into()),
            },
            "install" if confirm => {
                let (c, o) = match pm {
                    "apk" => run_cmd("apk", &["add", pkg]),
                    "pacman" => run_cmd("pacman", &["-S", "--noconfirm", pkg]),
                    "dnf" => run_cmd("dnf", &["install", "-y", "-q", pkg]),
                    "yum" => run_cmd("yum", &["install", "-y", "-q", pkg]),
                    "zypper" => run_cmd("zypper", &["install", "-y", pkg]),
                    _ => run_cmd("apt", &["install", "-y", "-qq", pkg]),
                };
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Installed: {}", pkg)
                    } else {
                        o.clone()
                    },
                    error: if c != 0 { Some(o) } else { None },
                }
            }
            "remove" if confirm => {
                let (c, o) = match pm {
                    "apk" => run_cmd("apk", &["del", pkg]),
                    "pacman" => run_cmd("pacman", &["-R", "--noconfirm", pkg]),
                    "dnf" => run_cmd("dnf", &["remove", "-y", "-q", pkg]),
                    "yum" => run_cmd("yum", &["remove", "-y", "-q", pkg]),
                    "zypper" => run_cmd("zypper", &["remove", "-y", pkg]),
                    _ => run_cmd("apt", &["remove", "-y", "-qq", pkg]),
                };
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Removed: {}", pkg)
                    } else {
                        o.clone()
                    },
                    error: if c != 0 { Some(o) } else { None },
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Action '{}' needs confirmation. Reply 'yes'.",
                    action
                )),
            },
        }
    }
}

// ── Process ──
pub struct ProcessTool;
impl ProcessTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for ProcessTool {
    fn name(&self) -> &str {
        "process"
    }
    fn description(&self) -> &str {
        "Manage processes: list, kill by PID or name, info"
    }
    fn permission(&self) -> Permission {
        Permission::Dangerous
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "process".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","kill","kill_by_name","info"]},"pid":{"type":"integer"},"name":{"type":"string"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        match action {
            "list" => {
                let (_, o) = run_cmd("ps", &["aux", "--sort=-%cpu", "--no-headers"]);
                ToolResult {
                    success: true,
                    output: o.lines().take(10).collect::<Vec<_>>().join("\n"),
                    error: None,
                }
            }
            "info" => {
                let pid = params["pid"].as_u64().unwrap_or(0);
                if pid == 0 {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("PID required".into()),
                    };
                }
                let (_, o) = run_cmd(
                    "ps",
                    &[
                        "-p",
                        &pid.to_string(),
                        "-o",
                        "pid,user,%cpu,%mem,comm",
                        "--no-headers",
                    ],
                );
                ToolResult {
                    success: true,
                    output: o,
                    error: None,
                }
            }
            "kill" if confirm => {
                let pid = params["pid"].as_u64().unwrap_or(0);
                if pid == 0 {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("PID required".into()),
                    };
                }
                let (c, _) = run_cmd("kill", &[&pid.to_string()]);
                if c != 0 {
                    let (c2, _) = run_cmd("kill", &["-9", &pid.to_string()]);
                    ToolResult {
                        success: c2 == 0,
                        output: format!("Force killed PID {}", pid),
                        error: None,
                    }
                } else {
                    ToolResult {
                        success: true,
                        output: format!("Killed PID {}", pid),
                        error: None,
                    }
                }
            }
            "kill_by_name" if confirm => {
                let name = params["name"].as_str().unwrap_or("");
                if name.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("Name required".into()),
                    };
                }
                let (c, _) = run_cmd("pkill", &[name]);
                ToolResult {
                    success: c == 0,
                    output: format!("Killed '{}' processes", name),
                    error: None,
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ Needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Service ──
pub struct ServiceTool;
impl ServiceTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for ServiceTool {
    fn name(&self) -> &str {
        "service"
    }
    fn description(&self) -> &str {
        "Manage systemd services: list, status, start, stop, restart, logs"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "service".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","status","start","stop","restart","logs"]},"name":{"type":"string"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        let name = params["name"].as_str().unwrap_or("");
        match action {
            "list" => {
                let (_, o) = run_cmd(
                    "systemctl",
                    &[
                        "list-units",
                        "--type=service",
                        "--state=running",
                        "--no-legend",
                    ],
                );
                ToolResult {
                    success: true,
                    output: o.lines().take(15).collect::<Vec<_>>().join("\n"),
                    error: None,
                }
            }
            "status" => {
                let (_, o) = run_cmd("systemctl", &["status", &format!("{}.service", name)]);
                ToolResult {
                    success: true,
                    output: o,
                    error: None,
                }
            }
            "logs" => {
                let (_, o) = run_cmd(
                    "journalctl",
                    &[
                        "-u",
                        &format!("{}.service", name),
                        "--lines=20",
                        "--no-pager",
                    ],
                );
                ToolResult {
                    success: true,
                    output: o,
                    error: None,
                }
            }
            a @ ("start" | "stop" | "restart") if confirm => {
                let (c, o) = run_cmd("systemctl", &[a, &format!("{}.service", name)]);
                ToolResult {
                    success: c == 0,
                    output: format!("{} {}: {}", a, name, if c == 0 { "✅" } else { "❌" }),
                    error: if c != 0 { Some(o) } else { None },
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ Needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Filesystem ──
pub struct FileSystemTool;
impl FileSystemTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for FileSystemTool {
    fn name(&self) -> &str {
        "filesystem"
    }
    fn description(&self) -> &str {
        "Inspect and manage files: list, read, find, disk_usage, mkdir, write"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "filesystem".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","read","find","disk_usage","mkdir","write"]},"path":{"type":"string"},"pattern":{"type":"string"},"content":{"type":"string"},"limit":{"type":"integer"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        let path = params["path"].as_str().unwrap_or(".");
        let limit = params["limit"].as_u64().unwrap_or(80).min(300) as usize;
        match action {
            "list" => match std::fs::read_dir(path) {
                Ok(entries) => {
                    let mut rows = Vec::new();
                    for e in entries.flatten().take(limit) {
                        let meta = e.metadata().ok();
                        let kind = if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                            "dir"
                        } else {
                            "file"
                        };
                        let size = meta.map(|m| m.len()).unwrap_or(0);
                        rows.push(format!(
                            "{}\t{}\t{}",
                            kind,
                            size,
                            e.file_name().to_string_lossy()
                        ));
                    }
                    ToolResult {
                        success: true,
                        output: rows.join("\n"),
                        error: None,
                    }
                }
                Err(e) => ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                },
            },
            "read" => match std::fs::read_to_string(path) {
                Ok(text) => ToolResult {
                    success: true,
                    output: text.lines().take(limit).collect::<Vec<_>>().join("\n"),
                    error: None,
                },
                Err(e) => ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                },
            },
            "find" => {
                let pattern = params["pattern"].as_str().unwrap_or("");
                if pattern.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("pattern required".into()),
                    };
                }
                let (_, out) = run_cmd("find", &[path, "-iname", pattern, "-maxdepth", "5"]);
                ToolResult {
                    success: true,
                    output: out.lines().take(limit).collect::<Vec<_>>().join("\n"),
                    error: None,
                }
            }
            "disk_usage" => {
                let (_, out) = run_cmd("du", &["-sh", path]);
                ToolResult {
                    success: true,
                    output: out,
                    error: None,
                }
            }
            "mkdir" if confirm => match std::fs::create_dir_all(path) {
                Ok(_) => ToolResult {
                    success: true,
                    output: format!("✅ Created directory: {}", path),
                    error: None,
                },
                Err(e) => ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                },
            },
            "write" if confirm => {
                let content = params["content"].as_str().unwrap_or("");
                match std::fs::write(path, content) {
                    Ok(_) => ToolResult {
                        success: true,
                        output: format!("✅ Wrote {} bytes to {}", content.len(), path),
                        error: None,
                    },
                    Err(e) => ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(e.to_string()),
                    },
                }
            }
            "mkdir" | "write" => ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "⚠️ Filesystem write operation needs confirmation. Reply 'yes'.".into(),
                ),
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unknown filesystem action: {}", action)),
            },
        }
    }
}

// ── Network ──
pub struct NetworkTool;
impl NetworkTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for NetworkTool {
    fn name(&self) -> &str {
        "network"
    }
    fn description(&self) -> &str {
        "Inspect network: interfaces, listening ports, routes, ping, dns_lookup"
    }
    fn permission(&self) -> Permission {
        Permission::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "network".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["interfaces","listening_ports","routes","ping","dns_lookup"]},"host":{"type":"string"},"port":{"type":"integer"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, _confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("interfaces");
        match action {
            "interfaces" => {
                let (_, out) = run_cmd("ip", &["-brief", "addr"]);
                ToolResult {
                    success: true,
                    output: out,
                    error: None,
                }
            }
            "listening_ports" => {
                let (_, out) = run_cmd("ss", &["-tulpen"]);
                ToolResult {
                    success: true,
                    output: out.lines().take(80).collect::<Vec<_>>().join("\n"),
                    error: None,
                }
            }
            "routes" => {
                let (_, out) = run_cmd("ip", &["route"]);
                ToolResult {
                    success: true,
                    output: out,
                    error: None,
                }
            }
            "ping" => {
                let host = params["host"].as_str().unwrap_or("");
                if host.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("host required".into()),
                    };
                }
                let (code, out) = run_cmd("ping", &["-c", "4", host]);
                ToolResult {
                    success: code == 0,
                    output: out,
                    error: if code != 0 {
                        Some("ping failed".into())
                    } else {
                        None
                    },
                }
            }
            "dns_lookup" => {
                let host = params["host"].as_str().unwrap_or("");
                if host.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("host required".into()),
                    };
                }
                let (code, out) = run_cmd("getent", &["hosts", host]);
                ToolResult {
                    success: code == 0,
                    output: out,
                    error: if code != 0 {
                        Some("DNS lookup failed".into())
                    } else {
                        None
                    },
                }
            }
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unknown network action: {}", action)),
            },
        }
    }
}

// ── User ──
pub struct UserTool;
impl UserTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for UserTool {
    fn name(&self) -> &str {
        "user"
    }
    fn description(&self) -> &str {
        "Manage users and groups: list, info, create, delete, modify, password, list_groups, group_info"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "user".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","info","create","delete","modify","password","list_groups","group_info"]},"username":{"type":"string"},"groupname":{"type":"string"},"shell":{"type":"string"},"home":{"type":"string"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        match action {
            "list" => {
                let (_, out) = run_cmd("getent", &["passwd"]);
                let users: Vec<&str> = out
                    .lines()
                    .filter_map(|l| {
                        let p: Vec<&str> = l.split(':').collect();
                        if p.len() >= 7 && p[2].parse::<u32>().unwrap_or(0) >= 1000 {
                            Some(p[0])
                        } else {
                            None
                        }
                    })
                    .collect();
                ToolResult {
                    success: true,
                    output: users.join("\n"),
                    error: None,
                }
            }
            "info" => {
                let user = params["username"].as_str().unwrap_or("");
                if user.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("username required".into()),
                    };
                }
                let (c, out) = run_cmd("id", &[user]);
                ToolResult {
                    success: c == 0,
                    output: out,
                    error: if c != 0 {
                        Some(format!("User '{}' not found", user))
                    } else {
                        None
                    },
                }
            }
            "list_groups" => {
                let (_, out) = run_cmd("getent", &["group"]);
                let groups: Vec<&str> = out
                    .lines()
                    .filter_map(|l| {
                        let p: Vec<&str> = l.split(':').collect();
                        if p.len() >= 4 && p[2].parse::<u32>().unwrap_or(0) >= 1000 {
                            Some(p[0])
                        } else {
                            None
                        }
                    })
                    .collect();
                ToolResult {
                    success: true,
                    output: groups.join("\n"),
                    error: None,
                }
            }
            "group_info" => {
                let group = params["groupname"].as_str().unwrap_or("");
                if group.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("groupname required".into()),
                    };
                }
                let (c, out) = run_cmd("getent", &["group", group]);
                ToolResult {
                    success: c == 0,
                    output: out,
                    error: if c != 0 {
                        Some(format!("Group '{}' not found", group))
                    } else {
                        None
                    },
                }
            }
            "create" if confirm => {
                let user = params["username"].as_str().unwrap_or("");
                let shell = params["shell"].as_str().unwrap_or("/bin/bash");
                if user.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("username required".into()),
                    };
                }
                let (c, out) = run_cmd("useradd", &["-m", "-s", shell, user]);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Created user: {}", user)
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "delete" if confirm => {
                let user = params["username"].as_str().unwrap_or("");
                if user.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("username required".into()),
                    };
                }
                let (c, out) = run_cmd("userdel", &["-r", user]);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Deleted user: {}", user)
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "modify" if confirm => {
                let user = params["username"].as_str().unwrap_or("");
                let shell = params["shell"].as_str();
                let home = params["home"].as_str();
                if user.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("username required".into()),
                    };
                }
                let mut args: Vec<&str> = vec![];
                let mut changes = Vec::new();
                if let Some(s) = shell {
                    args.extend_from_slice(&["-s", s]);
                    changes.push(format!("shell={}", s));
                }
                if let Some(h) = home {
                    args.extend_from_slice(&["-d", h]);
                    changes.push(format!("home={}", h));
                }
                if changes.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("Nothing to modify (use shell= or home=)".into()),
                    };
                }
                args.push(user);
                let (c, out) = run_cmd("usermod", &args);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Modified user {}: {}", user, changes.join(", "))
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "password" if confirm => {
                let user = params["username"].as_str().unwrap_or("");
                if user.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("username required".into()),
                    };
                }
                // Generate a random password and set it
                let (_, pw_out) = run_cmd("openssl", &["rand", "-base64", "12"]);
                let pw = pw_out.trim().to_string();
                if pw.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("Failed to generate password".into()),
                    };
                }
                let (c, out) = run_cmd("chpasswd", &[]);
                if c != 0 || out.contains("not found") {
                    // Fallback: use passwd --stdin
                    let echo_cmd = format!("echo '{}:{}' | chpasswd", user, pw);
                    let (c2, out2) = run_cmd("bash", &["-c", &echo_cmd]);
                    ToolResult {
                        success: c2 == 0,
                        output: if c2 == 0 {
                            format!("✅ Password set for {} (temp: {})", user, pw)
                        } else {
                            out2.clone()
                        },
                        error: if c2 != 0 { Some(out2) } else { None },
                    }
                } else {
                    let echo_cmd2 = format!("echo '{}:{}' | chpasswd", user, pw);
                    let (c3, out3) = run_cmd("bash", &["-c", &echo_cmd2]);
                    ToolResult {
                        success: c3 == 0,
                        output: if c3 == 0 {
                            format!("✅ Password set for {} (temp: {})", user, pw)
                        } else {
                            out3.clone()
                        },
                        error: if c3 != 0 { Some(out3) } else { None },
                    }
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ User operation needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Cron ──
pub struct CronTool;
impl CronTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }
    fn description(&self) -> &str {
        "Manage scheduled tasks: list, add, remove, list_timers (crontab + systemd timers)"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "cron".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","add","remove","list_timers"]},"schedule":{"type":"string","description":"Cron expression e.g. '0 4 * * *'"},"command":{"type":"string","description":"Command to run"},"comment":{"type":"string","description":"Comment for the cron entry"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        match action {
            "list" => {
                // Show current user's crontab
                let (c, out) = run_cmd("crontab", &["-l"]);
                if c != 0 {
                    ToolResult {
                        success: true,
                        output: "No crontab configured for current user".into(),
                        error: None,
                    }
                } else {
                    ToolResult {
                        success: true,
                        output: out,
                        error: None,
                    }
                }
            }
            "list_timers" => {
                let (_, out) = run_cmd("systemctl", &["list-timers", "--no-pager", "--no-legend"]);
                ToolResult {
                    success: true,
                    output: out.lines().take(20).collect::<Vec<_>>().join("\n"),
                    error: None,
                }
            }
            "add" if confirm => {
                let schedule = params["schedule"].as_str().unwrap_or("");
                let command = params["command"].as_str().unwrap_or("");
                let comment = params["comment"].as_str().unwrap_or("");
                if schedule.is_empty() || command.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("schedule and command required".into()),
                    };
                }
                // Get existing crontab
                let (_, existing) = run_cmd("crontab", &["-l"]);
                let mut new_cron = if existing.is_empty() || existing.contains("no crontab") {
                    String::new()
                } else {
                    existing.trim().to_string()
                };
                if !new_cron.is_empty() && !new_cron.ends_with('\n') {
                    new_cron.push('\n');
                }
                if !comment.is_empty() {
                    new_cron.push_str(&format!("# {}\n", comment));
                }
                new_cron.push_str(&format!("{} {}\n", schedule, command));
                let tmp = std::env::temp_dir().join(format!("anos-cron-{}", std::process::id()));
                if std::fs::write(&tmp, &new_cron).is_err() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("Failed to write temp crontab".into()),
                    };
                }
                let tmp_str = tmp.to_string_lossy().to_string();
                let (c, out) = run_cmd("crontab", &[&tmp_str]);
                let _ = std::fs::remove_file(&tmp);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Added cron: {} {}", schedule, command)
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "remove" if confirm => {
                let command = params["command"].as_str().unwrap_or("");
                if command.is_empty() {
                    // Remove entire crontab
                    let (c, out) = run_cmd("crontab", &["-r"]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            "✅ Removed all cron jobs".into()
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                } else {
                    // Remove lines matching command
                    let (_, existing) = run_cmd("crontab", &["-l"]);
                    let filtered: String = existing
                        .lines()
                        .filter(|l| !l.contains(command))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let tmp =
                        std::env::temp_dir().join(format!("anos-cron-{}", std::process::id()));
                    if std::fs::write(&tmp, &filtered).is_err() {
                        return ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some("Failed to write temp crontab".into()),
                        };
                    }
                    let tmp_str = tmp.to_string_lossy().to_string();
                    let (c, out) = run_cmd("crontab", &[&tmp_str]);
                    let _ = std::fs::remove_file(&tmp);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Removed cron jobs matching: {}", command)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ Cron modification needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Log ──
pub struct LogTool;
impl LogTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for LogTool {
    fn name(&self) -> &str {
        "log"
    }
    fn description(&self) -> &str {
        "View and inspect logs: journalctl, tail, list_logs, logrotate_status"
    }
    fn permission(&self) -> Permission {
        Permission::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "log".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["journalctl","tail","list_logs","logrotate_status"]},"service":{"type":"string"},"file":{"type":"string"},"lines":{"type":"integer","default":50}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, _confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list_logs");
        let lines = params["lines"].as_u64().unwrap_or(50).min(200) as usize;
        match action {
            "list_logs" => {
                let (_, out) = run_cmd(
                    "find",
                    &["/var/log", "-type", "f", "-name", "*.log", "-maxdepth", "2"],
                );
                let (_, out2) = run_cmd(
                    "find",
                    &["/var/log", "-type", "f", "-name", "*.gz", "-maxdepth", "2"],
                );
                ToolResult {
                    success: true,
                    output: format!(
                        "=== Log files ===\n{}\n\n=== Rotated (gzipped) ===\n{}",
                        out.lines().take(20).collect::<Vec<_>>().join("\n"),
                        out2.lines().take(10).collect::<Vec<_>>().join("\n")
                    ),
                    error: None,
                }
            }
            "journalctl" => {
                let service = params["service"].as_str().unwrap_or("");
                let lines_str = lines.to_string();
                let mut args = vec!["--lines", &lines_str, "--no-pager"];
                let unit_arg;
                if !service.is_empty() {
                    unit_arg = format!("{}.service", service);
                    args.extend_from_slice(&["-u", &unit_arg]);
                }
                let (_, out) = run_cmd("journalctl", &args);
                ToolResult {
                    success: true,
                    output: out,
                    error: None,
                }
            }
            "tail" => {
                let file = params["file"].as_str().unwrap_or("/var/log/syslog");
                let (c, out) = run_cmd("tail", &["-n", &lines.to_string(), file]);
                ToolResult {
                    success: c == 0,
                    output: out,
                    error: if c != 0 {
                        Some(format!("Cannot read: {}", file))
                    } else {
                        None
                    },
                }
            }
            "logrotate_status" => {
                let (c, out) = run_cmd("logrotate", &["-d", "/etc/logrotate.conf"]);
                if c != 0 {
                    // Try alternate path
                    let (c2, out2) = run_cmd("cat", &["/etc/logrotate.conf"]);
                    ToolResult {
                        success: c2 == 0,
                        output: if c2 == 0 {
                            format!("Logrotate config loaded ({} bytes)", out2.len())
                        } else {
                            "Logrotate not installed or configured".into()
                        },
                        error: None,
                    }
                } else {
                    ToolResult {
                        success: true,
                        output: out,
                        error: None,
                    }
                }
            }
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unknown log action: {}", action)),
            },
        }
    }
}

// ── SSH ──
pub struct SshTool;
impl SshTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for SshTool {
    fn name(&self) -> &str {
        "ssh"
    }
    fn description(&self) -> &str {
        "Manage SSH: show_config, status, keys, generate_key, restart"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "ssh".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["show_config","status","keys","generate_key","restart"]},"user":{"type":"string"},"comment":{"type":"string"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("status");
        match action {
            "show_config" => {
                let (c, out) = run_cmd("cat", &["/etc/ssh/sshd_config"]);
                if c != 0 {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(
                            "Cannot read /etc/ssh/sshd_config — SSH server may not be installed"
                                .into(),
                        ),
                    };
                }
                // Filter out comments and blank lines
                let active: Vec<&str> = out
                    .lines()
                    .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                    .collect();
                ToolResult {
                    success: true,
                    output: active.join("\n"),
                    error: None,
                }
            }
            "status" => {
                let (_, out) = run_cmd("systemctl", &["status", "sshd"]);
                if out.contains("not-found") || out.contains("could not be found") {
                    let (_, out2) = run_cmd("systemctl", &["status", "ssh"]);
                    ToolResult {
                        success: true,
                        output: out2,
                        error: None,
                    }
                } else {
                    ToolResult {
                        success: true,
                        output: out,
                        error: None,
                    }
                }
            }
            "keys" => {
                let user = params["user"].as_str().unwrap_or("root");
                let home = if user == "root" {
                    "/root".to_string()
                } else {
                    format!("/home/{}", user)
                };
                let (_, out) = run_cmd(
                    "find",
                    &[&home, "-name", "authorized_keys", "-maxdepth", "3"],
                );
                if out.trim().is_empty() {
                    ToolResult {
                        success: true,
                        output: format!("No authorized_keys found for {} in {}", user, home),
                        error: None,
                    }
                } else {
                    // Read each authorized_keys file
                    let mut result = String::new();
                    for path in out.lines() {
                        let (_, content) = run_cmd("cat", &[path]);
                        if !content.trim().is_empty() {
                            let n = content.lines().count();
                            result.push_str(&format!(
                                "=== {} ({} key{}) ===\n{}\n",
                                path,
                                n,
                                if n > 1 { "s" } else { "" },
                                content
                                    .lines()
                                    .map(|l| {
                                        let parts: Vec<&str> = l.split_whitespace().collect();
                                        if parts.len() >= 3 {
                                            format!("  Type: {} | Comment: {}", parts[0], parts[2])
                                        } else {
                                            format!("  {}", l)
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            ));
                        }
                    }
                    ToolResult {
                        success: true,
                        output: result,
                        error: None,
                    }
                }
            }
            "generate_key" if confirm => {
                let user = params["user"].as_str().unwrap_or("root");
                let comment = params["comment"].as_str().unwrap_or("anos-generated");
                let home = if user == "root" {
                    "/root".to_string()
                } else {
                    format!("/home/{}", user)
                };
                let key_path = format!("{}/.ssh/id_ed25519", home);
                let (c, out) = run_cmd(
                    "ssh-keygen",
                    &[
                        "-t", "ed25519", "-f", &key_path, "-C", comment, "-N", "", "-q",
                    ],
                );
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!(
                            "✅ Generated ED25519 key for {} at {}\nPublic key: {}.pub",
                            user, key_path, key_path
                        )
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "restart" if confirm => {
                let (c, _out) = run_cmd("systemctl", &["restart", "sshd"]);
                if c != 0 {
                    let (c2, out2) = run_cmd("systemctl", &["restart", "ssh"]);
                    ToolResult {
                        success: c2 == 0,
                        output: if c2 == 0 {
                            "✅ SSH service restarted".into()
                        } else {
                            out2.clone()
                        },
                        error: if c2 != 0 { Some(out2) } else { None },
                    }
                } else {
                    ToolResult {
                        success: true,
                        output: "✅ SSH service restarted".into(),
                        error: None,
                    }
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ SSH operation needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── WebServer ──
pub struct WebServerTool;
impl WebServerTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for WebServerTool {
    fn name(&self) -> &str {
        "webserver"
    }
    fn description(&self) -> &str {
        "Manage web server (Nginx/Apache): status, test_config, reload, list_sites, restart"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "webserver".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["status","test_config","reload","list_sites","restart","detect"]}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("detect");
        // Auto-detect web server
        let server = {
            let (c, _) = run_cmd("nginx", &["-v"]);
            if c == 0 {
                "nginx"
            } else {
                let (c2, _) = run_cmd("apache2ctl", &["-v"]);
                if c2 == 0 {
                    "apache"
                } else {
                    let (c3, _) = run_cmd("httpd", &["-v"]);
                    if c3 == 0 {
                        "apache"
                    } else {
                        ""
                    }
                }
            }
        };
        match action {
            "detect" => {
                if server.is_empty() {
                    ToolResult {
                        success: true,
                        output: "No web server detected. Install: apt install nginx".into(),
                        error: None,
                    }
                } else {
                    let (_, ver) = match server {
                        "nginx" => run_cmd("nginx", &["-v"]),
                        _ => run_cmd("apache2ctl", &["-v"]),
                    };
                    ToolResult {
                        success: true,
                        output: format!("Detected: {} — {}", server, ver),
                        error: None,
                    }
                }
            }
            "status" => {
                if server.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("No web server detected".into()),
                    };
                }
                let (_, out) = run_cmd("systemctl", &["status", server]);
                ToolResult {
                    success: true,
                    output: out,
                    error: None,
                }
            }
            "test_config" => {
                if server.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("No web server detected".into()),
                    };
                }
                let (c, out) = match server {
                    "nginx" => run_cmd("nginx", &["-t"]),
                    _ => run_cmd("apache2ctl", &["configtest"]),
                };
                ToolResult {
                    success: c == 0,
                    output: out,
                    error: if c != 0 {
                        Some("Config test failed — check syntax".into())
                    } else {
                        None
                    },
                }
            }
            "list_sites" => {
                if server.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("No web server detected".into()),
                    };
                }
                let (path, suffix) = match server {
                    "nginx" => ("/etc/nginx/sites-enabled", ""),
                    _ => ("/etc/apache2/sites-enabled", ".conf"),
                };
                let (_, out) = run_cmd("ls", &["-1", path]);
                let sites: Vec<&str> = out
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.trim_end_matches(suffix))
                    .collect();
                ToolResult {
                    success: true,
                    output: if sites.is_empty() {
                        "No sites enabled".into()
                    } else {
                        format!("{} enabled sites:\n{}", server, sites.join("\n"))
                    },
                    error: None,
                }
            }
            "reload" if confirm => {
                if server.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("No web server detected".into()),
                    };
                }
                let (c, out) = run_cmd("systemctl", &["reload", server]);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ {} reloaded", server)
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "restart" if confirm => {
                if server.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("No web server detected".into()),
                    };
                }
                let (c, out) = run_cmd("systemctl", &["restart", server]);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ {} restarted", server)
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ Web server operation needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Firewall ──
pub struct FirewallTool;
impl FirewallTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for FirewallTool {
    fn name(&self) -> &str {
        "firewall"
    }
    fn description(&self) -> &str {
        "Manage firewall: status, enable, disable, list_rules, allow_port, deny_port, allow_service, delete_rule (ufw + iptables)"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "firewall".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["status","enable","disable","list_rules","allow_port","deny_port","allow_service","delete_rule"]},"port":{"type":"integer"},"protocol":{"type":"string","enum":["tcp","udp","both"],"default":"tcp"},"service":{"type":"string"},"rule_num":{"type":"integer"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("status");
        // Detect firewall: prefer ufw, fallback to iptables
        let has_ufw = run_cmd("ufw", &["--version"]).0 == 0;
        match action {
            "status" => {
                if has_ufw {
                    let (_, out) = run_cmd("ufw", &["status", "verbose"]);
                    ToolResult {
                        success: true,
                        output: out,
                        error: None,
                    }
                } else {
                    let (_, out) = run_cmd("iptables", &["-L", "-n", "-v"]);
                    ToolResult {
                        success: true,
                        output: format!(
                            "⚠️ ufw not installed (showing raw iptables)\n\n{}",
                            out.lines().take(40).collect::<Vec<_>>().join("\n")
                        ),
                        error: None,
                    }
                }
            }
            "list_rules" => {
                if has_ufw {
                    let (_, out) = run_cmd("ufw", &["status", "numbered"]);
                    ToolResult {
                        success: true,
                        output: out,
                        error: None,
                    }
                } else {
                    let (_, out) = run_cmd("iptables", &["-L", "-n", "--line-numbers"]);
                    ToolResult {
                        success: true,
                        output: out.lines().take(30).collect::<Vec<_>>().join("\n"),
                        error: None,
                    }
                }
            }
            "enable" if confirm => {
                if has_ufw {
                    let (c, out) = run_cmd("ufw", &["--force", "enable"]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            "✅ Firewall enabled".into()
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                } else {
                    ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("ufw not installed. Try: apt install ufw".into()),
                    }
                }
            }
            "disable" if confirm => {
                if has_ufw {
                    let (c, out) = run_cmd("ufw", &["disable"]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            "✅ Firewall disabled".into()
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                } else {
                    ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("ufw not installed. Try: apt install ufw".into()),
                    }
                }
            }
            "allow_port" if confirm => {
                let port = params["port"].as_u64().unwrap_or(0);
                let proto = params["protocol"].as_str().unwrap_or("tcp");
                if port == 0 || port > 65535 {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("Invalid port (1-65535)".into()),
                    };
                }
                if has_ufw {
                    let cmd_str = if proto == "both" {
                        format!("ufw allow {}", port)
                    } else {
                        format!("ufw allow proto {} to any port {}", proto, port)
                    };
                    let (c, out) = run_cmd("bash", &["-c", &cmd_str]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Allowed port {}/{}", port, proto)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                } else {
                    let proto_flag = if proto == "both" { "" } else { proto };
                    let port_str = port.to_string();
                    let mut args = vec!["-A", "INPUT", "-p"];
                    if proto_flag.is_empty() {
                        // Both TCP+UDP: two rules
                        let (c1, _o1) = run_cmd(
                            "iptables",
                            &[
                                "-A", "INPUT", "-p", "tcp", "--dport", &port_str, "-j", "ACCEPT",
                            ],
                        );
                        let (c2, _o2) = run_cmd(
                            "iptables",
                            &[
                                "-A", "INPUT", "-p", "udp", "--dport", &port_str, "-j", "ACCEPT",
                            ],
                        );
                        ToolResult {
                            success: c1 == 0 && c2 == 0,
                            output: format!("✅ Allowed port {}/tcp+udp via iptables", port),
                            error: None,
                        }
                    } else {
                        args.push(proto_flag);
                        args.push("--dport");
                        args.push(&port_str);
                        args.push("-j");
                        args.push("ACCEPT");
                        let (c, out) = run_cmd("iptables", &args);
                        ToolResult {
                            success: c == 0,
                            output: if c == 0 {
                                format!("✅ Allowed port {}/{} via iptables", port, proto_flag)
                            } else {
                                out.clone()
                            },
                            error: if c != 0 { Some(out) } else { None },
                        }
                    }
                }
            }
            "deny_port" if confirm => {
                let port = params["port"].as_u64().unwrap_or(0);
                if port == 0 || port > 65535 {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("Invalid port (1-65535)".into()),
                    };
                }
                if has_ufw {
                    let port_str = port.to_string();
                    let (c, out) = run_cmd("ufw", &["deny", &port_str]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Denied port {}", port)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                } else {
                    let port_str = port.to_string();
                    let (c, out) = run_cmd(
                        "iptables",
                        &[
                            "-A", "INPUT", "-p", "tcp", "--dport", &port_str, "-j", "DROP",
                        ],
                    );
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Denied port {}/tcp via iptables", port)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                }
            }
            "allow_service" if confirm => {
                let service = params["service"].as_str().unwrap_or("");
                if service.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("service required (e.g. ssh, http, https)".into()),
                    };
                }
                let svc = match service {
                    "ssh" | "22" => "ssh",
                    "http" | "80" => "http",
                    "https" | "443" => "https",
                    "mysql" | "3306" => "mysql",
                    "postgresql" | "5432" => "postgresql",
                    "redis" | "6379" => "redis",
                    "mongodb" | "27017" => "mongodb",
                    other => other,
                };
                if has_ufw {
                    let cmd = format!("ufw allow {}", svc);
                    let (c, out) = run_cmd("bash", &["-c", &cmd]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Allowed service: {}", service)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                } else {
                    let port = match svc {
                        "ssh" => "22",
                        "http" => "80",
                        "https" => "443",
                        "mysql" => "3306",
                        "postgresql" => "5432",
                        "redis" => "6379",
                        "mongodb" => "27017",
                        other => other,
                    };
                    let (c, out) = run_cmd(
                        "iptables",
                        &["-A", "INPUT", "-p", "tcp", "--dport", port, "-j", "ACCEPT"],
                    );
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Allowed {} (port {}) via iptables", service, port)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                }
            }
            "delete_rule" if confirm => {
                let rule_num = params["rule_num"].as_u64().unwrap_or(0);
                if rule_num == 0 {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("rule_num required — use list_rules to find the number".into()),
                    };
                }
                if has_ufw {
                    let num_str = rule_num.to_string();
                    let full_cmd = format!("yes y | ufw delete {}", num_str);
                    let (c2, out2) = run_cmd("bash", &["-c", &full_cmd]);
                    ToolResult {
                        success: c2 == 0,
                        output: if c2 == 0 {
                            format!("✅ Deleted rule #{}", rule_num)
                        } else {
                            out2.clone()
                        },
                        error: if c2 != 0 { Some(out2) } else { None },
                    }
                } else {
                    let (c, out) = run_cmd("iptables", &["-D", "INPUT", &rule_num.to_string()]);
                    ToolResult {
                        success: c == 0,
                        output: if c == 0 {
                            format!("✅ Deleted iptables rule #{}", rule_num)
                        } else {
                            out.clone()
                        },
                        error: if c != 0 { Some(out) } else { None },
                    }
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ Firewall modification needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Certbot ──
pub struct CertbotTool;
impl CertbotTool {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl SystemTool for CertbotTool {
    fn name(&self) -> &str {
        "certbot"
    }
    fn description(&self) -> &str {
        "Manage SSL/TLS certificates: list, check_expiry, issue, renew, test_renewal, revoke (Let's Encrypt / Certbot)"
    }
    fn permission(&self) -> Permission {
        Permission::Confirm
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "certbot".into(),
            description: self.description().into(),
            parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","check_expiry","issue","renew","test_renewal","revoke"]},"domains":{"type":"string","description":"Comma-separated domain list"},"email":{"type":"string"},"webroot":{"type":"string"}},"required":["action"]}),
        }
    }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        // Check if certbot is installed
        let has_certbot = run_cmd("certbot", &["--version"]).0 == 0;
        if !has_certbot {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "certbot not installed. Install: apt install certbot python3-certbot-nginx"
                        .into(),
                ),
            };
        }
        match action {
            "list" => {
                let (c, out) = run_cmd("certbot", &["certificates"]);
                if c != 0 {
                    ToolResult {
                        success: true,
                        output: "No certificates found".into(),
                        error: None,
                    }
                } else {
                    ToolResult {
                        success: true,
                        output: out,
                        error: None,
                    }
                }
            }
            "check_expiry" => {
                let domains = params["domains"].as_str().unwrap_or("");
                if domains.is_empty() {
                    // Check all certificates
                    let (_, out) = run_cmd("certbot", &["certificates"]);
                    // Parse expiry dates
                    let mut summary = String::from("Certificate Expiry Summary:\n");
                    for line in out.lines() {
                        if line.contains("Expiry Date:")
                            || line.contains("Domains:")
                            || line.contains("Certificate Name:")
                        {
                            summary.push_str(&format!("  {}\n", line.trim()));
                        }
                    }
                    if summary == "Certificate Expiry Summary:\n" {
                        summary = "No certificates found".into();
                    }
                    ToolResult {
                        success: true,
                        output: summary,
                        error: None,
                    }
                } else {
                    let (c, out) = run_cmd("certbot", &["certificates", "-d", domains]);
                    ToolResult {
                        success: c == 0,
                        output: out,
                        error: if c != 0 {
                            Some(format!("No cert for: {}", domains))
                        } else {
                            None
                        },
                    }
                }
            }
            "issue" if confirm => {
                let domains = params["domains"].as_str().unwrap_or("");
                let email = params["email"].as_str().unwrap_or("");
                if domains.is_empty() || email.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("domains and email required".into()),
                    };
                }
                let mut args = vec![
                    "certonly",
                    "--non-interactive",
                    "--agree-tos",
                    "--email",
                    email,
                    "-d",
                    domains,
                ];
                let webroot = params["webroot"].as_str().unwrap_or("");
                if !webroot.is_empty() {
                    args.push("--webroot");
                    args.push("-w");
                    args.push(webroot);
                } else {
                    args.push("--standalone");
                }
                let (c, out) = run_cmd("certbot", &args);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ SSL certificate issued for: {}\nCert path: /etc/letsencrypt/live/{}/", domains, domains.split(',').next().unwrap_or("").trim())
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            "renew" if confirm => {
                let (c, out) = run_cmd("certbot", &["renew"]);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        "✅ All certificates renewed successfully".into()
                    } else {
                        out.clone()
                    },
                    error: if c != 0 {
                        Some("Renewal failed. Check expiry dates and DNS.".into())
                    } else {
                        None
                    },
                }
            }
            "test_renewal" => {
                let (c, out) = run_cmd("certbot", &["renew", "--dry-run"]);
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        "✅ Dry-run renewal successful — all certs can be renewed".into()
                    } else {
                        format!("⚠️ Dry-run failed:\n{}", out)
                    },
                    error: if c != 0 {
                        Some("Some certificates cannot be renewed automatically".into())
                    } else {
                        None
                    },
                }
            }
            "revoke" if confirm => {
                let domains = params["domains"].as_str().unwrap_or("");
                if domains.is_empty() {
                    return ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("domains required".into()),
                    };
                }
                let (c, out) = run_cmd(
                    "certbot",
                    &["revoke", "--cert-name", domains, "--non-interactive"],
                );
                ToolResult {
                    success: c == 0,
                    output: if c == 0 {
                        format!("✅ Revoked certificate for: {}", domains)
                    } else {
                        out.clone()
                    },
                    error: if c != 0 { Some(out) } else { None },
                }
            }
            _ if confirm => ToolResult {
                success: false,
                output: String::new(),
                error: None,
            },
            _ => ToolResult {
                success: false,
                output: String::new(),
                error: Some("⚠️ Certificate operation needs confirmation. Reply 'yes'.".into()),
            },
        }
    }
}

// ── Registry ──
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn SystemTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut m: HashMap<String, Box<dyn SystemTool>> = HashMap::new();
        let pt: Box<dyn SystemTool> = Box::new(PackageTool::new());
        m.insert(pt.name().into(), pt);
        let pr: Box<dyn SystemTool> = Box::new(ProcessTool::new());
        m.insert(pr.name().into(), pr);
        let sv: Box<dyn SystemTool> = Box::new(ServiceTool::new());
        m.insert(sv.name().into(), sv);
        let fs: Box<dyn SystemTool> = Box::new(FileSystemTool::new());
        m.insert(fs.name().into(), fs);
        let nt: Box<dyn SystemTool> = Box::new(NetworkTool::new());
        m.insert(nt.name().into(), nt);
        let ut: Box<dyn SystemTool> = Box::new(UserTool::new());
        m.insert(ut.name().into(), ut);
        let ct: Box<dyn SystemTool> = Box::new(CronTool::new());
        m.insert(ct.name().into(), ct);
        let lt: Box<dyn SystemTool> = Box::new(LogTool::new());
        m.insert(lt.name().into(), lt);
        let st: Box<dyn SystemTool> = Box::new(SshTool::new());
        m.insert(st.name().into(), st);
        let wt: Box<dyn SystemTool> = Box::new(WebServerTool::new());
        m.insert(wt.name().into(), wt);
        let fw: Box<dyn SystemTool> = Box::new(FirewallTool::new());
        m.insert(fw.name().into(), fw);
        let cb: Box<dyn SystemTool> = Box::new(CertbotTool::new());
        m.insert(cb.name().into(), cb);
        Self { tools: m }
    }
    pub fn schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }
    pub async fn execute(
        &mut self,
        name: &str,
        params: &serde_json::Value,
        confirm: bool,
    ) -> ToolResult {
        match self.tools.get(name) {
            Some(t) => {
                let r = t.execute(params, confirm).await;
                tracing::info!("TOOL {} | success={}", name, r.success);
                r
            }
            None => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Tool '{}' not found. Try /tools to list.", name)),
            },
        }
    }
}

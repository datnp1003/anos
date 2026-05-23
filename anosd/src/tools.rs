use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
pub enum Permission { ReadOnly = 0, Safe = 1, Confirm = 2, Dangerous = 3 }

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

fn run_cmd(cmd: &str, args: &[&str]) -> (i32, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) => { let out = String::from_utf8_lossy(&o.stdout).to_string(); let err = String::from_utf8_lossy(&o.stderr).to_string(); (o.status.code().unwrap_or(-1), if out.is_empty() { err } else { out }) }
        Err(e) => (-1, e.to_string())
    }
}

// ── Package ──
pub struct PackageTool;
impl PackageTool { pub fn new() -> Self { Self } }
#[async_trait]
impl SystemTool for PackageTool {
    fn name(&self) -> &str { "package" }
    fn description(&self) -> &str { "Manage packages: search, install, remove, update, info, list_upgradable" }
    fn permission(&self) -> Permission { Permission::Confirm }
    fn schema(&self) -> ToolSchema { ToolSchema { name: "package".into(), description: self.description().into(), parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["search","install","remove","update","info","list_upgradable"]},"package":{"type":"string"}},"required":["action"]}) } }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        let pkg = params["package"].as_str().unwrap_or("");
        match action {
            "search" => { let (_, o) = run_cmd("apt", &["search", pkg]); ToolResult { success: true, output: o.lines().take(15).collect::<Vec<_>>().join("\n"), error: None } }
            "info" => { let (c, o) = run_cmd("dpkg", &["-s", pkg]); ToolResult { success: c==0, output: o, error: if c!=0 { Some(format!("Not installed: {}", pkg)) } else { None } } }
            "list_upgradable" => { let (_, o) = run_cmd("apt", &["list", "--upgradable"]); let n = o.lines().filter(|l| !l.starts_with("Listing")).count(); ToolResult { success: true, output: format!("{} upgradable packages", n), error: None } }
            "install" if pkg.is_empty() => ToolResult { success: false, output: String::new(), error: Some("Package name required".into()) },
            "install" if confirm => { let (c, o) = run_cmd("apt", &["install", "-y", "-qq", pkg]); ToolResult { success: c==0, output: if c==0 { format!("✅ Installed: {}", pkg) } else { o.clone() }, error: if c!=0 { Some(o) } else { None } } }
            "remove" if confirm => { let (c, o) = run_cmd("apt", &["remove", "-y", "-qq", pkg]); ToolResult { success: c==0, output: if c==0 { format!("✅ Removed: {}", pkg) } else { o.clone() }, error: if c!=0 { Some(o) } else { None } } }
            _ if confirm => ToolResult { success: false, output: String::new(), error: None },
            _ => ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' needs confirmation. Reply 'yes'.", action)) },
        }
    }
}

// ── Process ──
pub struct ProcessTool;
impl ProcessTool { pub fn new() -> Self { Self } }
#[async_trait]
impl SystemTool for ProcessTool {
    fn name(&self) -> &str { "process" }
    fn description(&self) -> &str { "Manage processes: list, kill by PID or name, info" }
    fn permission(&self) -> Permission { Permission::Dangerous }
    fn schema(&self) -> ToolSchema { ToolSchema { name: "process".into(), description: self.description().into(), parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","kill","kill_by_name","info"]},"pid":{"type":"integer"},"name":{"type":"string"}},"required":["action"]}) } }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        match action {
            "list" => { let (_, o) = run_cmd("ps", &["aux","--sort=-%cpu","--no-headers"]); ToolResult { success: true, output: o.lines().take(10).collect::<Vec<_>>().join("\n"), error: None } }
            "info" => { let pid = params["pid"].as_u64().unwrap_or(0); if pid==0 { return ToolResult { success: false, output: String::new(), error: Some("PID required".into()) }; } let (_, o) = run_cmd("ps", &["-p",&pid.to_string(),"-o","pid,user,%cpu,%mem,comm","--no-headers"]); ToolResult { success: true, output: o, error: None } }
            "kill" if confirm => { let pid = params["pid"].as_u64().unwrap_or(0); if pid==0 { return ToolResult { success: false, output: String::new(), error: Some("PID required".into()) }; } let (c, _) = run_cmd("kill", &[&pid.to_string()]); if c!=0 { let (c2, _) = run_cmd("kill", &["-9",&pid.to_string()]); ToolResult { success: c2==0, output: format!("Force killed PID {}", pid), error: None } } else { ToolResult { success: true, output: format!("Killed PID {}", pid), error: None } } }
            "kill_by_name" if confirm => { let name = params["name"].as_str().unwrap_or(""); if name.is_empty() { return ToolResult { success: false, output: String::new(), error: Some("Name required".into()) }; } let (c, _) = run_cmd("pkill", &[name]); ToolResult { success: c==0, output: format!("Killed '{}' processes", name), error: None } }
            _ if confirm => ToolResult { success: false, output: String::new(), error: None },
            _ => ToolResult { success: false, output: String::new(), error: Some("⚠️ Needs confirmation. Reply 'yes'.".into()) },
        }
    }
}

// ── Service ──
pub struct ServiceTool;
impl ServiceTool { pub fn new() -> Self { Self } }
#[async_trait]
impl SystemTool for ServiceTool {
    fn name(&self) -> &str { "service" }
    fn description(&self) -> &str { "Manage systemd services: list, status, start, stop, restart, logs" }
    fn permission(&self) -> Permission { Permission::Confirm }
    fn schema(&self) -> ToolSchema { ToolSchema { name: "service".into(), description: self.description().into(), parameters: serde_json::json!({"type":"object","properties":{"action":{"type":"string","enum":["list","status","start","stop","restart","logs"]},"name":{"type":"string"}},"required":["action"]}) } }
    async fn execute(&self, params: &serde_json::Value, confirm: bool) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        let name = params["name"].as_str().unwrap_or("");
        match action {
            "list" => { let (_, o) = run_cmd("systemctl", &["list-units","--type=service","--state=running","--no-legend"]); ToolResult { success: true, output: o.lines().take(15).collect::<Vec<_>>().join("\n"), error: None } }
            "status" => { let (_, o) = run_cmd("systemctl", &["status", &format!("{}.service", name)]); ToolResult { success: true, output: o, error: None } }
            "logs" => { let (_, o) = run_cmd("journalctl", &["-u", &format!("{}.service", name), "--lines=20", "--no-pager"]); ToolResult { success: true, output: o, error: None } }
            a @ ("start"|"stop"|"restart") if confirm => { let (c, o) = run_cmd("systemctl", &[a, &format!("{}.service", name)]); ToolResult { success: c==0, output: format!("{} {}: {}", a, name, if c==0 {"✅"} else {"❌"}), error: if c!=0 { Some(o) } else { None } } }
            _ if confirm => ToolResult { success: false, output: String::new(), error: None },
            _ => ToolResult { success: false, output: String::new(), error: Some("⚠️ Needs confirmation. Reply 'yes'.".into()) },
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
        let pt: Box<dyn SystemTool> = Box::new(PackageTool::new()); m.insert(pt.name().into(), pt);
        let pr: Box<dyn SystemTool> = Box::new(ProcessTool::new()); m.insert(pr.name().into(), pr);
        let sv: Box<dyn SystemTool> = Box::new(ServiceTool::new()); m.insert(sv.name().into(), sv);
        Self { tools: m }
    }
    pub fn schemas(&self) -> Vec<ToolSchema> { self.tools.values().map(|t| t.schema()).collect() }
    pub async fn execute(&mut self, name: &str, params: &serde_json::Value, confirm: bool) -> ToolResult {
        match self.tools.get(name) {
            Some(t) => {
                let r = t.execute(params, confirm).await;
                tracing::info!("TOOL {} | success={}", name, r.success);
                r
            }
            None => ToolResult { success: false, output: String::new(), error: Some(format!("Tool '{}' not found. Try /tools to list.", name)) },
        }
    }
}

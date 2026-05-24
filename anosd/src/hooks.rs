//! Hook System — extensible pre/post action hooks.
//!
//! Phase 3: inject custom behavior before/after tool execution, chat turns,
//! and lifecycle events. Hooks are defined as shell commands or inline callbacks.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// When a hook fires
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HookEvent {
    /// Before a tool is executed
    PreTool(String),
    /// After a tool is executed
    PostTool(String),
    /// Before processing a chat message
    PreChat,
    /// After processing a chat message
    PostChat,
    /// Before confirmation flow
    PreConfirm,
    /// After confirmation is granted/denied
    PostConfirm,
    /// On provider switch
    OnModelSwitch,
    /// On session start
    OnSessionStart,
    /// On session end
    OnSessionEnd,
}

impl HookEvent {
    pub fn name(&self) -> String {
        match self {
            HookEvent::PreTool(t) => format!("pre_tool:{}", t),
            HookEvent::PostTool(t) => format!("post_tool:{}", t),
            HookEvent::PreChat => "pre_chat".into(),
            HookEvent::PostChat => "post_chat".into(),
            HookEvent::PreConfirm => "pre_confirm".into(),
            HookEvent::PostConfirm => "post_confirm".into(),
            HookEvent::OnModelSwitch => "on_model_switch".into(),
            HookEvent::OnSessionStart => "on_session_start".into(),
            HookEvent::OnSessionEnd => "on_session_end".into(),
        }
    }
}

/// A single hook definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    /// Human-readable name
    pub name: String,
    /// Shell command to run
    pub command: String,
    /// Enabled?
    pub enabled: bool,
    /// Timeout in seconds
    pub timeout_secs: u64,
}

/// Hook execution result
#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_name: String,
    pub success: bool,
    #[allow(dead_code)]
    pub output: String,
    #[allow(dead_code)]
    pub duration_ms: u64,
    #[allow(dead_code)]
    pub exit_code: i32,
}

/// Registry of hooks organized by event
pub struct HookRegistry {
    pub hooks: HashMap<String, Vec<Hook>>,
    #[allow(dead_code)]
    pub storage_path: PathBuf,
}

impl HookRegistry {
    /// Create an empty hook registry
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            hooks: HashMap::new(),
            storage_path: PathBuf::from("hooks.yaml"),
        }
    }

    /// Load hooks from file or create empty registry
    pub fn load(data_dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(data_dir).join("hooks.yaml");
        let hooks = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_yaml::from_str::<HashMap<String, Vec<Hook>>>(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };
        let count: usize = hooks.values().map(|v| v.len()).sum();
        tracing::info!("HookRegistry: {} hooks loaded", count);
        Ok(Self {
            hooks,
            storage_path: path,
        })
    }

    /// Register a hook for an event
    #[allow(dead_code)]
    pub fn register(&mut self, event: &HookEvent, name: &str, command: &str, timeout_secs: u64) {
        let hook = Hook {
            name: name.to_string(),
            command: command.to_string(),
            enabled: true,
            timeout_secs,
        };
        let key = event.name();
        self.hooks.entry(key).or_default().push(hook);
        self.save();
    }

    /// Remove a hook by name
    #[allow(dead_code)]
    pub fn remove(&mut self, name: &str) -> usize {
        let mut removed = 0;
        for hooks in self.hooks.values_mut() {
            let before = hooks.len();
            hooks.retain(|h| h.name != name);
            removed += before - hooks.len();
        }
        self.save();
        removed
    }

    /// List all registered hooks
    pub fn list(&self) -> Vec<(String, Hook)> {
        self.hooks
            .iter()
            .flat_map(|(event, hooks)| {
                hooks
                    .iter()
                    .map(|h| (event.clone(), h.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Execute all hooks for a given event (non-blocking sequence, returns results)
    pub async fn fire(&self, event: &HookEvent, context: Option<&str>) -> Vec<HookResult> {
        let key = event.name();
        let hooks = match self.hooks.get(&key) {
            Some(h) => h.clone(),
            None => return Vec::new(),
        };

        let mut results = Vec::new();
        for hook in &hooks {
            if !hook.enabled {
                continue;
            }
            let result = execute_hook(hook, context);
            results.push(result);
        }
        results
    }

    /// Check if any hooks are registered for an event
    pub fn has_hooks(&self, event: &HookEvent) -> bool {
        self.hooks
            .get(&event.name())
            .map(|v| v.iter().any(|h| h.enabled))
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    fn save(&self) {
        if let Ok(content) = serde_yaml::to_string(&self.hooks) {
            let _ = std::fs::write(&self.storage_path, content);
        }
    }
}

/// Execute a single hook synchronously (called from async context with spawn_blocking)
fn execute_hook(hook: &Hook, context: Option<&str>) -> HookResult {
    let start = std::time::Instant::now();
    let cmd_parts: Vec<&str> = hook.command.split_whitespace().collect();
    if cmd_parts.is_empty() {
        return HookResult {
            hook_name: hook.name.clone(),
            success: false,
            output: "Empty command".into(),
            duration_ms: 0,
            exit_code: -1,
        };
    }

    let mut cmd = Command::new(cmd_parts[0]);
    if cmd_parts.len() > 1 {
        cmd.args(&cmd_parts[1..]);
    }

    // Pass context as environment variable
    if let Some(ctx) = context {
        cmd.env("ANOS_HOOK_CONTEXT", ctx);
    }
    cmd.env("ANOS_HOOK_NAME", &hook.name);

    let (out, code) = match cmd.output() {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout).trim(),
                String::from_utf8_lossy(&o.stderr).trim(),
            );
            (combined, o.status.code().unwrap_or(-1))
        }
        Err(e) => (e.to_string(), -1),
    };

    HookResult {
        hook_name: hook.name.clone(),
        success: code == 0,
        output: out.chars().take(500).collect(),
        duration_ms: start.elapsed().as_millis() as u64,
        exit_code: code,
    }
}

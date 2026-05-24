//! Audit Logger — logs all tool executions, permission checks, and session events.
//!
//! Phase 2: production-grade audit trail with permission tracking.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Severity level for audit entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditLevel {
    Info,
    Warning,
    Critical,
}

/// What was the result of a permission check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionResult {
    Allowed,
    Denied,
    PendingConfirmation,
}

/// A single audit entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub level: AuditLevel,
    pub category: String,
    pub message: String,
    pub details: Option<String>,
}

/// Thread-safe audit logger
pub struct AuditLogger {
    path: PathBuf,
    entries: Arc<Mutex<Vec<AuditEntry>>>,
}

impl AuditLogger {
    /// Create a new audit logger that writes to the given file
    pub fn new(dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(dir).join("audit.jsonl");
        fs::create_dir_all(dir)?;
        Ok(Self {
            path,
            entries: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Log a general entry
    pub async fn log(
        &self,
        level: AuditLevel,
        category: &str,
        message: &str,
        details: Option<&str>,
    ) {
        let entry = AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level,
            category: category.to_string(),
            message: message.to_string(),
            details: details.map(|d| d.to_string()),
        };
        tracing::info!(
            "[AUDIT] {} | {} | {}",
            category,
            level_name(&entry.level),
            entry.message
        );
        self.append_to_file(&entry).await;
        self.entries.lock().await.push(entry);
    }

    /// Log a tool execution attempt
    pub async fn log_tool_attempt(
        &self,
        tool: &str,
        params: &serde_json::Value,
        permission_result: &PermissionResult,
    ) {
        let level = match permission_result {
            PermissionResult::Allowed => AuditLevel::Info,
            PermissionResult::Denied => AuditLevel::Warning,
            PermissionResult::PendingConfirmation => AuditLevel::Warning,
        };
        let msg = format!(
            "Tool '{}' {} — {}",
            tool,
            serde_json::to_string(params).unwrap_or_default(),
            permission_str(permission_result),
        );
        let details = Some(format!(
            "tool={}, params={}",
            tool,
            serde_json::to_string(params).unwrap_or_default()
        ));
        self.log(level, "tool_execution", &msg, details.as_deref())
            .await;
    }

    /// Log a tool execution result
    pub async fn log_tool_result(
        &self,
        tool: &str,
        success: bool,
        output: &str,
        duration_ms: Option<u64>,
    ) {
        let level = if success {
            AuditLevel::Info
        } else {
            AuditLevel::Warning
        };
        let msg = format!(
            "Tool '{}' {} — {}",
            tool,
            if success { "succeeded" } else { "failed" },
            output.chars().take(150).collect::<String>(),
        );
        let details = Some(format!(
            "tool={}, success={}, output_len={}, duration_ms={:?}",
            tool,
            success,
            output.len(),
            duration_ms
        ));
        self.log(level, "tool_result", &msg, details.as_deref())
            .await;
    }

    /// Log a user message
    pub async fn log_user_message(&self, message: &str, intent: &str) {
        self.log(
            AuditLevel::Info,
            "user_message",
            &format!("User: {}", message.chars().take(200).collect::<String>()),
            Some(&format!("intent={}, len={}", intent, message.len())),
        )
        .await;
    }

    /// Log a model switch
    pub async fn log_model_switch(&self, from: &str, to: &str) {
        self.log(
            AuditLevel::Info,
            "model_switch",
            &format!("Switched model: {} → {}", from, to),
            None,
        )
        .await;
    }

    /// Log confirmation grant/deny
    pub async fn log_confirmation(&self, action: &str, granted: bool) {
        let level = if granted {
            AuditLevel::Info
        } else {
            AuditLevel::Warning
        };
        self.log(
            level,
            "confirmation",
            &format!(
                "Confirmation {} for: {}",
                if granted { "GRANTED" } else { "DENIED" },
                action
            ),
            None,
        )
        .await;
    }

    /// Get summary statistics
    pub async fn stats(&self) -> String {
        let entries = self.entries.lock().await;
        let total = entries.len();
        let info = entries
            .iter()
            .filter(|e| e.level == AuditLevel::Info)
            .count();
        let warn = entries
            .iter()
            .filter(|e| e.level == AuditLevel::Warning)
            .count();
        let crit = entries
            .iter()
            .filter(|e| e.level == AuditLevel::Critical)
            .count();
        format!(
            "Audit: {} entries ({} info, {} warn, {} crit)",
            total, info, warn, crit
        )
    }

    /// Get recent entries
    pub async fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().await;
        entries.iter().rev().take(limit).cloned().collect()
    }

    async fn append_to_file(&self, entry: &AuditEntry) {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(f, "{}", serde_json::to_string(entry).unwrap_or_default());
        }
    }
}

fn level_name(level: &AuditLevel) -> &str {
    match level {
        AuditLevel::Info => "INFO",
        AuditLevel::Warning => "WARN",
        AuditLevel::Critical => "CRIT",
    }
}

fn permission_str(result: &PermissionResult) -> &str {
    match result {
        PermissionResult::Allowed => "ALLOWED",
        PermissionResult::Denied => "DENIED",
        PermissionResult::PendingConfirmation => "PENDING",
    }
}

//! Sub-agent Spawn System — background parallel task execution.
//!
//! Phase 3: spawn detached sub-agents for parallel work, track status,
//! and collect results asynchronously.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// A spawned sub-agent task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgent {
    pub id: String,
    pub name: String,
    pub task: String,
    pub status: AgentStatus,
    pub spawned_at: String,
    pub finished_at: Option<String>,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "🟢 Running"),
            AgentStatus::Completed => write!(f, "✅ Completed"),
            AgentStatus::Failed => write!(f, "❌ Failed"),
            AgentStatus::Killed => write!(f, "💀 Killed"),
        }
    }
}

/// Configuration for spawning a sub-agent
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Human-readable name
    pub name: String,
    /// Shell command to execute
    pub command: String,
    /// Working directory
    pub workdir: Option<String>,
    /// Timeout in seconds (0 = no timeout)
    #[allow(dead_code)]
    pub timeout_secs: u64,
    /// Environment variables
    pub env: Vec<(String, String)>,
}

/// Registry of all spawned sub-agents
pub struct AgentRegistry {
    agents: Arc<RwLock<Vec<SubAgent>>>,
    storage_path: PathBuf,
}

impl AgentRegistry {
    pub fn new(data_dir: &str) -> Self {
        let path = PathBuf::from(data_dir).join("subagents.jsonl");
        let agents = if path.exists() {
            let mut list = Vec::new();
            if let Ok(f) = fs::File::open(&path) {
                use std::io::BufRead;
                for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                    if let Ok(a) = serde_json::from_str::<SubAgent>(&line) {
                        list.push(a);
                    }
                }
            }
            list
        } else {
            Vec::new()
        };
        Self {
            agents: Arc::new(RwLock::new(agents)),
            storage_path: path,
        }
    }

    /// Spawn a sub-agent and return its ID immediately (non-blocking)
    pub async fn spawn(&self, config: SpawnConfig) -> SubAgent {
        let id = Uuid::new_v4()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        let now = Utc::now().to_rfc3339();

        let agent = SubAgent {
            id: id.clone(),
            name: config.name,
            task: config.command.clone(),
            status: AgentStatus::Running,
            spawned_at: now.clone(),
            finished_at: None,
            output: None,
            exit_code: None,
            pid: None,
        };

        // Persist
        self.save_agent(&agent).await;

        let agents = Arc::clone(&self.agents);
        let storage = self.storage_path.clone();
        let agent_for_thread = agent.clone();

        // Spawn in background
        tokio::spawn(async move {
            let cmd_parts: Vec<&str> = config.command.split_whitespace().collect();
            if cmd_parts.is_empty() {
                let mut failed = agent_for_thread.clone();
                failed.status = AgentStatus::Failed;
                failed.finished_at = Some(Utc::now().to_rfc3339());
                failed.output = Some("Empty command".into());
                failed.exit_code = Some(-1);
                update_agent(&agents, &failed, &storage).await;
                return;
            }

            let mut cmd = Command::new(cmd_parts[0]);
            if cmd_parts.len() > 1 {
                cmd.args(&cmd_parts[1..]);
            }
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            if let Some(wd) = &config.workdir {
                cmd.current_dir(wd);
            }
            for (k, v) in &config.env {
                cmd.env(k, v);
            }

            let start = std::time::Instant::now();
            match cmd.output() {
                Ok(out) => {
                    let mut done = agent_for_thread.clone();
                    done.status = if out.status.success() {
                        AgentStatus::Completed
                    } else {
                        AgentStatus::Failed
                    };
                    done.finished_at = Some(Utc::now().to_rfc3339());
                    done.exit_code = out.status.code();
                    done.output = Some(format!(
                        "{}{}",
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    ));
                    update_agent(&agents, &done, &storage).await;
                    tracing::info!(
                        "Sub-agent {} done in {:?} — {}",
                        done.id,
                        start.elapsed(),
                        done.status
                    );
                }
                Err(e) => {
                    let mut failed = agent_for_thread.clone();
                    failed.status = AgentStatus::Failed;
                    failed.finished_at = Some(Utc::now().to_rfc3339());
                    failed.output = Some(e.to_string());
                    failed.exit_code = Some(-1);
                    update_agent(&agents, &failed, &storage).await;
                }
            }
        });

        agent
    }

    /// List all agents
    pub async fn list(&self) -> Vec<SubAgent> {
        self.agents.read().await.clone()
    }

    /// Get a specific agent by ID
    #[allow(dead_code)]
    pub async fn get(&self, id: &str) -> Option<SubAgent> {
        self.agents
            .read()
            .await
            .iter()
            .find(|a| a.id == id)
            .cloned()
    }

    /// Kill a running agent
    #[allow(dead_code)]
    pub async fn kill(&self, id: &str) -> Option<SubAgent> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.iter_mut().find(|a| a.id == id) {
            if agent.status == AgentStatus::Running {
                if let Some(pid) = agent.pid {
                    let _ = Command::new("kill").arg(pid.to_string()).output();
                }
                agent.status = AgentStatus::Killed;
                agent.finished_at = Some(Utc::now().to_rfc3339());

                if let Ok(mut f) = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.storage_path)
                {
                    let _ = writeln!(f, "{}", serde_json::to_string(&*agent).unwrap_or_default());
                }
                return Some(agent.clone());
            }
        }
        None
    }

    /// Get summary statistics
    pub async fn stats(&self) -> String {
        let agents = self.agents.read().await;
        let total = agents.len();
        let running = agents
            .iter()
            .filter(|a| a.status == AgentStatus::Running)
            .count();
        let completed = agents
            .iter()
            .filter(|a| a.status == AgentStatus::Completed)
            .count();
        let failed = agents
            .iter()
            .filter(|a| a.status == AgentStatus::Failed)
            .count();
        format!(
            "Sub-agents: {} total ({} running, {} completed, {} failed)",
            total, running, completed, failed
        )
    }

    async fn save_agent(&self, agent: &SubAgent) {
        self.agents.write().await.push(agent.clone());
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.storage_path)
        {
            let _ = writeln!(f, "{}", serde_json::to_string(agent).unwrap_or_default());
        }
    }
}

async fn update_agent(agents: &Arc<RwLock<Vec<SubAgent>>>, updated: &SubAgent, storage: &PathBuf) {
    let mut list = agents.write().await;
    if let Some(existing) = list.iter_mut().find(|a| a.id == updated.id) {
        *existing = updated.clone();
    }
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(storage)
    {
        let _ = writeln!(f, "{}", serde_json::to_string(updated).unwrap_or_default());
    }
}

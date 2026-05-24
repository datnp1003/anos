use crate::agentic::AgenticEngine;
use crate::audit::{AuditLevel, AuditLogger, PermissionResult};
use crate::context::PromptContext;
use crate::hooks::{HookEvent, HookRegistry};
use crate::intent::IntentClassifier;
use crate::memory::Memory;
use crate::provider::{ChatCompletionRequest, ChatMessage, ProviderRegistry};
use crate::snapshot::SnapshotManager;
use crate::spawn::{AgentRegistry, SpawnConfig};
use crate::systemmap::SystemMap;
use crate::tools::{ToolRegistry, ToolSchema};
use crate::upgrade::SelfUpgrade;
use crate::watcher::Watcher;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWrite;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct PendingAction {
    tool_name: String,
    params: serde_json::Value,
    summary: String,
}

struct Session {
    messages: Vec<ChatMessage>,
    tools: ToolRegistry,
    pending: Option<PendingAction>,
}

impl Session {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            tools: ToolRegistry::new(),
            pending: None,
        }
    }
}

pub struct IpcServer {
    socket_path: PathBuf,
    registry: Arc<RwLock<ProviderRegistry>>,
    context: Arc<PromptContext>,
    data_dir: String,
    watcher: Arc<Watcher>,
}

impl IpcServer {
    pub fn new(
        socket_path: PathBuf,
        registry: Arc<RwLock<ProviderRegistry>>,
        context: Arc<PromptContext>,
        data_dir: String,
        watcher: Arc<Watcher>,
    ) -> Self {
        Self {
            socket_path,
            registry,
            context,
            data_dir,
            watcher,
        }
    }
    pub async fn run(self) -> Result<()> {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
        let listener = UnixListener::bind(&self.socket_path)?;
        tracing::info!(
            "🛠️  Tools: package, process, service, filesystem, network | Listening on {}",
            self.socket_path.display()
        );
        loop {
            let (stream, _) = listener.accept().await?;
            let r = Arc::clone(&self.registry);
            let c = Arc::clone(&self.context);
            let dir = self.data_dir.clone();
            let w = Arc::clone(&self.watcher);
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, r, c, &dir, w).await {
                    tracing::error!("Error: {e}");
                }
            });
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    registry: Arc<RwLock<ProviderRegistry>>,
    context: Arc<PromptContext>,
    data_dir: &str,
    watcher: Arc<Watcher>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut session = Session::new();
    let mut memory = Memory::load(data_dir)?;
    let audit = AuditLogger::new(data_dir)?;
    let agents = AgentRegistry::new(data_dir);
    let hooks = HookRegistry::load(data_dir).unwrap_or_else(|_| HookRegistry {
        hooks: HashMap::new(),
        storage_path: PathBuf::from(data_dir).join("hooks.yaml"),
    });

    {
        let reg = registry.read().await;
        let p = reg.active();
        let greeting = format!("ANO/1.0 200 — Anos [{}: {}]\n", p.id(), p.model());
        writer.write_all(greeting.as_bytes()).await?;
        let greeting_content = ChatMessage {
            role: "assistant".into(),
            content: format!("🦾 Anos daemon ready [{}: {}]", p.id(), p.model()),
            tool_calls: None,
            tool_call_id: None,
        };
        session.messages.push(greeting_content);
    }

    let mut line = String::new();
    loop {
        line.clear();
        if buf.read_line(&mut line).await? == 0 {
            break;
        }
        let msg = line.trim().to_string();
        if msg.is_empty() {
            continue;
        }
        let normalized = msg.to_lowercase();

        // ── Confirmation flow ──
        if matches!(
            normalized.as_str(),
            "yes" | "y" | "ok" | "okay" | "đồng ý" | "dong y" | "làm đi" | "lam di" | "confirm"
        ) && execute_pending(&mut session, &mut writer, &audit).await
        {
            continue;
        }
        if matches!(
            normalized.as_str(),
            "no" | "n" | "cancel" | "hủy" | "huy" | "không" | "khong"
        ) {
            if let Some(p) = session.pending.take() {
                audit.log_confirmation(&p.summary, false).await;
                writer
                    .write_all(
                        format!("Cancelled pending action: {}\n[END]\n", p.summary).as_bytes(),
                    )
                    .await?;
                continue;
            }
        }

        // ── Commands ──
        let parts: Vec<&str> = msg.splitn(2, ' ').collect();
        match parts[0] {
            "/exit" | "/quit" => {
                audit
                    .log(
                        AuditLevel::Info,
                        "session",
                        "Session ended",
                        Some(&format!("{} messages", session.messages.len())),
                    )
                    .await;
                writer.write_all(b"Bye!\n").await?;
                break;
            }
            "/ping" => {
                writer.write_all(b"pong\n").await?;
            }
            "/version" | "/v" | "/versions" => {
                writer
                    .write_all(
                        format!(
                            "Anos {} | protocol ANO/1.0 | daemon anosd | socket {} | SSE http://{}/events\n[END]\n",
                            env!("CARGO_PKG_VERSION"),
                            std::env::var("ANOS_SOCKET").unwrap_or_else(|_| "/tmp/anos.sock".into()),
                            std::env::var("ANOS_SSE_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into())
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
            "/providers" | "/p" => {
                let l = registry.read().await.list();
                writer
                    .write_all(format!("{}\n[END]\n", l).as_bytes())
                    .await?;
            }
            "/model" => {
                let old_model = {
                    let reg = registry.read().await;
                    reg.active().id().to_string()
                };
                if parts.len() > 1 {
                    let r = { registry.write().await.switch(parts[1]) };
                    match r {
                        Ok(m) => {
                            let reg = registry.read().await;
                            let p = reg.active();
                            audit.log_model_switch(&old_model, p.id()).await;
                            writer
                                .write_all(
                                    format!("✅ {} — [{}]\n[END]\n", m, p.model()).as_bytes(),
                                )
                                .await?;
                        }
                        Err(e) => {
                            writer
                                .write_all(format!("❌ {}\n[END]\n", e).as_bytes())
                                .await?;
                        }
                    }
                } else {
                    let reg = registry.read().await;
                    let p = reg.active();
                    writer
                        .write_all(
                            format!(
                                "★ {} — {} [{}]\n{}\n[END]\n",
                                p.id(),
                                p.name(),
                                p.model(),
                                reg.list()
                            )
                            .as_bytes(),
                        )
                        .await?;
                }
            }
            "/tools" => {
                let schemas = session.tools.schemas();
                let mut out = String::from("Available tools:\n");
                for s in schemas {
                    out.push_str(&format!("  🔧 {} — {}\n", s.name, s.description));
                }
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/memory" => {
                let stats = memory.stats();
                let recent = memory.recent(10);
                let mut out = format!("🧠 Memory: {}\n\n", stats);
                for e in &recent {
                    out.push_str(&format!(
                        "- [{}] {}: {}\n",
                        e.timestamp.chars().take(16).collect::<String>(),
                        e.category,
                        e.content.chars().take(120).collect::<String>(),
                    ));
                }
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/audit" => {
                let stats = audit.stats().await;
                let recent = audit.recent(10).await;
                let mut out = format!("📋 {}\n\n", stats);
                for e in &recent {
                    out.push_str(&format!(
                        "- [{}] {}: {}\n",
                        e.timestamp.chars().take(16).collect::<String>(),
                        e.category,
                        e.message.chars().take(120).collect::<String>(),
                    ));
                }
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/spawn" => {
                if parts.len() > 1 {
                    let cmd = parts[1].to_string();
                    let name = format!("spawn_{}", cmd.chars().take(20).collect::<String>());
                    let config = SpawnConfig {
                        name: name.clone(),
                        command: cmd.clone(),
                        workdir: Some(data_dir.to_string()),
                        timeout_secs: 300,
                        env: vec![],
                    };
                    let agent = agents.spawn(config).await;
                    writer.write_all(format!(
                        "🚀 Spawned sub-agent [{}]: '{}'\n   Status: {} | Use /agents to check\n[END]\n",
                        agent.id, agent.task, agent.status
                    ).as_bytes()).await?;
                } else {
                    writer
                        .write_all(b"Usage: /spawn <command>\n[END]\n")
                        .await?;
                }
            }
            "/agents" => {
                let list = agents.list().await;
                let stats = agents.stats().await;
                let mut out = format!("🤖 {}\n", stats);
                for a in &list {
                    out.push_str(&format!(
                        "  [{}] {}: {} — {}\n",
                        a.id,
                        a.name,
                        a.status,
                        a.task.chars().take(60).collect::<String>(),
                    ));
                    if let Some(ref out_text) = a.output {
                        out.push_str(&format!(
                            "    → {}\n",
                            out_text
                                .chars()
                                .take(150)
                                .collect::<String>()
                                .replace('\n', " ")
                        ));
                    }
                }
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/hooks" => {
                let all = hooks.list();
                if all.is_empty() {
                    writer.write_all(b"No hooks registered.\n[END]\n").await?;
                } else {
                    let mut out = String::from("🪝 Registered hooks:\n");
                    for (event, hook) in &all {
                        out.push_str(&format!(
                            "  {} → {} ({})\n",
                            event,
                            hook.name,
                            if hook.enabled { "enabled" } else { "disabled" }
                        ));
                    }
                    writer
                        .write_all(format!("{}\n[END]\n", out).as_bytes())
                        .await?;
                }
            }
            "/snapshot" => {
                let status = SnapshotManager::status();
                let snapshots = SnapshotManager::list();
                let mut out = format!("{}\n", status);
                for s in &snapshots {
                    out.push_str(&format!("  📸 {} — {}\n", s.id, s.reason));
                }
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/upgrade" => {
                let upgrader = SelfUpgrade::new(data_dir);
                writer
                    .write_all(
                        format!(
                            "🔄 Checking for updates...\nCurrent: {}\n",
                            upgrader.status()
                        )
                        .as_bytes(),
                    )
                    .await?;
                if let Some((ver, title)) = SelfUpgrade::check_updates() {
                    writer
                        .write_all(
                            format!(
                                "Latest: {} — {}\nReply 'yes' to upgrade now.\n[END]\n",
                                ver, title
                            )
                            .as_bytes(),
                        )
                        .await?;
                } else {
                    writer.write_all(b"No updates available or gh CLI not found.\nTry /upgrade source for source build upgrade.\n[END]\n").await?;
                }
            }
            "/watch" => {
                if parts.len() > 1 {
                    let sub = parts[1];
                    if sub == "on" || sub == "enable" {
                        if parts.len() > 2 {
                            let msg = watcher.enable(parts[2]).await;
                            writer
                                .write_all(format!("{}\n[END]\n", msg).as_bytes())
                                .await?;
                        } else {
                            writer.write_all(b"Usage: /watch on <disk|ram|updates|load|services|security|all>\n[END]\n").await?;
                        }
                    } else if sub == "off" || sub == "disable" {
                        if parts.len() > 2 {
                            let msg = watcher.disable(parts[2]).await;
                            writer
                                .write_all(format!("{}\n[END]\n", msg).as_bytes())
                                .await?;
                        } else {
                            writer.write_all(b"Usage: /watch off <disk|ram|updates|load|services|security|all>\n[END]\n").await?;
                        }
                    } else if sub == "threshold" {
                        if parts.len() > 3 {
                            let threshold = parts[3].parse::<f64>().unwrap_or(0.0);
                            let msg = watcher.set_threshold(parts[2], threshold).await;
                            writer
                                .write_all(format!("{}\n[END]\n", msg).as_bytes())
                                .await?;
                        } else {
                            writer
                                .write_all(b"Usage: /watch threshold <check> <value>\n[END]\n")
                                .await?;
                        }
                    } else if sub == "all" || sub == "on" && parts.len() == 2 {
                        let msg = watcher.enable("all").await;
                        writer
                            .write_all(format!("{}\n[END]\n", msg).as_bytes())
                            .await?;
                    } else {
                        writer
                            .write_all(b"Usage: /watch on|off|threshold <check>\n[END]\n")
                            .await?;
                    }
                } else {
                    let summary = watcher.summary().await;
                    writer
                        .write_all(format!("{}\n[END]\n", summary).as_bytes())
                        .await?;
                }
            }
            "/checks" => {
                let list = watcher.list().await;
                let mut out = String::from("👁️ Scheduled Checks:\n");
                for c in &list {
                    let status = if c.enabled { "🟢" } else { "⚫" };
                    let val = c.last_value.as_deref().unwrap_or("-");
                    out.push_str(&format!(
                        "  {} {} — {} (every {}s) [{}] alert: {}x\n",
                        status, c.name, c.description, c.interval_secs, val, c.alert_count
                    ));
                }
                out.push_str("\n/watch on <id>, /watch off <id>, /watch threshold <id> <value>\n");
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/alerts" => {
                let alerts = watcher.alerts(10).await;
                if alerts.is_empty() {
                    writer
                        .write_all(b"No watcher alerts stored.\n[END]\n")
                        .await?;
                } else {
                    let mut out = String::from("🚨 Recent Watcher Alerts:\n");
                    for a in &alerts {
                        out.push_str(&format!(
                            "  [{}] {:?} {}: {} ({})\n",
                            a.timestamp.chars().take(19).collect::<String>(),
                            a.severity,
                            a.check_name,
                            a.message,
                            a.value
                        ));
                    }
                    writer
                        .write_all(format!("{}\n[END]\n", out).as_bytes())
                        .await?;
                }
            }
            "/memstatus" => {
                let out = memory.qdrant_status().await;
                writer
                    .write_all(format!("{}\n[END]\n", out).as_bytes())
                    .await?;
            }
            "/memindex" => match memory.qdrant_index().await {
                Ok(msg) => {
                    writer
                        .write_all(format!("✅ {}\n[END]\n", msg).as_bytes())
                        .await?
                }
                Err(e) => {
                    writer
                        .write_all(format!("❌ Qdrant index failed: {}\n[END]\n", e).as_bytes())
                        .await?
                }
            },
            "/memsearch" => {
                if parts.len() > 1 {
                    let (backend, hits) = match memory.qdrant_search(parts[1], 10).await {
                        Ok(hits) if !hits.is_empty() => ("qdrant", hits),
                        Ok(_) => ("qdrant-empty→jsonl", memory.semantic_search(parts[1], 10)),
                        Err(e) => {
                            tracing::warn!("Qdrant search failed, falling back to JSONL: {}", e);
                            ("jsonl-fallback", memory.semantic_search(parts[1], 10))
                        }
                    };
                    let mut out = format!("🧠 Semantic memory backend: {}\n", backend);
                    for h in &hits {
                        out.push_str(&format!(
                            "  {:.2} [{}] {}: {} ({})\n",
                            h.score,
                            h.entry.timestamp.chars().take(16).collect::<String>(),
                            h.entry.category,
                            h.entry.content.chars().take(160).collect::<String>(),
                            h.reason
                        ));
                    }
                    writer
                        .write_all(format!("{}\n[END]\n", out).as_bytes())
                        .await?;
                } else {
                    writer
                        .write_all(b"Usage: /memsearch <query>\n[END]\n")
                        .await?;
                }
            }
            "/stream" => {
                let mut out = format!(
                    "📡 Streaming active. SSE endpoint: http://{}/events\n",
                    std::env::var("ANOS_SSE_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into())
                );
                out.push_str("Health: /health | Events: /events\n");
                out.push_str("Supported events: START, DELTA, TOOL_START, TOOL_RESULT, ALERT, ERROR, END, heartbeat\n");
                writer
                    .write_all(format!("{}[END]\n", out).as_bytes())
                    .await?;
            }
            "/auto" => {
                if parts.len() > 1 {
                    let goal = parts[1].to_string();
                    let confirm = goal.starts_with("confirm ");
                    let real_goal = if confirm {
                        goal.strip_prefix("confirm ").unwrap_or(&goal).to_string()
                    } else {
                        goal.clone()
                    };
                    writer
                        .write_all(
                            format!("🤖 Agentic mode: '{}'\n🧠 Planning...\n", real_goal)
                                .as_bytes(),
                        )
                        .await?;
                    let result =
                        AgenticEngine::run(&real_goal, &registry, &mut session.tools, confirm, 5)
                            .await;
                    writer
                        .write_all(format!("{}\n[END]\n", result.summary).as_bytes())
                        .await?;
                } else {
                    writer.write_all(b"Usage: /auto <goal> -- autonomous multi-step task\n  /auto confirm <goal> -- auto-confirm dangerous steps\n[END]\n").await?;
                }
            }
            "/help" => {
                writer
                    .write_all(
                        "Commands:\n  /version — show Anos version\n  /model [id] — switch provider\n  /providers — list providers\n  /tools — list tools\n  /auto <goal> — autonomous multi-step task\n  /watch — proactive monitoring\n  /checks — list scheduled checks\n  /alerts — latest watcher alerts\n  /memstatus — Qdrant/fallback memory status\n  /memindex — index memory into Qdrant\n  /memsearch <q> — semantic memory search\n  /stream — streaming scaffold status\n  /memory — show memory\n  /audit — show audit log\n  /spawn <cmd> — spawn sub-agent\n  /agents — list sub-agents\n  /hooks — list hooks\n  /snapshot — list snapshots\n  /upgrade — check for updates\n  /ping — health check\n  /exit — quit\n[END]\n"
                            .as_bytes(),
                    )
                    .await?;
            }
            _ if msg.starts_with('/') => {
                writer
                    .write_all(
                        format!(
                            "❌ Unknown command: {}\nTry /help. Did you mean /version?\n[END]\n",
                            parts[0]
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
            _ => {
                tracing::info!("User: {}", msg);
                process_chat(
                    &msg,
                    &mut session,
                    &registry,
                    &context,
                    &mut memory,
                    &audit,
                    &hooks,
                    &mut writer,
                )
                .await;
            }
        }
    }
    Ok(())
}

async fn execute_pending(
    session: &mut Session,
    writer: &mut (impl AsyncWrite + Unpin),
    audit: &AuditLogger,
) -> bool {
    let Some(pending) = session.pending.take() else {
        return false;
    };
    tracing::info!(
        "✅ Confirmed pending tool: {} ({})",
        pending.tool_name,
        pending.params
    );
    audit.log_confirmation(&pending.summary, true).await;
    let start = std::time::Instant::now();
    let r = session
        .tools
        .execute(&pending.tool_name, &pending.params, true)
        .await;
    let duration_ms = start.elapsed().as_millis() as u64;

    if r.success {
        audit
            .log_tool_result(&pending.tool_name, true, &r.output, Some(duration_ms))
            .await;
        writer
            .write_all(format!(">> 🔧 {}: {}\n[END]\n", pending.tool_name, r.output).as_bytes())
            .await
            .ok();
    } else if let Some(e) = r.error {
        audit
            .log_tool_result(&pending.tool_name, false, &e, Some(duration_ms))
            .await;
        writer
            .write_all(format!(">> ❌ {}: {}\n[END]\n", pending.tool_name, e).as_bytes())
            .await
            .ok();
    } else {
        audit
            .log_tool_result(&pending.tool_name, false, "no output", Some(duration_ms))
            .await;
        writer
            .write_all(format!(">> ❌ {} failed\n[END]\n", pending.tool_name).as_bytes())
            .await
            .ok();
    }
    true
}

#[allow(clippy::too_many_arguments)]
async fn process_chat(
    msg: &str,
    session: &mut Session,
    registry: &Arc<RwLock<ProviderRegistry>>,
    context: &PromptContext,
    memory: &mut Memory,
    audit: &AuditLogger,
    hooks: &HookRegistry,
    writer: &mut (impl AsyncWrite + Unpin),
) {
    // Phase 2: proper intent classification
    let classification = IntentClassifier::classify(msg);
    tracing::info!(
        "Intent: {:?} (confidence: {:.0}%)",
        classification.intent,
        classification.confidence * 100.0
    );

    audit.log_user_message(msg, &classification.summary).await;

    // Phase 3: fire pre-chat hooks
    if hooks.has_hooks(&HookEvent::PreChat) {
        let results = hooks.fire(&HookEvent::PreChat, Some(msg)).await;
        for r in &results {
            tracing::info!(
                "Hook '{}' → {} ({}ms)",
                r.hook_name,
                if r.success { "OK" } else { "FAIL" },
                r.duration_ms
            );
        }
    }

    session.messages.push(ChatMessage {
        role: "user".into(),
        content: msg.into(),
        tool_calls: None,
        tool_call_id: None,
    });

    // Build system prompt with intent-aware SystemMap + Memory context
    let sm = SystemMap::build(classification.skill_name.as_deref(), 2000).unwrap_or_default();
    let mem_ctx = memory.build_context(classification.skill_name.as_deref(), 1500);

    let sp = context.build_system_prompt(
        classification.skill_name.as_deref(),
        Some(&sm),
        classification.skill_name.as_deref(),
    );

    // Inject memory context into system prompt
    let sp_with_mem = if mem_ctx.is_empty() {
        sp
    } else {
        format!("{}\n\n{}", sp, mem_ctx)
    };

    let mut messages = vec![ChatMessage {
        role: "system".into(),
        content: sp_with_mem,
        tool_calls: None,
        tool_call_id: None,
    }];
    messages.extend(session.messages.clone());

    let (model, pid) = {
        let reg = registry.read().await;
        let p = reg.active();
        (p.model().to_string(), p.id().to_string())
    };

    let tools = Some(openai_tool_schemas(&session.tools.schemas()));
    let mut loop_messages = messages;
    let mut req = ChatCompletionRequest {
        model: model.clone(),
        messages: loop_messages.clone(),
        temperature: Some(0.7),
        max_tokens: Some(2048),
        stream: None,
        tools,
    };

    for step in 0..3 {
        let result = { registry.read().await.active().chat(req).await };
        match result {
            Ok(resp) => {
                let choice = resp.choices.first();
                let content = choice
                    .and_then(|c| c.message.content.clone())
                    .unwrap_or_default();
                let tool_calls = choice.and_then(|c| c.message.tool_calls.clone());

                if let Some(calls) = tool_calls {
                    if calls.is_empty() {
                        finish_assistant_response(&content, &pid, session, writer).await;
                        break;
                    }

                    loop_messages.push(ChatMessage {
                        role: "assistant".into(),
                        content: content.clone(),
                        tool_calls: Some(calls.clone()),
                        tool_call_id: None,
                    });

                    let mut blocked_for_confirmation = false;
                    for call in &calls {
                        let params: serde_json::Value =
                            serde_json::from_str(&call.function.arguments).unwrap_or_default();
                        tracing::info!(
                            "🔧 Tool: {} ({})",
                            call.function.name,
                            call.function.arguments
                        );

                        // Phase 4: snapshot before dangerous tools
                        let is_dangerous =
                            matches!(call.function.name.as_str(), "process" | "package");
                        if is_dangerous && SnapshotManager::btrfs_available() {
                            let snap_reason = format!("pre-{}-{}", call.function.name, call.id);
                            if let Some(snap) = SnapshotManager::create(&snap_reason) {
                                tracing::info!(
                                    "Snapshot created before {}: {}",
                                    call.function.name,
                                    snap.id
                                );
                                writer
                                    .write_all(
                                        format!(
                                            ">> 📸 Snapshot: {} (before {})\n",
                                            snap.id, call.function.name
                                        )
                                        .as_bytes(),
                                    )
                                    .await
                                    .ok();
                            }
                        }

                        // Phase 3: fire pre-tool hooks
                        if hooks.has_hooks(&HookEvent::PreTool(call.function.name.clone())) {
                            let hr = hooks
                                .fire(
                                    &HookEvent::PreTool(call.function.name.clone()),
                                    Some(&call.function.arguments),
                                )
                                .await;
                            for h in &hr {
                                tracing::info!(
                                    "PreTool hook '{}' → {} ({}ms)",
                                    h.hook_name,
                                    if h.success { "OK" } else { "FAIL" },
                                    h.duration_ms
                                );
                            }
                        }

                        audit
                            .log_tool_attempt(
                                &call.function.name,
                                &params,
                                &PermissionResult::PendingConfirmation,
                            )
                            .await;

                        let start = std::time::Instant::now();
                        let r = session
                            .tools
                            .execute(&call.function.name, &params, false)
                            .await;
                        let duration_ms = start.elapsed().as_millis() as u64;

                        if r.success {
                            audit
                                .log_tool_attempt(
                                    &call.function.name,
                                    &params,
                                    &PermissionResult::Allowed,
                                )
                                .await;
                            audit
                                .log_tool_result(
                                    &call.function.name,
                                    true,
                                    &r.output,
                                    Some(duration_ms),
                                )
                                .await;

                            // Record successful fix in memory and opportunistically sync Qdrant.
                            let _ = memory.record_fix(
                                &call.function.name,
                                &call.function.arguments,
                                &r.output,
                            );
                            if let Err(e) = memory.qdrant_index().await {
                                tracing::debug!("Qdrant sync skipped/failed: {}", e);
                            }

                            writer
                                .write_all(
                                    format!(">> 🔧 {}: {}\n", call.function.name, r.output)
                                        .as_bytes(),
                                )
                                .await
                                .ok();
                            loop_messages.push(ChatMessage {
                                role: "tool".into(),
                                content: r.output.clone(),
                                tool_calls: None,
                                tool_call_id: Some(call.id.clone()),
                            });
                        } else if let Some(ref e) = r.error {
                            let needs_confirm = e.contains("Reply 'yes'")
                                || e.contains("Needs confirmation")
                                || e.contains("needs confirmation");
                            if needs_confirm {
                                audit
                                    .log_tool_attempt(
                                        &call.function.name,
                                        &params,
                                        &PermissionResult::PendingConfirmation,
                                    )
                                    .await;

                                let summary =
                                    format!("{} {}", call.function.name, call.function.arguments);
                                session.pending = Some(PendingAction {
                                    tool_name: call.function.name.clone(),
                                    params: params.clone(),
                                    summary: summary.clone(),
                                });
                                writer
                                    .write_all(
                                        format!(
                                            ">> ⚠️ {}\n>> Pending: {}\n>> Reply 'yes' to confirm or 'no' to cancel.\n[END]\n",
                                            e, summary
                                        )
                                        .as_bytes(),
                                    )
                                    .await
                                    .ok();
                                session.messages.push(ChatMessage {
                                    role: "assistant".into(),
                                    content: format!("Pending confirmation: {}", summary),
                                    tool_calls: None,
                                    tool_call_id: None,
                                });
                                blocked_for_confirmation = true;
                                break;
                            }

                            audit
                                .log_tool_result(&call.function.name, false, e, Some(duration_ms))
                                .await;

                            writer
                                .write_all(
                                    format!(">> ❌ {}: {}\n", call.function.name, e).as_bytes(),
                                )
                                .await
                                .ok();
                            loop_messages.push(ChatMessage {
                                role: "tool".into(),
                                content: format!("ERROR: {}", e),
                                tool_calls: None,
                                tool_call_id: Some(call.id.clone()),
                            });
                        } else {
                            let summary =
                                format!("{} {}", call.function.name, call.function.arguments);
                            session.pending = Some(PendingAction {
                                tool_name: call.function.name.clone(),
                                params: params.clone(),
                                summary: summary.clone(),
                            });
                            writer
                                .write_all(
                                    format!(
                                        ">> ⚠️ Pending: {}\n>> Reply 'yes' to confirm or 'no' to cancel.\n[END]\n",
                                        summary
                                    )
                                    .as_bytes(),
                                )
                                .await
                                .ok();
                            blocked_for_confirmation = true;
                            break;
                        }
                    }

                    if blocked_for_confirmation {
                        break;
                    }

                    req = ChatCompletionRequest {
                        model: model.clone(),
                        messages: loop_messages.clone(),
                        temperature: Some(0.7),
                        max_tokens: Some(2048),
                        stream: None,
                        tools: Some(openai_tool_schemas(&session.tools.schemas())),
                    };
                    if step == 2 {
                        writer
                            .write_all(
                                ">> ⚠️ Tool loop limit reached before final answer.\n[END]\n"
                                    .as_bytes(),
                            )
                            .await
                            .ok();
                    }
                } else {
                    finish_assistant_response(&content, &pid, session, writer).await;
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Provider: {e}");
                audit
                    .log(
                        AuditLevel::Warning,
                        "provider_error",
                        &format!("Provider error: {}", e),
                        None,
                    )
                    .await;
                writer
                    .write_all(format!(">> ❌ {}\n[END]\n", e).as_bytes())
                    .await
                    .ok();
                break;
            }
        }
    }

    // Compaction: keep last 25 messages
    if session.messages.len() > 30 {
        let split = session.messages.len().saturating_sub(25);
        session.messages = session.messages.split_off(split);
    }
}

async fn finish_assistant_response(
    content: &str,
    provider_id: &str,
    session: &mut Session,
    writer: &mut (impl AsyncWrite + Unpin),
) {
    writer
        .write_all(format!("[THINKING]\n>> {}\n[{}]\n[END]\n", content, provider_id).as_bytes())
        .await
        .ok();
    session.messages.push(ChatMessage {
        role: "assistant".into(),
        content: content.to_string(),
        tool_calls: None,
        tool_call_id: None,
    });
}

fn openai_tool_schemas(schemas: &[ToolSchema]) -> Vec<serde_json::Value> {
    schemas
        .iter()
        .map(|s| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": s.name,
                    "description": s.description,
                    "parameters": s.parameters,
                }
            })
        })
        .collect()
}

use crate::audit::{AuditLevel, AuditLogger, PermissionResult};
use crate::context::PromptContext;
use crate::intent::IntentClassifier;
use crate::memory::Memory;
use crate::provider::{ChatCompletionRequest, ChatMessage, ProviderRegistry};
use crate::systemmap::SystemMap;
use crate::tools::{ToolRegistry, ToolSchema};
use anyhow::Result;
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
}

impl IpcServer {
    pub fn new(
        socket_path: PathBuf,
        registry: Arc<RwLock<ProviderRegistry>>,
        context: Arc<PromptContext>,
        data_dir: String,
    ) -> Self {
        Self {
            socket_path,
            registry,
            context,
            data_dir,
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
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, r, c, &dir).await {
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
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut session = Session::new();
    let mut memory = Memory::load(data_dir)?;
    let audit = AuditLogger::new(data_dir)?;

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
            "/help" => {
                writer
                    .write_all(
                        "Commands:\n  /model [id] — switch provider\n  /providers — list providers\n  /tools — list tools\n  /memory — show memory\n  /audit — show audit log\n  /ping — health check\n  /exit — quit\n[END]\n"
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

async fn process_chat(
    msg: &str,
    session: &mut Session,
    registry: &Arc<RwLock<ProviderRegistry>>,
    context: &PromptContext,
    memory: &mut Memory,
    audit: &AuditLogger,
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

                            // Record successful fix in memory
                            let _ = memory.record_fix(
                                &call.function.name,
                                &call.function.arguments,
                                &r.output,
                            );

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

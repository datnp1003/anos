use crate::context::PromptContext;
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
}

impl IpcServer {
    pub fn new(
        socket_path: PathBuf,
        registry: Arc<RwLock<ProviderRegistry>>,
        context: Arc<PromptContext>,
    ) -> Self {
        Self {
            socket_path,
            registry,
            context,
        }
    }
    pub async fn run(self) -> Result<()> {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
        let listener = UnixListener::bind(&self.socket_path)?;
        tracing::info!(
            "🛠️  Tools: package, process, service | Listening on {}",
            self.socket_path.display()
        );
        loop {
            let (stream, _) = listener.accept().await?;
            let r = Arc::clone(&self.registry);
            let c = Arc::clone(&self.context);
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, r, c).await {
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
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut session = Session::new();
    {
        let reg = registry.read().await;
        let p = reg.active();
        writer
            .write_all(format!("ANO/1.0 200 — Anos [{}: {}]\n", p.id(), p.model()).as_bytes())
            .await?;
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
        if matches!(
            normalized.as_str(),
            "yes" | "y" | "ok" | "okay" | "đồng ý" | "dong y" | "làm đi" | "lam di" | "confirm"
        ) && execute_pending(&mut session, &mut writer).await
        {
            continue;
        }
        if matches!(
            normalized.as_str(),
            "no" | "n" | "cancel" | "hủy" | "huy" | "không" | "khong"
        ) {
            if let Some(p) = session.pending.take() {
                writer
                    .write_all(
                        format!("Cancelled pending action: {}\n[END]\n", p.summary).as_bytes(),
                    )
                    .await?;
                continue;
            }
        }

        let parts: Vec<&str> = msg.splitn(2, ' ').collect();
        match parts[0] {
            "/exit" | "/quit" => {
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
                if parts.len() > 1 {
                    let r = { registry.write().await.switch(parts[1]) };
                    match r {
                        Ok(m) => {
                            let reg = registry.read().await;
                            let p = reg.active();
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
            _ => {
                tracing::info!("User: {}", msg);
                process_chat(&msg, &mut session, &registry, &context, &mut writer).await;
            }
        }
    }
    Ok(())
}

async fn execute_pending(session: &mut Session, writer: &mut (impl AsyncWrite + Unpin)) -> bool {
    let Some(pending) = session.pending.take() else {
        return false;
    };
    tracing::info!(
        "✅ Confirmed pending tool: {} ({})",
        pending.tool_name,
        pending.params
    );
    let r = session
        .tools
        .execute(&pending.tool_name, &pending.params, true)
        .await;
    if r.success {
        writer
            .write_all(format!(">> 🔧 {}: {}\n[END]\n", pending.tool_name, r.output).as_bytes())
            .await
            .ok();
    } else if let Some(e) = r.error {
        writer
            .write_all(format!(">> ❌ {}: {}\n[END]\n", pending.tool_name, e).as_bytes())
            .await
            .ok();
    } else {
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
    writer: &mut (impl AsyncWrite + Unpin),
) {
    session.messages.push(ChatMessage {
        role: "user".into(),
        content: msg.into(),
        tool_calls: None,
        tool_call_id: None,
    });
    let hint = classify_intent(msg);
    let sm = SystemMap::build(hint.as_deref(), 2000).unwrap_or_default();
    let sp = context.build_system_prompt(hint.as_deref(), Some(&sm), hint.as_deref());
    let mut messages = vec![ChatMessage {
        role: "system".into(),
        content: sp,
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
                        let r = session
                            .tools
                            .execute(&call.function.name, &params, false)
                            .await;

                        if r.success {
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
                                let summary =
                                    format!("{} {}", call.function.name, call.function.arguments);
                                session.pending = Some(PendingAction {
                                    tool_name: call.function.name.clone(),
                                    params: params.clone(),
                                    summary: summary.clone(),
                                });
                                writer.write_all(format!(">> ⚠️ {}\n>> Pending: {}\n>> Reply 'yes' to confirm or 'no' to cancel.\n[END]\n", e, summary).as_bytes()).await.ok();
                                session.messages.push(ChatMessage {
                                    role: "assistant".into(),
                                    content: format!("Pending confirmation: {}", summary),
                                    tool_calls: None,
                                    tool_call_id: None,
                                });
                                blocked_for_confirmation = true;
                                break;
                            }

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
                            writer.write_all(format!(">> ⚠️ Pending: {}\n>> Reply 'yes' to confirm or 'no' to cancel.\n[END]\n", summary).as_bytes()).await.ok();
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
                writer
                    .write_all(format!(">> ❌ {}\n[END]\n", e).as_bytes())
                    .await
                    .ok();
                break;
            }
        }
    }

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

fn classify_intent(msg: &str) -> Option<String> {
    let l = msg.to_lowercase();
    let p: &[(&[&str], &str)] = &[
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
            ],
            "package",
        ),
        (
            &[
                "chậm", "lag", "cpu", "ram", "memory", "lỗi", "error", "crash",
            ],
            "system",
        ),
        (
            &[
                "mạng", "network", "port", "dns", "internet", "ping", "route",
            ],
            "network",
        ),
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
            ],
            "filesystem",
        ),
        (
            &[
                "process",
                "kill",
                "tiến trình",
                "service",
                "status",
                "start",
                "stop",
                "restart",
            ],
            "process",
        ),
        (&["kernel", "sysctl"], "kernel"),
        (&["bảo mật", "security", "firewall"], "security"),
        (&["gui", "desktop", "hyprland", "sway"], "gui"),
    ];
    for (kws, name) in p {
        if kws.iter().any(|kw| l.contains(kw)) {
            return Some(name.to_string());
        }
    }
    None
}

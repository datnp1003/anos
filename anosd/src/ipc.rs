use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::io::AsyncWrite;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use crate::context::PromptContext;
use crate::provider::{ChatCompletionRequest, ChatMessage, ProviderRegistry};
use crate::systemmap::SystemMap;
use crate::tools::ToolRegistry;

struct Session { messages: Vec<ChatMessage>, tools: ToolRegistry }
impl Session { fn new() -> Self { Self { messages: Vec::new(), tools: ToolRegistry::new() } } }

pub struct IpcServer { socket_path: PathBuf, registry: Arc<RwLock<ProviderRegistry>>, context: Arc<PromptContext> }

impl IpcServer {
    pub fn new(socket_path: PathBuf, registry: Arc<RwLock<ProviderRegistry>>, context: Arc<PromptContext>) -> Self { Self { socket_path, registry, context } }
    pub async fn run(self) -> Result<()> {
        if self.socket_path.exists() { let _ = std::fs::remove_file(&self.socket_path); }
        let listener = UnixListener::bind(&self.socket_path)?;
        tracing::info!("🛠️  Tools: package, process, service | Listening on {}", self.socket_path.display());
        loop {
            let (stream, _) = listener.accept().await?;
            let r = Arc::clone(&self.registry); let c = Arc::clone(&self.context);
            tokio::spawn(async move { if let Err(e) = handle_connection(stream, r, c).await { tracing::error!("Error: {e}"); } });
        }
    }
}

async fn handle_connection(stream: UnixStream, registry: Arc<RwLock<ProviderRegistry>>, context: Arc<PromptContext>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut session = Session::new();
    { let reg = registry.read().await; let p = reg.active(); writer.write_all(format!("ANO/1.0 200 — Anos [{}: {}]\n", p.id(), p.model()).as_bytes()).await?; }
    let mut line = String::new();
    loop {
        line.clear(); if buf.read_line(&mut line).await? == 0 { break; }
        let msg = line.trim().to_string(); if msg.is_empty() { continue; }
        let parts: Vec<&str> = msg.splitn(2, ' ').collect();
        match parts[0] {
            "/exit"|"/quit" => { writer.write_all(b"Bye!\n").await?; break; }
            "/ping" => { writer.write_all(b"pong\n").await?; }
            "/providers"|"/p" => { let l = registry.read().await.list(); writer.write_all(format!("{}\n[END]\n", l).as_bytes()).await?; }
            "/model" => {
                if parts.len() > 1 {
                    let r = { registry.write().await.switch(parts[1]) };
                    match r {
                        Ok(m) => { let reg = registry.read().await; let p = reg.active(); writer.write_all(format!("✅ {} — [{}]\n[END]\n", m, p.model()).as_bytes()).await?; }
                        Err(e) => { writer.write_all(format!("❌ {}\n[END]\n", e).as_bytes()).await?; }
                    }
                } else { let reg = registry.read().await; let p = reg.active(); writer.write_all(format!("★ {} — {} [{}]\n{}\n[END]\n", p.id(), p.name(), p.model(), reg.list()).as_bytes()).await?; }
            }
            "/tools" => {
                let schemas = session.tools.schemas();
                let mut out = String::from("Available tools:\n");
                for s in schemas { out.push_str(&format!("  🔧 {} — {}\n", s.name, s.description)); }
                writer.write_all(format!("{}\n[END]\n", out).as_bytes()).await?;
            }
            _ => { tracing::info!("User: {}", msg); process_chat(&msg, &mut session, &registry, &context, &mut writer).await; }
        }
    }
    Ok(())
}

async fn process_chat(msg: &str, session: &mut Session, registry: &Arc<RwLock<ProviderRegistry>>, context: &PromptContext, writer: &mut (impl AsyncWrite + Unpin)) {
    session.messages.push(ChatMessage { role: "user".into(), content: msg.into(), tool_calls: None, tool_call_id: None });
    let hint = classify_intent(msg);
    let sm = SystemMap::build(hint.as_deref(), 2000).unwrap_or_default();
    let sp = context.build_system_prompt(hint.as_deref(), Some(&sm), hint.as_deref());
    let mut messages = vec![ChatMessage { role: "system".into(), content: sp, tool_calls: None, tool_call_id: None }];
    messages.extend(session.messages.clone());
    let (model, pid) = { let reg = registry.read().await; let p = reg.active(); (p.model().to_string(), p.id().to_string()) };
    let req = ChatCompletionRequest { model, messages, temperature: Some(0.7), max_tokens: Some(2048), stream: None };
    let result = { registry.read().await.active().chat(req).await };
    match result {
        Ok(resp) => {
            let choice = resp.choices.first();
            let content = choice.and_then(|c| c.message.content.clone()).unwrap_or_default();
            let tool_calls = choice.and_then(|c| c.message.tool_calls.clone());
            if let Some(calls) = tool_calls {
                for call in &calls {
                    let params: serde_json::Value = serde_json::from_str(&call.function.arguments).unwrap_or_default();
                    tracing::info!("🔧 Tool: {} ({})", call.function.name, call.function.arguments);
                    let r = session.tools.execute(&call.function.name, &params, false).await;
                    if r.success { writer.write_all(format!(">> 🔧 {}: {}\n", call.function.name, r.output).as_bytes()).await.ok(); }
                    else if let Some(ref e) = r.error { writer.write_all(format!(">> ⚠️ {}: {}\n", call.function.name, e).as_bytes()).await.ok(); }
                    else { writer.write_all(format!(">> ⚠️ {}: Action needs confirmation. Reply 'yes'.\n", call.function.name).as_bytes()).await.ok(); }
                    session.messages.push(ChatMessage { role: "tool".into(), content: format!("{} result: {}", call.function.name, r.output), tool_calls: None, tool_call_id: Some(call.id.clone()) });
                }
                writer.write_all(b"[END]\n").await.ok();
            } else {
                writer.write_all(format!("[THINKING]\n>> {}\n[{}]\n[END]\n", content, pid).as_bytes()).await.ok();
                session.messages.push(ChatMessage { role: "assistant".into(), content, tool_calls: None, tool_call_id: None });
            }
            if session.messages.len() > 30 { let split = session.messages.len().saturating_sub(25); session.messages = session.messages.split_off(split); }
        }
        Err(e) => { tracing::error!("Provider: {e}"); writer.write_all(format!(">> ❌ {}\n[END]\n", e).as_bytes()).await.ok(); }
    }
}

fn classify_intent(msg: &str) -> Option<String> {
    let l = msg.to_lowercase();
    let p: &[(&[&str], &str)] = &[
        (&["cài","install","setup","gỡ","xóa","remove","update","upgrade","nâng cấp"], "package"),
        (&["chậm","lag","cpu","ram","memory","lỗi","error","crash"], "system"),
        (&["mạng","network","port","dns","internet"], "network"),
        (&["disk","ổ cứng","dọn","clean","btrfs"], "filesystem"),
        (&["process","kill","tiến trình","service","status","start","stop","restart"], "process"),
        (&["kernel","sysctl"], "kernel"), (&["bảo mật","security","firewall"], "security"),
        (&["gui","desktop","hyprland","sway"], "gui"),
    ];
    for (kws, name) in p { if kws.iter().any(|kw| l.contains(kw)) { return Some(name.to_string()); } }
    None
}

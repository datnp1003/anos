mod agentic;
mod audit;
mod context;
mod hooks;
mod intent;
mod ipc;
mod memory;
mod provider;
mod snapshot;
mod spawn;
mod systemmap;
mod tools;
mod upgrade;
mod watcher;

use anyhow::Result;
use context::PromptContext;
use ipc::IpcServer;
use provider::ProviderRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;
use watcher::Watcher;

fn load_key_from_config() -> Result<String> {
    let path = format!(
        "{}/.openclaw/openclaw.json",
        std::env::var("HOME").unwrap_or_default()
    );
    let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    if let Some(ps) = cfg["models"]["providers"].as_object() {
        for (_, v) in ps {
            if v["name"].as_str().unwrap_or("").contains("9router") {
                if let Some(k) = v["apiKey"].as_str() {
                    return Ok(k.into());
                }
            }
        }
    }
    anyhow::bail!("No API key found")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("anosd=info")),
        )
        .init();
    tracing::info!("🦾 Anos Daemon starting...");

    let dir = std::env::var("ANOS_DIR").unwrap_or_else(|_| {
        format!(
            "{}/.openclaw/workspace/anos",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let ctx = PromptContext::load(&dir)?;

    if std::env::var("ANOS_API_KEY").unwrap_or_default().is_empty() {
        if let Ok(k) = load_key_from_config() {
            std::env::set_var("ANOS_API_KEY", &k);
            tracing::info!("🔑 Key loaded from config");
        }
    }

    let pp = format!("{}/config/providers.yaml", dir);
    let registry = ProviderRegistry::load(&pp)?;
    let a = registry.active();
    tracing::info!("Active: {} — {} [{}]", a.id(), a.name(), a.model());

    let registry = Arc::new(RwLock::new(registry));
    let socket = std::env::var("ANOS_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/anos.sock"));

    // Phase 2: initialize memory and audit logger
    let mem = memory::Memory::load(&dir)?;
    tracing::info!("Memory: {} entries", mem.stats());

    // Phase 6: start proactive watcher
    let watcher = Arc::new(Watcher::new());
    watcher.start().await;

    let server = IpcServer::new(socket, registry, Arc::new(ctx), dir, watcher);
    tracing::info!("🦾 Ready. Socket: /tmp/anos.sock");
    server.run().await
}

//! Provider registry with multi-model support
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    #[allow(dead_code)]
    pub id: String,
    pub choices: Vec<Choice>,
    #[allow(dead_code)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    #[allow(dead_code)]
    pub index: u32,
    pub message: AssistantMessage,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessage {
    #[allow(dead_code)]
    pub role: Option<String>,
    pub content: Option<String>,
    #[allow(dead_code)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[allow(dead_code)]
    pub prompt_tokens: u32,
    #[allow(dead_code)]
    pub completion_tokens: u32,
    #[allow(dead_code)]
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub endpoint: String,
    pub model: String,
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub priority: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvidersFile {
    #[serde(default)]
    pub active: String,
    pub providers: Vec<ProviderConfig>,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn chat(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse>;
}

pub struct OpenAiProvider {
    id: String,
    name: String,
    endpoint: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(config: &ProviderConfig) -> Result<Self> {
        let api_key = config.api_key_env.as_ref()
            .and_then(|e| std::env::var(e).ok())
            .unwrap_or_default();
        Ok(Self {
            id: config.id.clone(), name: config.name.clone(),
            endpoint: format!("{}/chat/completions", config.endpoint.trim_end_matches('/')),
            model: config.model.clone(), api_key,
            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build()?,
        })
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn model(&self) -> &str { &self.model }
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse> {
        let resp = self.client.post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("{}: {} {}", self.id, status, body.chars().take(300).collect::<String>());
        }
        Ok(serde_json::from_str(&body)?)
    }
}

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn AiProvider>>,
    active: String,
}

impl ProviderRegistry {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ProvidersFile = serde_yaml::from_str(&content)?;
        let mut providers: HashMap<String, Box<dyn AiProvider>> = HashMap::new();
        for pc in &config.providers {
            let p: Box<dyn AiProvider> = Box::new(OpenAiProvider::new(pc)?);
            tracing::info!("  {} [{}] -> {}/{}", pc.name, pc.id, pc.endpoint, pc.model);
            providers.insert(pc.id.clone(), p);
        }
        let active = if config.active.is_empty() { providers.keys().next().cloned().unwrap_or_default() } else { config.active };
        if !providers.contains_key(&active) { anyhow::bail!("Active provider '{}' not found", active); }
        Ok(Self { providers, active })
    }
    pub fn active(&self) -> &dyn AiProvider { self.providers.get(&self.active).unwrap().as_ref() }
    pub fn switch(&mut self, id: &str) -> Result<String> {
        if let Some(p) = self.providers.get(id) {
            self.active = id.to_string();
            Ok(format!("Switched to {} ({}/{})", p.name(), p.id(), p.model()))
        } else {
            let list: Vec<String> = self.providers.values().map(|p| format!("{} — {}/{}", p.id(), p.name(), p.model())).collect();
            anyhow::bail!("Provider '{}' not found.\nAvailable:\n  {}", id, list.join("\n  "))
        }
    }
    pub fn list(&self) -> String {
        let mut out = String::from("Configured providers:\n");
        for (id, p) in &self.providers {
            let marker = if id == &self.active { "★" } else { " " };
            out.push_str(&format!("  {} {} — {} [{}]\n", marker, id, p.name(), p.model()));
        }
        out
    }
}

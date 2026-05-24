//! Streaming scaffold — event-oriented output protocol for future SSE/gRPC.
//!
//! Phase 7: Anos still speaks Unix socket, but responses can now be modeled as
//! stream events. This keeps CLI compatibility while preparing SSE/gRPC transport.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEventKind {
    Start,
    Delta,
    ToolStart,
    ToolResult,
    Alert,
    Error,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub kind: StreamEventKind,
    pub timestamp: String,
    pub content: String,
    pub meta: Option<serde_json::Value>,
}

impl StreamEvent {
    pub fn new(kind: StreamEventKind, content: impl Into<String>) -> Self {
        Self {
            kind,
            timestamp: chrono::Utc::now().to_rfc3339(),
            content: content.into(),
            meta: None,
        }
    }

    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Wire format for current Unix socket protocol.
    /// Future transports can encode this as SSE `event:` / `data:` or gRPC messages.
    pub fn wire(&self) -> String {
        let kind = match self.kind {
            StreamEventKind::Start => "START",
            StreamEventKind::Delta => "DELTA",
            StreamEventKind::ToolStart => "TOOL_START",
            StreamEventKind::ToolResult => "TOOL_RESULT",
            StreamEventKind::Alert => "ALERT",
            StreamEventKind::Error => "ERROR",
            StreamEventKind::End => "END",
        };
        format!("[EVENT:{}] {}\n", kind, self.content)
    }

    #[allow(dead_code)]
    pub fn json_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

#[allow(dead_code)]
pub struct StreamBuffer {
    events: Vec<StreamEvent>,
}

#[allow(dead_code)]
impl StreamBuffer {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: StreamEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[StreamEvent] {
        &self.events
    }

    pub fn transcript(&self) -> String {
        self.events
            .iter()
            .map(|e| e.wire())
            .collect::<Vec<_>>()
            .join("")
    }
}

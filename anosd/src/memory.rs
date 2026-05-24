//! Memory System — persistent file-based memory with simple search.
//!
//! Phase 2: learns from tool results, remembers fixes, preferences, and system state.

use crate::vector_memory::{JsonlSemanticMemory, SemanticHit, SemanticMemory};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Memory entry stored in the log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub timestamp: String,
    pub category: MemoryCategory,
    pub content: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryCategory {
    /// A fix that was applied successfully
    Fix,
    /// A decision made
    Decision,
    /// User preference
    Preference,
    /// System state snapshot
    State,
    /// General observation
    Observation,
    /// Lesson learned
    Lesson,
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryCategory::Fix => write!(f, "Fix"),
            MemoryCategory::Decision => write!(f, "Decision"),
            MemoryCategory::Preference => write!(f, "Preference"),
            MemoryCategory::State => write!(f, "State"),
            MemoryCategory::Observation => write!(f, "Observation"),
            MemoryCategory::Lesson => write!(f, "Lesson"),
        }
    }
}

pub struct Memory {
    path: PathBuf,
    entries: Vec<MemoryEntry>,
}

impl Memory {
    /// Load existing memory or create a new file
    pub fn load(dir: &str) -> Result<Self> {
        let path = PathBuf::from(dir).join("memory.jsonl");
        let mut entries = Vec::new();
        if path.exists() {
            let f = fs::File::open(&path)?;
            for line in BufReader::new(f).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<MemoryEntry>(&line) {
                    entries.push(entry);
                }
            }
        }
        tracing::info!("Memory: {} entries loaded", entries.len());
        Ok(Self { path, entries })
    }

    /// Record a new memory entry
    pub fn record(
        &mut self,
        category: MemoryCategory,
        content: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        let entry = MemoryEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            category,
            content: content.to_string(),
            tags,
        };
        // Append to file
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(&entry)?)?;
        self.entries.push(entry);
        Ok(())
    }

    /// Record a successful tool execution
    pub fn record_fix(&mut self, tool: &str, params: &str, output: &str) -> Result<()> {
        let content = format!(
            "Tool '{}' executed with params '{}': {}",
            tool, params, output
        );
        self.record(MemoryCategory::Fix, &content, vec![tool.to_string()])
    }

    /// Record a user preference
    #[allow(dead_code)]
    pub fn record_preference(&mut self, content: &str) -> Result<()> {
        self.record(
            MemoryCategory::Preference,
            content,
            vec!["preference".into()],
        )
    }

    /// Simple keyword search (returns recent entries first)
    pub fn search(&self, query: &str, limit: usize) -> Vec<&MemoryEntry> {
        let lower = query.to_lowercase();
        let mut matched: Vec<&MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&lower)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&lower))
            })
            .collect();

        // Limit and return newest first (reverse)
        matched.reverse();
        matched.truncate(limit);
        matched
    }

    /// Semantic/vector-ready search. Uses JSONL lexical fallback for now.
    pub fn semantic_search(&self, query: &str, limit: usize) -> Vec<SemanticHit> {
        JsonlSemanticMemory::new(self.entries.clone()).search_semantic(query, limit)
    }

    pub fn semantic_backend(&self) -> &'static str {
        JsonlSemanticMemory::new(Vec::new()).backend_name()
    }

    /// Get recent entries for context injection
    pub fn recent(&self, limit: usize) -> Vec<&MemoryEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    /// Search for fixes related to a tool
    #[allow(dead_code)]
    pub fn fixes_for_tool(&self, tool: &str, limit: usize) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == MemoryCategory::Fix && e.tags.iter().any(|t| t == tool))
            .rev()
            .take(limit)
            .collect()
    }

    /// Build a context string for the system prompt
    pub fn build_context(&self, query: Option<&str>, max_chars: usize) -> String {
        let semantic_hits;
        let recent_entries;
        let entries: Vec<&MemoryEntry> = if let Some(q) = query {
            semantic_hits = self.semantic_search(q, 10);
            if !semantic_hits.is_empty() {
                let mut ctx = format!("## Relevant Memory ({})\n\n", self.semantic_backend());
                for hit in &semantic_hits {
                    let e = &hit.entry;
                    let line = format!(
                        "- [score {:.2}] [{}] {}: {}\n",
                        hit.score,
                        e.timestamp.chars().take(19).collect::<String>(),
                        e.category,
                        e.content.chars().take(200).collect::<String>(),
                    );
                    if ctx.len() + line.len() > max_chars {
                        ctx.push_str("... (truncated)\n");
                        break;
                    }
                    ctx.push_str(&line);
                }
                return ctx;
            }
            self.search(q, 10)
        } else {
            recent_entries = self.recent(10);
            recent_entries
        };

        if entries.is_empty() {
            return String::new();
        }

        let mut ctx = String::from("## Recent Memory\n\n");
        for e in &entries {
            let line = format!(
                "- [{}] {}: {}\n",
                e.timestamp.chars().take(19).collect::<String>(),
                e.category,
                e.content.chars().take(200).collect::<String>(),
            );
            if ctx.len() + line.len() > max_chars {
                ctx.push_str("... (truncated)\n");
                break;
            }
            ctx.push_str(&line);
        }
        ctx
    }

    /// Count entries by category
    pub fn stats(&self) -> String {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for e in &self.entries {
            let cat = match e.category {
                MemoryCategory::Fix => "fixes",
                MemoryCategory::Decision => "decisions",
                MemoryCategory::Preference => "preferences",
                MemoryCategory::State => "states",
                MemoryCategory::Observation => "observations",
                MemoryCategory::Lesson => "lessons",
            };
            *counts.entry(cat).or_default() += 1;
        }
        counts
            .iter()
            .map(|(k, v)| format!("{} {}", v, k))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_record_and_search() {
        let dir = TempDir::new().unwrap();
        let mut mem = Memory::load(dir.path().to_str().unwrap()).unwrap();

        mem.record(
            MemoryCategory::Fix,
            "Fixed nginx restart",
            vec!["service".into()],
        )
        .unwrap();
        mem.record(
            MemoryCategory::Preference,
            "User prefers dark theme",
            vec!["preference".into()],
        )
        .unwrap();

        let results = mem.search("nginx", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, MemoryCategory::Fix);

        let prefs = mem.search("dark", 5);
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].category, MemoryCategory::Preference);
    }
}

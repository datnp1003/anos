//! Semantic memory abstraction — vector-ready interface with JSONL keyword fallback.
//!
//! Phase 7: prepares Anos for Qdrant/embedding memory without making Qdrant a hard
//! dependency. Current implementation uses lexical scoring over JSONL entries.

use crate::memory::MemoryEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHit {
    pub score: f32,
    pub entry: MemoryEntry,
    pub reason: String,
}

pub trait SemanticMemory: Send + Sync {
    fn search_semantic(&self, query: &str, limit: usize) -> Vec<SemanticHit>;
    fn backend_name(&self) -> &'static str;
}

/// JSONL fallback: no embeddings, but better than substring search.
/// Scores overlap on query terms + tag matches + recency order.
pub struct JsonlSemanticMemory {
    entries: Vec<MemoryEntry>,
}

impl JsonlSemanticMemory {
    pub fn new(entries: Vec<MemoryEntry>) -> Self {
        Self { entries }
    }

    fn tokenize(s: &str) -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() >= 2)
            .map(|t| t.to_string())
            .collect()
    }

    fn score(query_terms: &[String], entry: &MemoryEntry) -> f32 {
        if query_terms.is_empty() {
            return 0.0;
        }
        let content_terms = Self::tokenize(&entry.content);
        let tag_terms: Vec<String> = entry.tags.iter().flat_map(|t| Self::tokenize(t)).collect();

        let mut score = 0.0;
        for term in query_terms {
            if content_terms.iter().any(|t| t == term) {
                score += 1.0;
            }
            if tag_terms.iter().any(|t| t == term) {
                score += 1.5;
            }
            if entry.content.to_lowercase().contains(term) {
                score += 0.25;
            }
        }
        score / query_terms.len() as f32
    }
}

impl SemanticMemory for JsonlSemanticMemory {
    fn search_semantic(&self, query: &str, limit: usize) -> Vec<SemanticHit> {
        let terms = Self::tokenize(query);
        let mut hits: Vec<SemanticHit> = self
            .entries
            .iter()
            .cloned()
            .filter_map(|entry| {
                let score = Self::score(&terms, &entry);
                if score > 0.0 {
                    Some(SemanticHit {
                        score,
                        entry,
                        reason: "jsonl lexical/tag overlap".into(),
                    })
                } else {
                    None
                }
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit);
        hits
    }

    fn backend_name(&self) -> &'static str {
        "jsonl-semantic-fallback"
    }
}

/// Qdrant placeholder: future implementation should live behind this same trait.
#[allow(dead_code)]
pub struct QdrantSemanticMemory {
    pub endpoint: String,
    pub collection: String,
}

impl SemanticMemory for QdrantSemanticMemory {
    fn search_semantic(&self, _query: &str, _limit: usize) -> Vec<SemanticHit> {
        tracing::warn!(
            "QdrantSemanticMemory is configured but not implemented yet; returning no hits"
        );
        Vec::new()
    }

    fn backend_name(&self) -> &'static str {
        "qdrant-placeholder"
    }
}

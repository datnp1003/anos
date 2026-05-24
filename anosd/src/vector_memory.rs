//! Qdrant-backed semantic memory with JSONL fallback.
//!
//! Phase 8: real vector memory. Anos indexes MemoryEntry records into Qdrant
//! using a deterministic local hashing embedding (no external embedding service
//! required). If Qdrant is unavailable, JSONL lexical search remains the fallback.

use crate::memory::{MemoryCategory, MemoryEntry};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const DEFAULT_VECTOR_SIZE: usize = 384;

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

pub struct JsonlSemanticMemory {
    entries: Vec<MemoryEntry>,
}

impl JsonlSemanticMemory {
    pub fn new(entries: Vec<MemoryEntry>) -> Self {
        Self { entries }
    }

    fn tokenize(s: &str) -> Vec<String> {
        tokenize(s)
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

#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub endpoint: String,
    pub collection: String,
    pub vector_size: usize,
}

impl QdrantConfig {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("ANOS_QDRANT_URL")
                .or_else(|_| std::env::var("QDRANT_URL"))
                .unwrap_or_else(|_| "http://127.0.0.1:6333".into()),
            collection: std::env::var("ANOS_QDRANT_COLLECTION")
                .unwrap_or_else(|_| "anos_memory".into()),
            vector_size: DEFAULT_VECTOR_SIZE,
        }
    }
}

#[derive(Clone)]
pub struct QdrantClient {
    config: QdrantConfig,
    client: reqwest::Client,
}

impl QdrantClient {
    pub fn new(config: QdrantConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub fn config(&self) -> &QdrantConfig {
        &self.config
    }

    pub async fn health(&self) -> Result<String> {
        let url = format!("{}/", self.config.endpoint.trim_end_matches('/'));
        let resp = self.client.get(url).send().await?;
        Ok(format!("{}", resp.status()))
    }

    pub async fn ensure_collection(&self) -> Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.collection
        );
        let body = serde_json::json!({
            "vectors": {
                "size": self.config.vector_size,
                "distance": "Cosine"
            }
        });
        let resp = self.client.put(url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant ensure collection failed: {} {}", status, text);
        }
        Ok(())
    }

    pub async fn upsert_entries(&self, entries: &[MemoryEntry]) -> Result<usize> {
        self.ensure_collection().await?;
        let points: Vec<serde_json::Value> = entries
            .iter()
            .map(|entry| {
                let text = entry_text(entry);
                serde_json::json!({
                    "id": point_id(entry),
                    "vector": hashing_embedding(&text, self.config.vector_size),
                    "payload": {
                        "timestamp": entry.timestamp,
                        "category": entry.category,
                        "content": entry.content,
                        "tags": entry.tags,
                        "text": text,
                    }
                })
            })
            .collect();

        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.config.endpoint.trim_end_matches('/'),
            self.config.collection
        );
        let body = serde_json::json!({ "points": points });
        let resp = self.client.put(url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant upsert failed: {} {}", status, text);
        }
        Ok(entries.len())
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SemanticHit>> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.config.endpoint.trim_end_matches('/'),
            self.config.collection
        );
        let body = serde_json::json!({
            "vector": hashing_embedding(query, self.config.vector_size),
            "limit": limit,
            "with_payload": true
        });
        let resp = self.client.post(url).json(&body).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("Qdrant search failed: {} {}", status, text);
        }
        let parsed: QdrantSearchResponse = serde_json::from_str(&text)?;
        Ok(parsed
            .result
            .into_iter()
            .filter_map(|p| p.into_hit())
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    result: Vec<QdrantScoredPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantScoredPoint {
    score: f32,
    payload: Option<serde_json::Value>,
}

impl QdrantScoredPoint {
    fn into_hit(self) -> Option<SemanticHit> {
        let payload = self.payload?;
        let category: MemoryCategory =
            serde_json::from_value(payload.get("category")?.clone()).ok()?;
        let tags: Vec<String> =
            serde_json::from_value(payload.get("tags")?.clone()).unwrap_or_default();
        let entry = MemoryEntry {
            timestamp: payload.get("timestamp")?.as_str()?.to_string(),
            category,
            content: payload.get("content")?.as_str()?.to_string(),
            tags,
        };
        Some(SemanticHit {
            score: self.score,
            entry,
            reason: "qdrant vector cosine similarity".into(),
        })
    }
}

pub fn hashing_embedding(text: &str, dim: usize) -> Vec<f32> {
    let mut vector = vec![0.0f32; dim];
    for token in tokenize(text) {
        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let hash = hasher.finish();
        let idx = (hash as usize) % dim;
        let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
        vector[idx] += sign;
    }
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vector {
            *v /= norm;
        }
    }
    vector
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_string())
        .collect()
}

fn entry_text(entry: &MemoryEntry) -> String {
    format!(
        "{} {} {}",
        entry.category,
        entry.tags.join(" "),
        entry.content
    )
}

fn point_id(entry: &MemoryEntry) -> u64 {
    let mut hasher = DefaultHasher::new();
    entry.timestamp.hash(&mut hasher);
    entry.content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_is_normalized() {
        let v = hashing_embedding("nginx failed port conflict", DEFAULT_VECTOR_SIZE);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }
}

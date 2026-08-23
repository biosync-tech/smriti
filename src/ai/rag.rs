//! RAG Query Engine — "Ask your knowledge graph anything"
//!
//! Combines hybrid search (FTS5 + semantic) with graph traversal
//! to build context, then uses the configured InferenceBackend for generation.
//! Research ref: Graph-Based Memory Survey arXiv:2602.05665

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::inference::{AuditedInference, CallContext, GenerateRequest, InferenceError};
use crate::storage::Database;

/// Configuration for RAG queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    /// Max notes to retrieve for context
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Max tokens for context window
    #[serde(default = "default_context_tokens")]
    pub context_max_tokens: usize,
    /// Max tokens for the answer
    #[serde(default = "default_answer_tokens")]
    pub answer_max_tokens: u32,
    /// Weight for FTS vs semantic (0.0 = pure semantic, 1.0 = pure FTS)
    #[serde(default = "default_fts_weight")]
    pub fts_weight: f64,
    /// Graph traversal depth for expanding context
    #[serde(default = "default_graph_depth")]
    pub graph_depth: usize,
    /// Temperature for answer generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_top_k() -> usize {
    10
}
fn default_context_tokens() -> usize {
    4096
}
fn default_answer_tokens() -> u32 {
    1024
}
fn default_fts_weight() -> f64 {
    0.5
}
fn default_graph_depth() -> usize {
    1
}
fn default_temperature() -> f32 {
    0.3
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: default_top_k(),
            context_max_tokens: default_context_tokens(),
            answer_max_tokens: default_answer_tokens(),
            fts_weight: default_fts_weight(),
            graph_depth: default_graph_depth(),
            temperature: default_temperature(),
        }
    }
}

/// A RAG query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQuery {
    /// The user's question
    pub question: String,
    /// Optional tag filter
    pub tag_filter: Option<String>,
    /// Override default config
    #[serde(default)]
    pub config: Option<RagConfig>,
}

/// A RAG query response with answer and sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    /// The generated answer
    pub answer: String,
    /// Source notes used for context
    pub sources: Vec<RagSource>,
    /// Token usage statistics
    pub tokens_used: Option<crate::inference::TokenUsage>,
    /// Model used for generation
    pub model: String,
}

/// A source note referenced in the answer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagSource {
    pub note_id: String,
    pub title: String,
    pub relevance_score: f64,
    pub match_type: String, // "fts", "semantic", "both", "graph"
    pub preview: String,
}

/// The RAG query engine
pub struct RagEngine {
    db: Arc<Database>,
    backend: Arc<AuditedInference>,
    config: RagConfig,
}

impl RagEngine {
    pub fn new(db: Arc<Database>, backend: Arc<AuditedInference>, config: RagConfig) -> Self {
        Self {
            db,
            backend,
            config,
        }
    }

    /// Answer a question using RAG over the knowledge graph
    pub async fn query(&self, query: &RagQuery) -> Result<RagResponse, InferenceError> {
        let config = query.config.as_ref().unwrap_or(&self.config);

        // Step 1: Generate query embedding
        let query_embedding = self
            .backend
            .embed(std::slice::from_ref(&query.question))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| InferenceError::EmbeddingFailed("No embedding returned".into()))?;

        // Step 2: Hybrid search
        let search_results = self
            .db
            .hybrid_search(
                &query.question,
                &query_embedding,
                config.top_k,
                config.fts_weight,
            )
            .map_err(|e| InferenceError::GenerationFailed(format!("Search failed: {}", e)))?;

        // Step 3: Build sources list and fetch full note content
        let mut sources: Vec<RagSource> = Vec::new();
        let mut context_notes: Vec<(String, String, String)> = Vec::new(); // (id, title, content)
        let hit_ids: Vec<String> = search_results.iter().map(|r| r.id.clone()).collect();
        let fetched = self
            .db
            .get_notes_by_ids(&hit_ids)
            .map_err(|e| InferenceError::GenerationFailed(format!("Fetch notes failed: {}", e)))?;
        let notes_by_id: std::collections::HashMap<_, _> =
            fetched.into_iter().map(|n| (n.id.clone(), n)).collect();

        for result in &search_results {
            if let Some(note) = notes_by_id.get(&result.id) {
                sources.push(RagSource {
                    note_id: note.id.clone(),
                    title: note.title.clone(),
                    relevance_score: result.score,
                    match_type: result.match_source.clone(),
                    preview: crate::safe_truncate(&note.content, 200).to_string(),
                });
                context_notes.push((note.id.clone(), note.title.clone(), note.content.clone()));
            }
        }

        // Step 4: Expand context via graph neighborhood (honors graph_depth)
        if config.graph_depth > 0 && !context_notes.is_empty() {
            let seed: Vec<&str> = context_notes.iter().map(|(id, _, _)| id.as_str()).collect();
            let neighbor_limit = config.top_k.max(5);
            let neighbor_ids = self
                .db
                .graph_neighbors(&seed, config.graph_depth, neighbor_limit)
                .unwrap_or_default();

            let neighbor_notes = self.db.get_notes_by_ids(&neighbor_ids).unwrap_or_default();
            for note in neighbor_notes {
                if context_notes.iter().any(|(id, _, _)| id == &note.id) {
                    continue;
                }
                sources.push(RagSource {
                    note_id: note.id.clone(),
                    title: note.title.clone(),
                    relevance_score: 0.0,
                    match_type: "graph".into(),
                    preview: crate::safe_truncate(&note.content, 200).to_string(),
                });
                context_notes.push((note.id, note.title, note.content));
            }
        }

        // Step 5: Assemble prompt with context
        let context = self.build_context(&context_notes, config.context_max_tokens);

        let system_prompt = "You are Smriti AI, a knowledge assistant that answers questions \
            based on the user's personal knowledge graph. Use ONLY the provided context to answer. \
            If the context doesn't contain enough information, say so. \
            Always cite which notes you're referencing by their title. \
            Be concise and accurate."
            .to_string();

        let user_prompt = format!(
            "## Context (from knowledge graph)\n\n{}\n\n## Question\n\n{}",
            context, query.question
        );

        // Step 6: Generate answer
        let request = GenerateRequest {
            prompt: user_prompt,
            system: Some(system_prompt),
            max_tokens: Some(config.answer_max_tokens),
            temperature: Some(config.temperature),
            top_p: Some(0.9),
            stop: None,
            thinking: Some(false),
        };

        let note_ids: Vec<String> = sources.iter().map(|s| s.note_id.clone()).collect();
        let ctx = CallContext::new("notes_ask")
            .with_notes(note_ids)
            .with_template("rag@v1");
        let response = self.backend.generate_audited(&request, &ctx).await?;

        Ok(RagResponse {
            answer: response.text,
            sources,
            tokens_used: response.tokens_used,
            model: response.model,
        })
    }

    /// Build context string from notes, respecting token budget
    fn build_context(&self, notes: &[(String, String, String)], max_tokens: usize) -> String {
        let mut context = String::new();
        let mut chars_used = 0;

        for (i, (_id, title, content)) in notes.iter().enumerate() {
            let note_block = format!("### Note {}: {}\n{}\n\n", i + 1, title, content);

            // Rough token estimate: ~4 chars per token
            if chars_used + note_block.len() > max_tokens * 4 {
                // Truncate this note's content to fit
                let remaining = (max_tokens * 4).saturating_sub(chars_used);
                if remaining > 100 {
                    let truncated = crate::safe_truncate(&note_block, remaining);
                    context.push_str(truncated);
                }
                break;
            }

            context.push_str(&note_block);
            chars_used += note_block.len();
        }

        context
    }
}

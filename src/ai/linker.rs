//! AI-Powered Smart Linking — semantic similarity-based link suggestions
//!
//! Upgrades the existing SmartLinker with embedding-based similarity,
//! replacing keyword Jaccard with cosine distance.

use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::inference::{AuditedInference, InferenceError};
use crate::models::*;
use crate::storage::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiLinkSuggestion {
    pub source_id: String,
    pub source_title: String,
    pub target_id: String,
    pub target_title: String,
    pub similarity_score: f64,
    pub reason: String,
    pub suggested_link_type: String,
}

pub struct AiLinker {
    db: Arc<Database>,
    backend: Arc<AuditedInference>,
}

impl AiLinker {
    pub fn new(db: Arc<Database>, backend: Arc<AuditedInference>) -> Self {
        Self { db, backend }
    }

    /// Find semantic connections between notes using embeddings
    pub async fn find_semantic_links(
        &self,
        note_id: &str,
        max_suggestions: usize,
        min_similarity: f64,
    ) -> Result<Vec<AiLinkSuggestion>, InferenceError> {
        let note = self.db.get_note(note_id).map_err(|e| {
            InferenceError::GenerationFailed(format!("Note not found: {}", e))
        })?;

        // Generate embedding for this note
        let text = format!("{}\n\n{}", note.title, note.content);
        let embeddings = self.backend.embed(&[text]).await?;
        let query_embedding = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| InferenceError::EmbeddingFailed("No embedding returned".into()))?;

        // Semantic search for similar notes
        let results = self
            .db
            .semantic_search(&query_embedding, max_suggestions + 5)
            .map_err(|e| InferenceError::GenerationFailed(format!("Search failed: {}", e)))?;

        // Get existing links to filter them out
        let existing_links = self.db.get_forward_links(note_id).unwrap_or_default();
        let existing_targets: std::collections::HashSet<String> = existing_links
            .iter()
            .map(|n| n.id.clone())
            .collect();

        let mut suggestions: Vec<AiLinkSuggestion> = Vec::new();

        for result in &results {
            // Skip self and already-linked notes
            if result.id == note_id || existing_targets.contains(&result.id) {
                continue;
            }

            // Convert distance to similarity (cosine distance: 0 = identical, 2 = opposite)
            let similarity = 1.0 - (result.distance / 2.0);

            if similarity < min_similarity {
                continue;
            }

            suggestions.push(AiLinkSuggestion {
                source_id: note_id.to_string(),
                source_title: note.title.clone(),
                target_id: result.id.clone(),
                target_title: result.title.clone(),
                similarity_score: similarity,
                reason: format!("Semantic similarity: {:.0}%", similarity * 100.0),
                suggested_link_type: "Semantic".into(),
            });

            if suggestions.len() >= max_suggestions {
                break;
            }
        }

        Ok(suggestions)
    }

    /// Find and auto-create semantic links for all notes above threshold
    pub async fn auto_link_all(
        &self,
        min_similarity: f64,
        max_per_note: usize,
    ) -> Result<usize, InferenceError> {
        let notes = self
            .db
            .list_notes(&NoteListQuery {
                limit: 10_000,
                offset: 0,
                sort: SortOrder::UpdatedDesc,
                tag: None,
            })
            .map_err(|e| InferenceError::GenerationFailed(e.to_string()))?;

        let mut total_created = 0;

        for note in &notes {
            match self
                .find_semantic_links(&note.id, max_per_note, min_similarity)
                .await
            {
                Ok(suggestions) => {
                    for s in &suggestions {
                        if s.similarity_score >= min_similarity {
                            if let Err(e) =
                                self.db.create_link(&s.source_id, &s.target_id, LinkType::Semantic)
                            {
                                tracing::warn!("Failed to create link: {}", e);
                            } else {
                                total_created += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to find links for {}: {}", note.id, e);
                }
            }
        }

        Ok(total_created)
    }
}

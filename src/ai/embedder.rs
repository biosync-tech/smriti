//! Auto-Embedding Pipeline
//!
//! Automatically generates embeddings for notes when they are created
//! or updated, removing the need for external embedding APIs.

use std::sync::Arc;

use crate::inference::{InferenceError, SharedBackend};
use crate::inference::queue::EmbeddingQueue;
use crate::models::NoteListQuery;
use crate::storage::Database;

/// The auto-embedder manages the embedding queue and provides
/// convenience methods for embedding operations.
pub struct AutoEmbedder {
    queue: EmbeddingQueue,
    backend: SharedBackend,
    db: Arc<Database>,
}

impl AutoEmbedder {
    /// Create a new auto-embedder with a background processing queue
    pub fn new(db: Arc<Database>, backend: SharedBackend, batch_size: usize) -> Self {
        let queue = EmbeddingQueue::new(backend.clone(), batch_size);
        Self { queue, backend, db }
    }

    /// Queue a note for embedding (non-blocking)
    pub async fn on_note_created(&self, note_id: &str) -> Result<(), InferenceError> {
        self.queue.enqueue(note_id.to_string()).await
    }

    /// Queue a note for re-embedding after update
    pub async fn on_note_updated(&self, note_id: &str) -> Result<(), InferenceError> {
        self.queue.enqueue(note_id.to_string()).await
    }

    /// Embed a single note immediately (blocking, bypasses queue)
    pub async fn embed_now(&self, note_id: &str) -> Result<Vec<f32>, InferenceError> {
        let note = self.db.get_note(note_id).map_err(|e| {
            InferenceError::GenerationFailed(format!("Note not found: {}", e))
        })?;

        let text = format!("{}\n\n{}", note.title, note.content);
        let embeddings = self.backend.embed(&[text]).await?;

        let embedding = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| InferenceError::EmbeddingFailed("No embedding returned".into()))?;

        // Store in DB
        self.db
            .store_embedding(note_id, &embedding, Some(self.backend.name()))
            .map_err(|e| {
                InferenceError::GenerationFailed(format!("Failed to store: {}", e))
            })?;

        Ok(embedding)
    }

    /// Embed all notes that don't have embeddings yet
    pub async fn embed_missing(&self) -> Result<usize, InferenceError> {
        let notes = self.db.list_notes(&NoteListQuery {
            limit: 100_000,
            offset: 0,
            sort: crate::models::SortOrder::UpdatedDesc,
            tag: None,
        }).map_err(|e| InferenceError::GenerationFailed(e.to_string()))?;

        // Check which notes already have embeddings
        let mut missing: Vec<String> = Vec::new();
        for note in &notes {
            let has_embedding = self
                .db
                .execute(|conn| {
                    let count: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM note_embeddings_meta WHERE note_id = ?1",
                            [&note.id],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);
                    Ok(count > 0)
                })
                .unwrap_or(false);

            if !has_embedding {
                missing.push(note.id.clone());
            }
        }

        let count = missing.len();
        if count > 0 {
            tracing::info!("Queuing {} notes for embedding", count);
            self.queue.enqueue_batch(missing).await?;
        }

        Ok(count)
    }

    /// Re-embed all notes (e.g., after model change)
    pub async fn reembed_all(&self) -> Result<(), InferenceError> {
        self.queue.reembed_all().await
    }

    /// Get embedding queue statistics
    pub async fn stats(&self) -> crate::inference::queue::EmbedQueueStats {
        self.queue.get_stats().await
    }

    /// Shutdown the background processor
    pub async fn shutdown(&self) {
        self.queue.shutdown().await;
    }
}

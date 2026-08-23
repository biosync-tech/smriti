//! Embedding Queue — background processor for auto-embedding notes
//!
//! When a note is created or updated, it gets queued for embedding.
//! A background Tokio task processes the queue in batches, generating
//! embeddings via the configured InferenceBackend and storing them
//! in sqlite-vec.

use std::sync::Arc;
use tokio::sync::RwLock;

use super::{InferenceError, SharedBackend};
use crate::storage::Database;

/// Message types for the embedding queue
#[derive(Debug, Clone)]
pub enum EmbedMessage {
    /// Embed a single note
    EmbedNote { note_id: String },
    /// Embed multiple notes (batch)
    EmbedBatch { note_ids: Vec<String> },
    /// Re-embed all notes (e.g., after model change)
    ReembedAll,
    /// Graceful shutdown
    Shutdown,
}

/// Stats for the embedding queue
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EmbedQueueStats {
    pub pending: usize,
    pub processed: usize,
    pub failed: usize,
    pub last_error: Option<String>,
}

/// Embed `note_ids` now: fetch title+content, call the backend, store vectors.
pub async fn embed_note_ids(
    db: &Database,
    backend: &SharedBackend,
    note_ids: &[String],
) -> Result<usize, InferenceError> {
    if note_ids.is_empty() {
        return Ok(0);
    }

    let mut texts: Vec<(String, String)> = Vec::new();
    for id in note_ids {
        match db.get_note(id) {
            Ok(note) => {
                texts.push((note.id, format!("{}\n\n{}", note.title, note.content)));
            }
            Err(e) => {
                tracing::warn!("Skipping embed for missing note {}: {}", id, e);
            }
        }
    }
    if texts.is_empty() {
        return Ok(0);
    }

    let text_strs: Vec<String> = texts.iter().map(|(_, t)| t.clone()).collect();
    let embeddings = backend.embed(&text_strs).await?;
    if embeddings.len() != texts.len() {
        return Err(InferenceError::EmbeddingFailed(format!(
            "backend returned {} embeddings for {} texts",
            embeddings.len(),
            texts.len()
        )));
    }

    let model = backend.name().to_string();
    for ((note_id, _), embedding) in texts.iter().zip(embeddings.into_iter()) {
        db.store_embedding(note_id, &embedding, Some(&model))
            .map_err(|e| InferenceError::GenerationFailed(format!("Failed to store: {}", e)))?;
    }
    Ok(texts.len())
}

/// The embedding queue processor
pub struct EmbeddingQueue {
    sender: tokio::sync::mpsc::Sender<EmbedMessage>,
    stats: Arc<RwLock<EmbedQueueStats>>,
}

impl EmbeddingQueue {
    /// Create a new embedding queue and spawn the background processor
    pub fn new(db: Arc<Database>, backend: SharedBackend, batch_size: usize) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<EmbedMessage>(1000);
        let stats = Arc::new(RwLock::new(EmbedQueueStats::default()));

        let processor_stats = stats.clone();

        tokio::spawn(async move {
            Self::process_loop(receiver, db, backend, batch_size, processor_stats).await;
        });

        Self { sender, stats }
    }

    /// Queue a note for embedding
    pub async fn enqueue(&self, note_id: String) -> Result<(), InferenceError> {
        self.sender
            .send(EmbedMessage::EmbedNote { note_id })
            .await
            .map_err(|e| InferenceError::GenerationFailed(format!("Queue send failed: {}", e)))?;

        let mut stats = self.stats.write().await;
        stats.pending += 1;

        Ok(())
    }

    /// Queue multiple notes for embedding
    pub async fn enqueue_batch(&self, note_ids: Vec<String>) -> Result<(), InferenceError> {
        let count = note_ids.len();
        self.sender
            .send(EmbedMessage::EmbedBatch { note_ids })
            .await
            .map_err(|e| InferenceError::GenerationFailed(format!("Queue send failed: {}", e)))?;

        let mut stats = self.stats.write().await;
        stats.pending += count;

        Ok(())
    }

    /// Trigger re-embedding of all notes
    pub async fn reembed_all(&self) -> Result<(), InferenceError> {
        self.sender
            .send(EmbedMessage::ReembedAll)
            .await
            .map_err(|e| InferenceError::GenerationFailed(format!("Queue send failed: {}", e)))?;
        Ok(())
    }

    /// Get current queue statistics
    pub async fn get_stats(&self) -> EmbedQueueStats {
        self.stats.read().await.clone()
    }

    /// Shutdown the queue processor
    pub async fn shutdown(&self) {
        let _ = self.sender.send(EmbedMessage::Shutdown).await;
    }

    async fn process_loop(
        mut receiver: tokio::sync::mpsc::Receiver<EmbedMessage>,
        db: Arc<Database>,
        backend: SharedBackend,
        batch_size: usize,
        stats: Arc<RwLock<EmbedQueueStats>>,
    ) {
        let mut pending_ids: Vec<String> = Vec::new();

        tracing::info!(
            "Embedding queue processor started (batch_size={})",
            batch_size
        );

        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(200), receiver.recv()).await
            {
                Ok(Some(msg)) => match msg {
                    EmbedMessage::EmbedNote { note_id } => {
                        pending_ids.push(note_id);
                    }
                    EmbedMessage::EmbedBatch { note_ids } => {
                        pending_ids.extend(note_ids);
                    }
                    EmbedMessage::ReembedAll => match db.list_note_ids_missing_embeddings() {
                        Ok(missing) => {
                            // Re-embed everything: missing + already-embedded.
                            // list_notes via missing is not enough — pull all ids.
                            pending_ids.extend(all_note_ids(&db));
                            if missing.is_empty() {
                                tracing::info!("Re-embed all: queued {} notes", pending_ids.len());
                            }
                        }
                        Err(e) => {
                            tracing::error!("Re-embed all failed to list notes: {}", e);
                        }
                    },
                    EmbedMessage::Shutdown => {
                        if !pending_ids.is_empty() {
                            let _ = Self::process_batch(&db, &backend, &pending_ids, &stats).await;
                        }
                        tracing::info!("Embedding queue shutting down");
                        break;
                    }
                },
                Ok(None) => break,
                Err(_) => {}
            }

            if !pending_ids.is_empty() {
                let batch: Vec<String> = pending_ids
                    .drain(..pending_ids.len().min(batch_size))
                    .collect();
                if let Err(e) = Self::process_batch(&db, &backend, &batch, &stats).await {
                    tracing::error!("Embedding batch failed: {}", e);
                    let mut s = stats.write().await;
                    s.failed += batch.len();
                    s.last_error = Some(e.to_string());
                }
            }
        }
    }

    async fn process_batch(
        db: &Database,
        backend: &SharedBackend,
        note_ids: &[String],
        stats: &Arc<RwLock<EmbedQueueStats>>,
    ) -> Result<(), InferenceError> {
        let count = embed_note_ids(db, backend, note_ids).await?;
        let mut s = stats.write().await;
        s.processed += count;
        if s.pending >= count {
            s.pending -= count;
        } else {
            s.pending = 0;
        }
        tracing::debug!("Embedded {} notes (pending: {})", count, s.pending);
        Ok(())
    }
}

fn all_note_ids(db: &Database) -> Vec<String> {
    match db.list_notes(&crate::models::NoteListQuery {
        limit: 100_000,
        offset: 0,
        sort: crate::models::SortOrder::UpdatedDesc,
        tag: None,
    }) {
        Ok(notes) => notes.into_iter().map(|n| n.id).collect(),
        Err(e) => {
            tracing::error!("Failed to list notes for re-embed: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::mock::MockBackend;
    use crate::models::note::CreateNoteRequest;
    use crate::storage::Database;

    #[tokio::test]
    async fn embed_note_ids_stores_real_note_text_embedding() {
        let db = Database::new(":memory:").unwrap();
        let note = db
            .create_note(CreateNoteRequest {
                title: "Protocol v2.3".into(),
                content: "washout is 14 days".into(),
                tags: vec![],
            })
            .unwrap();
        let backend: SharedBackend = Arc::new(MockBackend::new("unused"));
        let n = embed_note_ids(&db, &backend, &[note.id.clone()])
            .await
            .unwrap();
        assert_eq!(n, 1);

        let missing = db.list_note_ids_missing_embeddings().unwrap();
        assert!(
            missing.is_empty(),
            "note should have an embedding after embed_note_ids"
        );
    }
}

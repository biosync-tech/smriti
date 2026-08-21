//! Embedding Queue — background processor for auto-embedding notes
//!
//! When a note is created or updated, it gets queued for embedding.
//! A background Tokio task processes the queue in batches, generating
//! embeddings via the configured InferenceBackend and storing them
//! in sqlite-vec.

use std::sync::Arc;
use tokio::sync::RwLock;

use super::{InferenceError, SharedBackend};

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

/// The embedding queue processor
pub struct EmbeddingQueue {
    sender: tokio::sync::mpsc::Sender<EmbedMessage>,
    stats: Arc<RwLock<EmbedQueueStats>>,
}

impl EmbeddingQueue {
    /// Create a new embedding queue and spawn the background processor
    pub fn new(
        backend: SharedBackend,
        batch_size: usize,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<EmbedMessage>(1000);
        let stats = Arc::new(RwLock::new(EmbedQueueStats::default()));

        let processor_stats = stats.clone();

        // Spawn background processor
        tokio::spawn(async move {
            Self::process_loop(receiver, backend, batch_size, processor_stats).await;
        });

        Self { sender, stats }
    }

    /// Queue a note for embedding
    pub async fn enqueue(&self, note_id: String) -> Result<(), InferenceError> {
        self.sender
            .send(EmbedMessage::EmbedNote { note_id })
            .await
            .map_err(|e| {
                InferenceError::GenerationFailed(format!("Queue send failed: {}", e))
            })?;

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
            .map_err(|e| {
                InferenceError::GenerationFailed(format!("Queue send failed: {}", e))
            })?;

        let mut stats = self.stats.write().await;
        stats.pending += count;

        Ok(())
    }

    /// Trigger re-embedding of all notes
    pub async fn reembed_all(&self) -> Result<(), InferenceError> {
        self.sender
            .send(EmbedMessage::ReembedAll)
            .await
            .map_err(|e| {
                InferenceError::GenerationFailed(format!("Queue send failed: {}", e))
            })?;
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

    /// Background processing loop
    async fn process_loop(
        mut receiver: tokio::sync::mpsc::Receiver<EmbedMessage>,
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
            // Collect messages until we have a full batch or channel is empty
            match tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv()).await {
                Ok(Some(msg)) => {
                    match msg {
                        EmbedMessage::EmbedNote { note_id } => {
                            pending_ids.push(note_id);
                        }
                        EmbedMessage::EmbedBatch { note_ids } => {
                            pending_ids.extend(note_ids);
                        }
                        EmbedMessage::ReembedAll => {
                            tracing::info!("Re-embed all notes message received");
                            // In a real scenario, this would query the database for all notes
                            // For now, we just log it
                        }
                        EmbedMessage::Shutdown => {
                            tracing::info!("Embedding queue shutting down");
                            break;
                        }
                    }
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout — process whatever we have
                }
            }

            // Process batch if we have enough
            if pending_ids.len() >= batch_size
                || (!pending_ids.is_empty() && pending_ids.len() < batch_size)
            {
                let batch: Vec<String> = pending_ids
                    .drain(..pending_ids.len().min(batch_size))
                    .collect();

                if let Err(e) = Self::process_batch(&backend, &batch, &stats).await {
                    tracing::error!("Embedding batch failed: {}", e);
                    let mut s = stats.write().await;
                    s.failed += batch.len();
                    s.last_error = Some(e.to_string());
                }
            }
        }
    }

    /// Process a batch of notes
    async fn process_batch(
        backend: &SharedBackend,
        note_ids: &[String],
        stats: &Arc<RwLock<EmbedQueueStats>>,
    ) -> Result<(), InferenceError> {
        if note_ids.is_empty() {
            return Ok(());
        }

        // In a real scenario, we would fetch note contents here
        // For now, we'll just demonstrate the structure
        let mut texts: Vec<(String, String)> = Vec::new();
        for id in note_ids {
            // In production, fetch from database
            let text = format!("Note: {}", id);
            texts.push((id.clone(), text));
        }

        // Generate embeddings
        let text_strs: Vec<String> = texts.iter().map(|(_, t)| t.clone()).collect();
        let _embeddings = backend.embed(&text_strs).await?;

        // In a real scenario, store embeddings in database here
        // db.store_embedding(&note_id, &embedding, ...)?;

        // Update stats
        let mut s = stats.write().await;
        let count = texts.len();
        s.processed += count;
        if s.pending >= count {
            s.pending -= count;
        }

        tracing::debug!("Embedded {} notes (pending: {})", count, s.pending);

        Ok(())
    }
}

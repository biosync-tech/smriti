//! Multimodal Ingestion — create notes from images and other media
//!
//! Uses Gemma 4's vision capabilities to describe images and
//! automatically create notes from them.

use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::inference::{InferenceError, SharedBackend};
use crate::models::*;
use crate::storage::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestImageRequest {
    /// Optional title (auto-generated if not provided)
    pub title: Option<String>,
    /// Additional context or instructions
    pub context: Option<String>,
    /// Tags to apply
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResponse {
    pub note_id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub model: String,
}

pub struct MultimodalIngestor {
    db: Arc<Database>,
    backend: SharedBackend,
}

impl MultimodalIngestor {
    pub fn new(db: Arc<Database>, backend: SharedBackend) -> Self {
        Self { db, backend }
    }

    /// Create a note from an image using Gemma 4 vision
    pub async fn ingest_image(
        &self,
        image_bytes: &[u8],
        request: &IngestImageRequest,
    ) -> Result<IngestResponse, InferenceError> {
        // Use Gemma 4's vision capability to describe the image
        let prompt = match &request.context {
            Some(ctx) => format!(
                "Describe this image in detail. Create a knowledge note about it. \
                 Additional context: {}. \
                 Format: Start with a brief title line, then a detailed description.",
                ctx
            ),
            None => "Describe this image in detail. Create a knowledge note about it. \
                     Format: Start with a brief title line, then a detailed description."
                .into(),
        };

        let description = self
            .backend
            .describe_image(image_bytes, &prompt)
            .await?;

        // Extract title from description (first line) or use provided title
        let (title, content) = if let Some(ref t) = request.title {
            (t.clone(), description.clone())
        } else {
            // Try to split first line as title
            let lines: Vec<&str> = description.splitn(2, '\n').collect();
            if lines.len() > 1 {
                (lines[0].trim().to_string(), lines[1].trim().to_string())
            } else {
                ("Image Note".to_string(), description.clone())
            }
        };

        // Create the note
        let note = self
            .db
            .create_note(CreateNoteRequest {
                title: title.clone(),
                content: content.clone(),
                tags: request.tags.clone(),
            })
            .map_err(|e| InferenceError::GenerationFailed(format!("Failed to create note: {}", e)))?;

        Ok(IngestResponse {
            note_id: note.id,
            title,
            description: content,
            tags: request.tags.clone(),
            model: self.backend.name().to_string(),
        })
    }
}

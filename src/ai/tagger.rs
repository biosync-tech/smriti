//! Auto-Tagging — AI-suggested tags for notes

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::inference::{AuditedInference, CallContext, GenerateRequest, InferenceError};
use crate::storage::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSuggestion {
    pub tag: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTagResponse {
    pub note_id: String,
    pub suggested_tags: Vec<TagSuggestion>,
    pub existing_tags: Vec<String>,
    pub model: String,
}

pub struct AutoTagger {
    db: Arc<Database>,
    backend: Arc<AuditedInference>,
}

impl AutoTagger {
    pub fn new(db: Arc<Database>, backend: Arc<AuditedInference>) -> Self {
        Self { db, backend }
    }

    /// Suggest tags for a note based on its content
    pub async fn suggest_tags(
        &self,
        note_id: &str,
        max_tags: usize,
    ) -> Result<AutoTagResponse, InferenceError> {
        let note = self
            .db
            .get_note(note_id)
            .map_err(|e| InferenceError::GenerationFailed(format!("Note not found: {}", e)))?;

        let system = format!(
            "You are Smriti AI. Suggest up to {} tags for the following note. \
             Return ONLY a JSON array of objects with fields: tag (lowercase, hyphenated), \
             confidence (0.0-1.0), reason (brief explanation). \
             Example: [{{\"tag\": \"machine-learning\", \"confidence\": 0.9, \"reason\": \"discusses neural networks\"}}]. \
             Tags should be concise (1-3 words, hyphenated).",
            max_tags
        );

        let prompt = format!("# {}\n\n{}", note.title, note.content);

        let request = GenerateRequest {
            prompt,
            system: Some(system),
            max_tokens: Some(512),
            temperature: Some(0.2),
            top_p: Some(0.9),
            stop: None,
            thinking: None,
        };

        let ctx = CallContext::new("notes_categorize")
            .with_notes(vec![note_id.to_string()])
            .with_template("categorize@v1");
        let response = self.backend.generate_audited(&request, &ctx).await?;

        // Parse JSON response
        let suggested_tags = Self::parse_tag_suggestions(&response.text).unwrap_or_default();

        Ok(AutoTagResponse {
            note_id: note_id.to_string(),
            suggested_tags,
            existing_tags: note.tags,
            model: response.model,
        })
    }

    fn parse_tag_suggestions(text: &str) -> Option<Vec<TagSuggestion>> {
        // Try to find JSON array in the response
        let start = text.find('[')?;
        let end = text.rfind(']')? + 1;
        let json_str = &text[start..end];

        serde_json::from_str(json_str).ok()
    }
}

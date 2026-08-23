//! Note Summarization — AI-generated summaries of notes or note groups

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::inference::{AuditedInference, CallContext, GenerateRequest, InferenceError};
use crate::storage::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeRequest {
    /// Note IDs to summarize
    pub note_ids: Vec<String>,
    /// Maximum length of summary in tokens
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Style: "brief", "detailed", "bullet_points"
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_max_tokens() -> u32 {
    512
}
fn default_style() -> String {
    "brief".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeResponse {
    pub summary: String,
    pub note_count: usize,
    pub model: String,
}

pub struct Summarizer {
    db: Arc<Database>,
    backend: Arc<AuditedInference>,
}

impl Summarizer {
    pub fn new(db: Arc<Database>, backend: Arc<AuditedInference>) -> Self {
        Self { db, backend }
    }

    /// Summarize one or more notes
    pub async fn summarize(
        &self,
        request: &SummarizeRequest,
    ) -> Result<SummarizeResponse, InferenceError> {
        // Fetch all notes
        let mut notes_content = Vec::new();
        for id in &request.note_ids {
            match self.db.get_note(id) {
                Ok(note) => {
                    notes_content.push(format!("## {}\n{}", note.title, note.content));
                }
                Err(e) => {
                    tracing::warn!("Skipping note {}: {}", id, e);
                }
            }
        }

        if notes_content.is_empty() {
            return Err(InferenceError::GenerationFailed("No notes found".into()));
        }

        let style_instruction = match request.style.as_str() {
            "detailed" => {
                "Provide a detailed summary that captures all key points, arguments, and insights."
            }
            "bullet_points" => "Provide a summary as a bulleted list of key points.",
            _ => "Provide a brief, concise summary capturing the essence of the content.",
        };

        let system = format!(
            "You are Smriti AI. Summarize the following notes from the user's knowledge graph. \
             {}. If the notes are related, highlight connections between them.",
            style_instruction
        );

        let prompt = notes_content.join("\n\n---\n\n");

        let gen_request = GenerateRequest {
            prompt,
            system: Some(system),
            max_tokens: Some(request.max_tokens),
            temperature: Some(0.3),
            top_p: Some(0.9),
            stop: None,
            thinking: None,
        };

        let ctx = CallContext::new("notes_summarize")
            .with_notes(request.note_ids.clone())
            .with_template("summarize@v1");
        let response = self.backend.generate_audited(&gen_request, &ctx).await?;

        Ok(SummarizeResponse {
            summary: response.text,
            note_count: notes_content.len(),
            model: response.model,
        })
    }
}

#[cfg(test)]
mod e2e_audit_tests {
    use super::*;
    use crate::inference::audited::AuditedInference;
    use crate::inference::mock::MockBackend;
    use crate::models::note::CreateNoteRequest;
    use crate::storage::Database;
    use std::sync::Arc;

    #[tokio::test]
    async fn summarize_writes_audit_chain() {
        // Setup: in-memory DB with a seeded note for the summarizer to fetch
        let db = Arc::new(Database::new(":memory:").unwrap());

        // Create a note to summarize.
        // NOTE: create_note does NOT write an events row, so we seed one explicitly
        // to ensure the LLM call is NOT the first event in the chain. This is the
        // condition that previously masked Issue #1 — a fresh DB always passes the
        // chain walk because the first event's prev_hash=NULL matches last_hash=None.
        let note_req = CreateNoteRequest {
            title: "Test Note".into(),
            content: "Hello world. This is a test note for summarization.".into(),
            tags: vec![],
        };
        let note = db.create_note(note_req).expect("create note");
        let note_id = note.id.clone();

        // Seed a prior event so the chain has at least one row before the LLM call.
        // This ensures that when generate_audited runs, last_hash is Some("...") and
        // the llm_call event MUST carry a matching prev_hash or the chain will break.
        db.append_event(
            "note.created",
            "note",
            &note_id,
            &serde_json::json!({"title": "Test Note"}),
            None,
            None,
        )
        .expect("seed prior event");

        // Setup audited inference with mock backend
        let mock = Arc::new(MockBackend::new("This is a summary."));
        let audited = Arc::new(AuditedInference::new(mock, db.clone()));

        let summarizer = Summarizer::new(db.clone(), audited);
        let req = SummarizeRequest {
            note_ids: vec![note_id],
            max_tokens: 100,
            style: "brief".into(),
        };
        let res = summarizer.summarize(&req).await.unwrap();
        assert_eq!(res.summary, "This is a summary.");

        // Verify audit chain: one llm_call event + one llm_audit row
        {
            let conn = db.conn.lock().unwrap();

            let event_count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM events WHERE event_type='llm_call'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(event_count, 1, "expected one llm_call event");

            let (audit_count, tool_name, outcome): (i64, String, String) = conn
                .query_row(
                    "SELECT count(*), max(tool_name), max(outcome) FROM llm_audit",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(audit_count, 1, "expected one llm_audit row");
            assert_eq!(tool_name, "notes_summarize");
            assert_eq!(outcome, "success");
        } // release conn lock before calling db.verify()

        // Full chain integrity check: every prev_hash must match the preceding
        // row's event_hash. With a prior seeded event, this catches the raw-INSERT
        // bug (Issue #1) where prev_hash was NULL on a non-first event.
        let report = db.verify().expect("verify should not error");
        assert!(
            report.event_chain_errors.is_empty(),
            "event chain broken: {:?}",
            report.event_chain_errors
        );
    }
}

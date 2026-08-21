//! AuditedInference — wraps any InferenceBackend with hash-chained audit
//! logging. Every call writes one row to events (event_type='llm_call')
//! and one row to llm_audit (denormalized for queries).
//!
//! Spec: docs/superpowers/specs/2026-05-02-llm-integration-design.md §5

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing;
use uuid::Uuid;

use super::{
    BackendCapabilities, BackendStatus, GenerateRequest, GenerateResponse, InferenceBackend,
    InferenceError,
};
use crate::models::llm_audit::{LlmAuditRow, LlmCallEvent, LlmOutcome};
use crate::storage::Database;

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn hash_request(req: &GenerateRequest) -> String {
    // Deterministic concatenation. Only fields that affect output are included.
    let mut s = String::new();
    s.push_str("prompt:"); s.push_str(&req.prompt); s.push('\n');
    if let Some(sys) = &req.system { s.push_str("sys:"); s.push_str(sys); s.push('\n'); }
    if let Some(t) = req.temperature { s.push_str(&format!("t:{}\n", t)); }
    if let Some(m) = req.max_tokens { s.push_str(&format!("m:{}\n", m)); }
    sha256_hex(&s)
}

/// Context the caller supplies for every audited call. Identifies who/why.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallContext {
    pub agent_id: String,
    pub tool_name: String,
    pub note_ids: Vec<String>,
    pub prompt_template_version: String,
}

impl CallContext {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            agent_id: "default".into(),
            tool_name: tool_name.into(),
            note_ids: Vec::new(),
            prompt_template_version: "v1".into(),
        }
    }
    pub fn with_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = agent_id.into();
        self
    }
    pub fn with_notes(mut self, note_ids: Vec<String>) -> Self {
        self.note_ids = note_ids;
        self
    }
    pub fn with_template(mut self, v: impl Into<String>) -> Self {
        self.prompt_template_version = v.into();
        self
    }
}

pub struct AuditedInference {
    inner: Arc<dyn InferenceBackend>,
    db: Arc<Database>,
}

impl AuditedInference {
    pub fn new(inner: Arc<dyn InferenceBackend>, db: Arc<Database>) -> Self {
        Self { inner, db }
    }

    /// Audited generate. The plain InferenceBackend trait method exists for
    /// callers that don't have a CallContext yet — those callers should be
    /// migrated to `generate_audited` over time.
    pub async fn generate_audited(
        &self,
        req: &GenerateRequest,
        ctx: &CallContext,
    ) -> Result<GenerateResponse, InferenceError> {
        let started = std::time::Instant::now();
        let prompt_hash = hash_request(req);
        let result = self.inner.generate(req).await;
        let duration_ms = started.elapsed().as_millis() as u64;

        let (response_hash, model, prompt_tokens, completion_tokens, outcome, err) = match &result {
            Ok(r) => (
                Some(sha256_hex(&r.text)),
                r.model.clone(),
                r.tokens_used.as_ref().map(|t| t.prompt_tokens),
                r.tokens_used.as_ref().map(|t| t.completion_tokens),
                LlmOutcome::Success,
                None,
            ),
            Err(e) => (
                None,
                self.inner.name().to_string(),
                None,
                None,
                LlmOutcome::Error,
                Some(e.to_string()),
            ),
        };

        let event_id = Uuid::new_v4().to_string();
        let audit_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let event_payload = LlmCallEvent {
            id: audit_id.clone(),
            agent_id: ctx.agent_id.clone(),
            tool_name: ctx.tool_name.clone(),
            model: model.clone(),
            prompt_hash: prompt_hash.clone(),
            response_hash: response_hash.clone(),
            prompt_template_version: ctx.prompt_template_version.clone(),
            note_ids: ctx.note_ids.clone(),
            temperature: req.temperature.unwrap_or(0.0),
            seed: None,
            prompt_tokens,
            completion_tokens,
            duration_ms,
            outcome: outcome.clone(),
            error_message: err.clone(),
            created_at: now,
        };

        let payload_value = match serde_json::to_value(&event_payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("LLM audit serialize failed: {}", e);
                // Return original LLM result without writing audit rows
                return result;
            }
        };

        // Write events row via the canonical append_event helper, which correctly
        // looks up the previous event_hash and sets prev_hash for chain integrity.
        // Best-effort: failure logs a warning and does NOT fail the LLM call.
        let actual_event_id = match self.db.append_event(
            "llm_call",
            "llm",
            &ctx.agent_id,
            &payload_value,
            Some(ctx.agent_id.as_str()),
            None,
        ) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("LLM audit events insert failed: {}", e);
                // Still attempt the denormalized row with the pre-generated event_id
                // (it may fail on the FK, which is acceptable under best-effort).
                event_id
            }
        };

        // Denormalized row — best-effort: a DB failure does NOT fail the LLM call
        let row = LlmAuditRow {
            id: audit_id,
            event_id: actual_event_id,
            agent_id: ctx.agent_id.clone(),
            tool_name: ctx.tool_name.clone(),
            model,
            prompt_hash,
            response_hash,
            prompt_template_version: ctx.prompt_template_version.clone(),
            note_ids: ctx.note_ids.clone(),
            temperature: req.temperature.unwrap_or(0.0),
            seed: None,
            prompt_tokens,
            completion_tokens,
            duration_ms,
            outcome,
            error_message: err,
            created_at: now,
        };
        if let Err(e) = self.db.insert_llm_audit_row(&row) {
            tracing::warn!("LLM audit row insert failed: {}", e);
        }

        result
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        // Embeddings are not currently audited (low PHI, high volume).
        self.inner.embed(texts).await
    }
}

// Pass-through trait impl so existing callers can swap SharedBackend → Arc<AuditedInference>
// without changing call sites.
#[async_trait]
impl InferenceBackend for AuditedInference {
    async fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        // No-context calls pass through unaudited. Real callers are migrated
        // to generate_audited in subsequent tasks.
        self.inner.generate(req).await
    }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        self.inner.embed(texts).await
    }
    async fn describe_image(&self, b: &[u8], p: &str) -> Result<String, InferenceError> {
        self.inner.describe_image(b, p).await
    }
    fn capabilities(&self) -> BackendCapabilities {
        self.inner.capabilities()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        self.inner.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::mock::MockBackend;

    #[tokio::test]
    async fn passthrough_generate_returns_inner_response() {
        let mock = Arc::new(MockBackend::new("hello"));
        let db = Arc::new(Database::new(":memory:").unwrap());
        let audited = AuditedInference::new(mock.clone(), db);
        let req = GenerateRequest::default();
        let res = audited
            .generate_audited(&req, &CallContext::new("tool"))
            .await
            .unwrap();
        assert_eq!(res.text, "hello");
        assert_eq!(mock.calls(), 1);
    }

    #[tokio::test]
    async fn call_context_builder_chain() {
        let ctx = CallContext::new("notes_summarize")
            .with_agent("agent-456")
            .with_notes(vec!["n1".to_string(), "n2".to_string()])
            .with_template("summarize@v2");
        assert_eq!(ctx.agent_id, "agent-456");
        assert_eq!(ctx.tool_name, "notes_summarize");
        assert_eq!(ctx.note_ids, vec!["n1".to_string(), "n2".to_string()]);
        assert_eq!(ctx.prompt_template_version, "summarize@v2");
    }

    #[tokio::test]
    async fn audited_implements_backend_trait() {
        let mock = Arc::new(MockBackend::new("test"));
        let db = Arc::new(Database::new(":memory:").unwrap());
        let audited = AuditedInference::new(mock, db);

        // Direct trait calls should work (pass-through)
        let req = GenerateRequest::default();
        let res = audited.generate(&req).await.unwrap();
        assert_eq!(res.text, "test");

        // Capabilities available
        let caps = audited.capabilities();
        assert!(caps.text_generation);

        // Name available
        assert_eq!(audited.name(), "mock");
    }

    #[tokio::test]
    async fn generate_audited_writes_llm_audit_row() {
        let mock = Arc::new(MockBackend::new("hi"));
        let db = Arc::new(Database::new(":memory:").unwrap());
        let audited = AuditedInference::new(mock.clone(), db.clone());
        let req = GenerateRequest {
            prompt: "test prompt".into(),
            system: None, max_tokens: None, temperature: Some(0.0),
            top_p: None, stop: None, thinking: None,
        };
        let ctx = CallContext::new("notes_summarize")
            .with_agent("agent_x")
            .with_notes(vec!["n1".into()]);

        let _res = audited.generate_audited(&req, &ctx).await.unwrap();

        let conn = db.conn.lock().unwrap();
        let (count, agent, tool, model, outcome): (i64, String, String, String, String) = conn
            .query_row(
                "SELECT count(*), max(agent_id), max(tool_name), max(model), max(outcome)
                 FROM llm_audit",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(agent, "agent_x");
        assert_eq!(tool, "notes_summarize");
        assert_eq!(model, "mock:v1");
        assert_eq!(outcome, "success");
    }

    #[tokio::test]
    async fn generate_audited_writes_events_row() {
        let mock = Arc::new(MockBackend::new("hi"));
        let db = Arc::new(Database::new(":memory:").unwrap());
        let audited = AuditedInference::new(mock, db.clone());
        let req = GenerateRequest {
            prompt: "p".into(), system: None, max_tokens: None,
            temperature: Some(0.0), top_p: None, stop: None, thinking: None,
        };
        let _ = audited.generate_audited(&req, &CallContext::new("t1")).await.unwrap();

        let conn = db.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM events WHERE event_type='llm_call'",
            [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}

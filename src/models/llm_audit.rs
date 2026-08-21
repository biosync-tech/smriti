//! LLM audit types — payload format for `event_type='llm_call'` events
//! and the denormalized `llm_audit` table row shape.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Outcome of a single LLM call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmOutcome {
    Success,
    Error,
    Timeout,
    InvalidJson,
}

impl LlmOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmOutcome::Success => "success",
            LlmOutcome::Error => "error",
            LlmOutcome::Timeout => "timeout",
            LlmOutcome::InvalidJson => "invalid_json",
        }
    }
}

/// Payload for `events.event_type='llm_call'`. This is the canonical
/// hash-chained record. `LlmAuditRow` denormalizes a subset for query
/// performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallEvent {
    pub id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub model: String,
    pub prompt_hash: String,
    pub response_hash: Option<String>,
    pub prompt_template_version: String,
    pub note_ids: Vec<String>,
    pub temperature: f32,
    pub seed: Option<u64>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub duration_ms: u64,
    pub outcome: LlmOutcome,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One row of the `llm_audit` denormalization table. Mirrors columns
/// in Migration 010; `event_id` links back to the canonical events row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAuditRow {
    pub id: String,
    pub event_id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub model: String,
    pub prompt_hash: String,
    pub response_hash: Option<String>,
    pub prompt_template_version: String,
    pub note_ids: Vec<String>,
    pub temperature: f32,
    pub seed: Option<u64>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub duration_ms: u64,
    pub outcome: LlmOutcome,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_serializes_snake_case() {
        let s = serde_json::to_string(&LlmOutcome::InvalidJson).unwrap();
        assert_eq!(s, "\"invalid_json\"");
    }

    #[test]
    fn outcome_as_str_matches_serde() {
        assert_eq!(LlmOutcome::Success.as_str(), "success");
        assert_eq!(LlmOutcome::Error.as_str(), "error");
        assert_eq!(LlmOutcome::Timeout.as_str(), "timeout");
        assert_eq!(LlmOutcome::InvalidJson.as_str(), "invalid_json");
    }

    #[test]
    fn llm_call_event_roundtrips() {
        let ev = LlmCallEvent {
            id: "ev1".into(),
            agent_id: "agent1".into(),
            tool_name: "notes_summarize".into(),
            model: "ollama:llama3.1:8b".into(),
            prompt_hash: "abc".into(),
            response_hash: Some("def".into()),
            prompt_template_version: "summarize@v1".into(),
            note_ids: vec!["n1".into(), "n2".into()],
            temperature: 0.0,
            seed: Some(42),
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            duration_ms: 1234,
            outcome: LlmOutcome::Success,
            error_message: None,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: LlmCallEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tool_name, "notes_summarize");
        assert_eq!(back.note_ids.len(), 2);
        assert_eq!(back.outcome.as_str(), "success");
    }
}

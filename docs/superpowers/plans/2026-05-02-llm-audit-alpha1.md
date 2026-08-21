# LLM Audit Layer (v0.3.0-alpha.1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire every existing AI feature call through a hash-chained audit layer, so every LLM invocation is reproducible by an FDA / sponsor auditor.

**Architecture:** Add an `AuditedInference` wrapper around the existing `InferenceBackend` trait. Every call writes one row to the existing hash-chained `events` table (`event_type='llm_call'`) plus a denormalized row to a new `llm_audit` table for query performance. Refactor `AiAppState` and the four existing feature modules (`summarizer`, `rag`, `tagger`, `linker`) to take `Arc<AuditedInference>` instead of `SharedBackend`.

**Tech Stack:** Rust + Tokio + rusqlite + thiserror + serde + sha2 (existing dependencies; no new crates).

**Spec:** `docs/superpowers/specs/2026-05-02-llm-integration-design.md`

**What this plan does NOT do** (deferred to alpha.2 or later):
- New MCP tool exposure (alpha.2)
- Output landing through `wiki_transactions` / `agent_memory` / hallucination guards (alpha.2)
- New feature module D (`deviations.rs`) (rc.1)

After this plan ships: every existing REST `/api/v1/ai/*` call still works identically from the outside, but each call now produces a hash-chained audit row.

---

## File structure

| File | Status | Responsibility |
|---|---|---|
| `src/inference/audited.rs` | NEW | `AuditedInference` wrapper struct + `CallContext` |
| `src/inference/mock.rs` | NEW (`#[cfg(test)]`) | `MockInferenceBackend` for unit tests |
| `src/inference/mod.rs` | MODIFY | `pub mod audited; pub use audited::*;` |
| `src/models/llm_audit.rs` | NEW | `LlmCallEvent` and `LlmAuditRow` types (serde) |
| `src/models/mod.rs` | MODIFY | `pub mod llm_audit;` |
| `src/storage/db.rs` | MODIFY | Migration 010 — `llm_audit` table DDL |
| `src/storage/operations.rs` | MODIFY | `Database::insert_llm_audit_row()` helper |
| `src/ai/summarizer.rs` | MODIFY | Take `Arc<AuditedInference>` |
| `src/ai/rag.rs` | MODIFY | Take `Arc<AuditedInference>` |
| `src/ai/tagger.rs` | MODIFY | Take `Arc<AuditedInference>` |
| `src/ai/linker.rs` | MODIFY | Take `Arc<AuditedInference>` |
| `src/api/server.rs` | MODIFY | `AiAppState.backend` becomes `Arc<AuditedInference>` |
| `src/api/routes/ai.rs` | MODIFY | Construct features with the wrapper |
| `src/main.rs` | MODIFY | Wire `AuditedInference` at startup |
| `tests/llm_audit_chain.rs` | NEW | Integration test: audit row + chain integrity |

---

## Task 1: MockInferenceBackend

**Files:**
- Create: `src/inference/mock.rs`
- Modify: `src/inference/mod.rs:13-19` (add `pub mod mock;`)

This is the test helper Tasks 3-9 depend on. Returns canned responses; lets us test the wrapper without a real LLM.

- [ ] **Step 1: Write the failing test**

Create `src/inference/mock.rs`:

```rust
//! Mock inference backend for tests — returns canned responses.
#![cfg(test)]

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use super::{
    BackendCapabilities, BackendStatus, GenerateRequest, GenerateResponse, InferenceBackend,
    InferenceError, TokenUsage,
};

#[derive(Clone)]
pub struct MockBackend {
    pub canned_text: Arc<Mutex<String>>,
    pub call_count: Arc<Mutex<u32>>,
    pub canned_embedding: Vec<f32>,
}

impl MockBackend {
    pub fn new(canned_text: &str) -> Self {
        Self {
            canned_text: Arc::new(Mutex::new(canned_text.to_string())),
            call_count: Arc::new(Mutex::new(0)),
            canned_embedding: vec![0.1; 384],
        }
    }
    pub fn calls(&self) -> u32 {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl InferenceBackend for MockBackend {
    async fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        *self.call_count.lock().unwrap() += 1;
        Ok(GenerateResponse {
            text: self.canned_text.lock().unwrap().clone(),
            model: "mock:v1".into(),
            tokens_used: Some(TokenUsage { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 }),
            finish_reason: super::FinishReason::Stop,
        })
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        Ok(texts.iter().map(|_| self.canned_embedding.clone()).collect())
    }

    async fn describe_image(&self, _bytes: &[u8], _prompt: &str) -> Result<String, InferenceError> {
        Ok("mock image description".into())
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_generation: true,
            supports_embeddings: true,
            supports_vision: false,
            max_context_tokens: 4096,
            embedding_dim: 384,
        }
    }

    fn name(&self) -> &str { "mock" }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        Ok(BackendStatus { healthy: true, model_loaded: true, message: "mock ok".into() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_backend_returns_canned_text() {
        let m = MockBackend::new("hello");
        let req = GenerateRequest {
            prompt: "anything".into(),
            system: None, max_tokens: None, temperature: None, top_p: None,
            stop: None, thinking: None,
        };
        let res = m.generate(&req).await.unwrap();
        assert_eq!(res.text, "hello");
        assert_eq!(m.calls(), 1);
    }
}
```

- [ ] **Step 2: Wire the module**

In `src/inference/mod.rs`, locate the line `pub mod queue;` (around line 19) and add immediately after:

```rust
#[cfg(test)]
pub mod mock;
```

- [ ] **Step 3: Run the test**

```
cargo test --lib inference::mock::tests::mock_backend_returns_canned_text
```

Expected: `test result: ok. 1 passed`

If this fails because field names don't match (e.g. `BackendCapabilities` has different fields), open `src/inference/mod.rs` and adjust the `MockBackend::capabilities()` and the `BackendStatus` literal in `health_check()` to match the real struct definitions exactly. Re-run.

- [ ] **Step 4: Commit**

```
git add src/inference/mock.rs src/inference/mod.rs
git commit -m "test: add MockInferenceBackend for audit-layer tests"
```

---

## Task 2: Migration 010 — `llm_audit` table

**Files:**
- Modify: `src/storage/db.rs` (append after Migration 009 block, before `apply_migrations` closing brace)

- [ ] **Step 1: Write the failing test**

Append to `src/storage/db.rs` (inside any existing `#[cfg(test)] mod tests` block, or add one):

```rust
#[cfg(test)]
mod migration_010_tests {
    use super::Database;

    #[test]
    fn llm_audit_table_exists_after_migration() {
        let db = Database::new(":memory:").expect("open in-memory db");
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='llm_audit'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "llm_audit table should exist after migration 010");
    }

    #[test]
    fn llm_audit_indexes_exist() {
        let db = Database::new(":memory:").unwrap();
        let conn = db.conn.lock().unwrap();
        let agent_idx: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_llm_audit_agent_time'",
            [], |row| row.get(0)).unwrap();
        let tool_idx: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_llm_audit_tool_time'",
            [], |row| row.get(0)).unwrap();
        assert_eq!(agent_idx, 1);
        assert_eq!(tool_idx, 1);
    }
}
```

- [ ] **Step 2: Run the test — expect failure**

```
cargo test --lib storage::db::migration_010_tests
```

Expected: 2 tests fail with `assertion `left == right` failed: left: 0, right: 1`.

- [ ] **Step 3: Implement Migration 010**

In `src/storage/db.rs`, find the end of the Migration 009 block (search for `idx_access_note_time` near the end of `apply_migrations`). Immediately before the closing brace `}` of the `apply_migrations` function, paste:

```rust
        // ── Migration 010: LLM audit denormalization ──────────────────────
        // Spec: docs/superpowers/specs/2026-05-02-llm-integration-design.md §6
        //
        // Every LLM call writes one row to the existing `events` table
        // (event_type='llm_call') for hash-chain integrity. This table is a
        // query-performance denormalization — without it, "show all LLM calls
        // by agent X" requires walking JSON payloads in events.
        //
        // Stores hashes only (not raw prompts/responses) to minimize PHI
        // surface. Re-derivation: same notes + same prompt template version +
        // same temperature + same seed + same model = same hash.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS llm_audit (
                id                       TEXT PRIMARY KEY,
                event_id                 TEXT NOT NULL REFERENCES events(id),
                agent_id                 TEXT NOT NULL,
                tool_name                TEXT NOT NULL,
                model                    TEXT NOT NULL,
                prompt_hash              TEXT NOT NULL,
                response_hash            TEXT,
                prompt_template_version  TEXT NOT NULL,
                note_ids                 TEXT NOT NULL,
                temperature              REAL NOT NULL,
                seed                     INTEGER,
                prompt_tokens            INTEGER,
                completion_tokens        INTEGER,
                duration_ms              INTEGER NOT NULL,
                outcome                  TEXT NOT NULL,
                error_message            TEXT,
                created_at               TEXT NOT NULL
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_llm_audit_agent_time
             ON llm_audit(agent_id, created_at DESC);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_llm_audit_tool_time
             ON llm_audit(tool_name, created_at DESC);",
        );
```

- [ ] **Step 4: Run the test — expect pass**

```
cargo test --lib storage::db::migration_010_tests
```

Expected: 2 tests pass.

- [ ] **Step 5: Run the full test suite to confirm nothing else broke**

```
cargo test --lib
```

Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```
git add src/storage/db.rs
git commit -m "feat(storage): migration 010 — llm_audit denormalization table"
```

---

## Task 3: `LlmCallEvent` and `LlmAuditRow` types

**Files:**
- Create: `src/models/llm_audit.rs`
- Modify: `src/models/mod.rs:1` (add `pub mod llm_audit;`)

- [ ] **Step 1: Write the failing test**

Create `src/models/llm_audit.rs`:

```rust
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
    pub tool_name: String,                  // "notes_summarize" etc.
    pub model: String,                      // "ollama:llama3.1:8b"
    pub prompt_hash: String,                // sha256 hex
    pub response_hash: Option<String>,      // None on early failure
    pub prompt_template_version: String,    // "summarize@v1"
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

/// One row of the `llm_audit` denormalization table. Mirrors the columns
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
    pub note_ids: Vec<String>,              // serialized as JSON in DB
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
```

- [ ] **Step 2: Wire the module**

Open `src/models/mod.rs` and add at the top with the other `pub mod` lines:

```rust
pub mod llm_audit;
```

- [ ] **Step 3: Run tests**

```
cargo test --lib models::llm_audit::tests
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```
git add src/models/llm_audit.rs src/models/mod.rs
git commit -m "feat(models): LlmCallEvent and LlmAuditRow types for audit chain"
```

---

## Task 4: `Database::insert_llm_audit_row` helper

**Files:**
- Modify: `src/storage/operations.rs` (append a new impl block at the end)

- [ ] **Step 1: Write the failing test**

Add to `src/storage/operations.rs` at the bottom of the file:

```rust
#[cfg(test)]
mod llm_audit_op_tests {
    use crate::models::llm_audit::{LlmAuditRow, LlmOutcome};
    use crate::storage::Database;
    use chrono::Utc;

    #[test]
    fn insert_and_count_llm_audit_row() {
        let db = Database::new(":memory:").unwrap();
        // Need an events row to satisfy FK
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO events (id, event_type, entity_type, entity_id, payload, event_time, ingestion_time, event_hash)
             VALUES ('ev1', 'llm_call', 'llm', 'agent1', '{}', ?1, ?1, 'h1')",
            [Utc::now().to_rfc3339()],
        ).unwrap();
        drop(conn);

        let row = LlmAuditRow {
            id: "a1".into(),
            event_id: "ev1".into(),
            agent_id: "agent1".into(),
            tool_name: "notes_summarize".into(),
            model: "mock:v1".into(),
            prompt_hash: "abc".into(),
            response_hash: Some("def".into()),
            prompt_template_version: "summarize@v1".into(),
            note_ids: vec!["n1".into()],
            temperature: 0.0,
            seed: Some(42),
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            duration_ms: 100,
            outcome: LlmOutcome::Success,
            error_message: None,
            created_at: Utc::now(),
        };
        db.insert_llm_audit_row(&row).expect("insert");

        let conn = db.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT count(*) FROM llm_audit", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 2: Run — expect compile error**

```
cargo test --lib storage::operations::llm_audit_op_tests
```

Expected: compile error `no method named 'insert_llm_audit_row' found`.

- [ ] **Step 3: Implement the method**

Find the existing `impl Database` block in `src/storage/operations.rs` (there are likely several; pick one). Inside any `impl Database` block, add:

```rust
    /// Insert a denormalized LLM audit row. Caller is responsible for having
    /// already inserted the corresponding `events` row (FK enforced).
    pub fn insert_llm_audit_row(
        &self,
        row: &crate::models::llm_audit::LlmAuditRow,
    ) -> crate::errors::AppResult<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let note_ids_json = serde_json::to_string(&row.note_ids)?;
        conn.execute(
            "INSERT INTO llm_audit
                (id, event_id, agent_id, tool_name, model, prompt_hash, response_hash,
                 prompt_template_version, note_ids, temperature, seed, prompt_tokens,
                 completion_tokens, duration_ms, outcome, error_message, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            rusqlite::params![
                row.id,
                row.event_id,
                row.agent_id,
                row.tool_name,
                row.model,
                row.prompt_hash,
                row.response_hash,
                row.prompt_template_version,
                note_ids_json,
                row.temperature,
                row.seed,
                row.prompt_tokens,
                row.completion_tokens,
                row.duration_ms as i64,
                row.outcome.as_str(),
                row.error_message,
                row.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
```

- [ ] **Step 4: Run — expect pass**

```
cargo test --lib storage::operations::llm_audit_op_tests
```

Expected: 1 test passes.

- [ ] **Step 5: Commit**

```
git add src/storage/operations.rs
git commit -m "feat(storage): insert_llm_audit_row helper"
```

---

## Task 5: `AuditedInference` — call-through scaffold

**Files:**
- Create: `src/inference/audited.rs`
- Modify: `src/inference/mod.rs` (add `pub mod audited;` and re-exports)

This task adds the wrapper but does NOT yet write audit rows — that's Task 6. Splitting the work makes the diff easier to review.

- [ ] **Step 1: Write the failing test**

Create `src/inference/audited.rs`:

```rust
//! AuditedInference — wraps any InferenceBackend with hash-chained audit
//! logging. Every call writes one row to events (event_type='llm_call')
//! and one row to llm_audit (denormalized for queries).
//!
//! Spec: docs/superpowers/specs/2026-05-02-llm-integration-design.md §5

use std::sync::Arc;

use async_trait::async_trait;

use super::{
    BackendCapabilities, BackendStatus, GenerateRequest, GenerateResponse, InferenceBackend,
    InferenceError,
};
use crate::storage::Database;

/// Context the caller supplies for every audited call. Identifies who/why.
#[derive(Debug, Clone)]
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
        _ctx: &CallContext,
    ) -> Result<GenerateResponse, InferenceError> {
        // Audit logging arrives in Task 6. For now, just call through.
        self.inner.generate(req).await
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
    fn capabilities(&self) -> BackendCapabilities { self.inner.capabilities() }
    fn name(&self) -> &str { self.inner.name() }
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
        let req = GenerateRequest {
            prompt: "x".into(),
            system: None, max_tokens: None, temperature: None, top_p: None,
            stop: None, thinking: None,
        };
        let res = audited.generate_audited(&req, &CallContext::new("tool")).await.unwrap();
        assert_eq!(res.text, "hello");
        assert_eq!(mock.calls(), 1);
    }
}
```

- [ ] **Step 2: Wire the module**

In `src/inference/mod.rs`, after `pub mod queue;` add:

```rust
pub mod audited;
pub use audited::{AuditedInference, CallContext};
```

- [ ] **Step 3: Run the test**

```
cargo test --lib inference::audited::tests::passthrough_generate_returns_inner_response
```

Expected: 1 test passes. If it fails on `Database::new(":memory:")` because the function is private, surface a `#[cfg(test)]`-gated public alias or use the existing public constructor (it IS public per Task 2's test).

- [ ] **Step 4: Commit**

```
git add src/inference/audited.rs src/inference/mod.rs
git commit -m "feat(inference): AuditedInference wrapper scaffold (call-through only)"
```

---

## Task 6: `AuditedInference` — write hash-chained audit rows

**Files:**
- Modify: `src/inference/audited.rs` (replace `generate_audited` body)

- [ ] **Step 1: Write the failing test**

Add to `src/inference/audited.rs` inside the `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn generate_audited_writes_llm_audit_row() {
        use crate::storage::Database;
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
        use crate::storage::Database;
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
```

- [ ] **Step 2: Run — expect failures**

```
cargo test --lib inference::audited::tests
```

Expected: the two new tests fail (count=0 instead of 1).

- [ ] **Step 3: Add hashing helpers**

Near the top of `src/inference/audited.rs` (after the imports), add:

```rust
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
```

Add these crate imports to `src/inference/audited.rs`:

```rust
use chrono::Utc;
use uuid::Uuid;
use crate::models::llm_audit::{LlmAuditRow, LlmCallEvent, LlmOutcome};
```

- [ ] **Step 4: Implement audited generate**

Replace the existing `generate_audited` method body in `src/inference/audited.rs` with:

```rust
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

        let payload_json = serde_json::to_string(&event_payload)
            .map_err(|e| InferenceError::GenerationFailed(format!("audit serialize: {}", e)))?;
        let event_hash = sha256_hex(&format!("{}|{}|{}", event_id, "llm_call", payload_json));

        // Write events row (hash chain) — best-effort: log but do not fail the call
        if let Ok(conn) = self.db.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO events (id, event_type, entity_type, entity_id, payload,
                                     agent_id, event_time, ingestion_time, event_hash)
                 VALUES (?1,'llm_call','llm',?2,?3,?4,?5,?5,?6)",
                rusqlite::params![
                    event_id, ctx.agent_id, payload_json, ctx.agent_id,
                    now.to_rfc3339(), event_hash,
                ],
            );
        }

        // Denormalized row
        let row = LlmAuditRow {
            id: audit_id,
            event_id,
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
        let _ = self.db.insert_llm_audit_row(&row);

        result
    }
```

- [ ] **Step 5: Run — expect pass**

```
cargo test --lib inference::audited::tests
```

Expected: all 3 tests in the module pass.

- [ ] **Step 6: Run the whole test suite to confirm nothing else broke**

```
cargo test --lib
```

Expected: all existing tests still pass. (One known caveat: `Database::new` becomes called from this module's tests; if those exhaust connection pools or hit lock contention, run `cargo test --lib -- --test-threads=1`.)

- [ ] **Step 7: Commit**

```
git add src/inference/audited.rs
git commit -m "feat(inference): AuditedInference writes hash-chained audit rows"
```

---

## Task 7: Refactor `AiAppState` to hold `Arc<AuditedInference>`

**Files:**
- Modify: `src/api/server.rs` (the `AiAppState` struct definition)

The change is purely structural — `backend: SharedBackend` becomes `backend: Arc<AuditedInference>`. Because `AuditedInference` implements `InferenceBackend` (via the `#[async_trait] impl` in Task 5), all existing call sites compile unchanged.

- [ ] **Step 1: Read the current type definition**

Open `src/api/server.rs` lines 24-32 — confirm the `AiAppState` struct has `pub backend: SharedBackend`.

- [ ] **Step 2: Update the struct**

Replace the line `pub backend: SharedBackend,` in `AiAppState` with:

```rust
    pub backend: std::sync::Arc<crate::inference::AuditedInference>,
```

- [ ] **Step 3: Update the constructor in `create_ai_router`**

Find the `create_ai_router` function (around line 97). Its signature currently takes `backend: SharedBackend`. Change to:

```rust
pub fn create_ai_router(
    db: Arc<Database>,
    backend: Arc<crate::inference::AuditedInference>,
    inference_config: InferenceConfig,
    feature_gate: FeatureGate,
    auto_embedder: Option<Arc<AutoEmbedder>>,
) -> Router {
```

- [ ] **Step 4: Update `main.rs` to construct `AuditedInference`**

Open `src/main.rs`. Find where `create_ai_router` is called (probably the only place — search for `create_ai_router`). Before that call site, wrap the existing `backend` like so:

```rust
let audited_backend = std::sync::Arc::new(
    smriti::inference::AuditedInference::new(backend.clone(), db.clone())
);
// ... pass `audited_backend` to create_ai_router instead of `backend`
```

If `backend` was originally `SharedBackend` (which is `Arc<dyn InferenceBackend>`), this works because `AuditedInference::new(inner: Arc<dyn InferenceBackend>, db: Arc<Database>)` accepts that exact type.

- [ ] **Step 5: Compile**

```
cargo build
```

Expected: clean build. If errors mention type mismatches in callers of feature constructors (`Summarizer::new`, `RagEngine::new`, etc.), those are addressed in Task 8 — for now, the structural change should compile because `AuditedInference: InferenceBackend`.

If build fails because of `pub use audited::{AuditedInference, CallContext}` not being importable, double-check `src/inference/mod.rs` has the `pub use` line from Task 5.

- [ ] **Step 6: Run the full test suite**

```
cargo test --lib
```

Expected: all pass.

- [ ] **Step 7: Commit**

```
git add src/api/server.rs src/main.rs
git commit -m "refactor(api): AiAppState holds Arc<AuditedInference>"
```

---

## Task 8: Migrate feature modules to call `generate_audited`

**Files:**
- Modify: `src/ai/summarizer.rs`
- Modify: `src/ai/rag.rs`
- Modify: `src/ai/tagger.rs`
- Modify: `src/ai/linker.rs`

Each module currently calls `self.backend.generate(&request)`. We change those to `self.backend.generate_audited(&request, &ctx)`. The trait-level passthrough still works for callers that haven't migrated; this task migrates the four real call sites.

The `backend` field type changes from `SharedBackend` to `Arc<AuditedInference>` to expose `generate_audited`.

### 8a — `Summarizer`

- [ ] **Step 1: Update field type**

In `src/ai/summarizer.rs`, replace:

```rust
use crate::inference::{GenerateRequest, InferenceError, SharedBackend};
```

with:

```rust
use crate::inference::{AuditedInference, CallContext, GenerateRequest, InferenceError};
use std::sync::Arc;
```

(Note: `Arc` and `std::sync` already imported via `use std::sync::Arc;` at the top — keep whichever is there; add only what's missing.)

Replace `backend: SharedBackend` with `backend: Arc<AuditedInference>`. Replace the `Summarizer::new` signature `backend: SharedBackend` with `backend: Arc<AuditedInference>`.

- [ ] **Step 2: Update the call site**

Find the line `let response = self.backend.generate(&gen_request).await?;`. Replace with:

```rust
        let ctx = CallContext::new("notes_summarize")
            .with_notes(request.note_ids.clone())
            .with_template("summarize@v1");
        let response = self.backend.generate_audited(&gen_request, &ctx).await?;
```

- [ ] **Step 3: Compile**

```
cargo build
```

Expected: clean.

### 8b — `RagEngine`

- [ ] **Step 1: Update imports & field**

In `src/ai/rag.rs`, change:

```rust
use crate::inference::{GenerateRequest, InferenceError, SharedBackend};
```

to:

```rust
use crate::inference::{AuditedInference, CallContext, GenerateRequest, InferenceError};
use std::sync::Arc;
```

Replace `backend: SharedBackend` with `backend: Arc<AuditedInference>` in the struct definition and `new` signature.

- [ ] **Step 2: Update the generate call site**

Find `let response = self.backend.generate(&request).await?;` (near line 212). Replace with:

```rust
        let note_ids: Vec<String> = sources.iter().map(|s| s.note_id.clone()).collect();
        let ctx = CallContext::new("notes_ask")
            .with_notes(note_ids)
            .with_template("rag@v1");
        let response = self.backend.generate_audited(&request, &ctx).await?;
```

- [ ] **Step 3: Note**

The `embed` call (`self.backend.embed(...)`) does NOT change. Embeddings pass through the trait impl unaudited (per Task 5's design choice).

### 8c — `AutoTagger`

- [ ] **Step 1: Update imports & field**

In `src/ai/tagger.rs`, change:

```rust
use crate::inference::{GenerateRequest, InferenceError, SharedBackend};
```

to:

```rust
use crate::inference::{AuditedInference, CallContext, GenerateRequest, InferenceError};
use std::sync::Arc;
```

Replace `backend: SharedBackend` with `backend: Arc<AuditedInference>`.

- [ ] **Step 2: Update the call site**

Find `let response = self.backend.generate(&request).await?;` (near line 77). Replace with:

```rust
        let ctx = CallContext::new("notes_categorize")
            .with_notes(vec![note_id.to_string()])
            .with_template("categorize@v1");
        let response = self.backend.generate_audited(&request, &ctx).await?;
```

### 8d — `AiLinker`

- [ ] **Step 1: Update imports & field**

In `src/ai/linker.rs`, change:

```rust
use crate::inference::{InferenceError, SharedBackend};
```

to:

```rust
use crate::inference::{AuditedInference, InferenceError};
use std::sync::Arc;
```

Replace `backend: SharedBackend` with `backend: Arc<AuditedInference>`.

(`AiLinker` only calls `embed`, not `generate`, so no `generate_audited` migration is needed here. The structural type swap is the entire change.)

### 8e — Compile and run tests

- [ ] **Step 1: Build**

```
cargo build
```

Expected: clean. If `src/api/routes/ai.rs` has lines like `Summarizer::new(state.db.clone(), state.backend.clone())`, those should already compile because `state.backend` is now `Arc<AuditedInference>` (Task 7) and the constructors now expect that exact type.

- [ ] **Step 2: Run full test suite**

```
cargo test --lib
```

Expected: all pass.

- [ ] **Step 3: Commit**

```
git add src/ai/summarizer.rs src/ai/rag.rs src/ai/tagger.rs src/ai/linker.rs
git commit -m "refactor(ai): feature modules call generate_audited via AuditedInference"
```

---

## Task 9: End-to-end audit chain integration test

**Files:**
- Create: `tests/llm_audit_chain.rs`

- [ ] **Step 1: Write the test**

Create `tests/llm_audit_chain.rs`:

```rust
//! Integration test: confirm that calling Summarizer through AuditedInference
//! produces both an `events` row (event_type='llm_call') and an `llm_audit`
//! row, with consistent hashes.

use std::sync::Arc;

use smriti::ai::Summarizer;
use smriti::inference::{
    AuditedInference,
    mock::MockBackend,
};
use smriti::models::Note;
use smriti::storage::Database;

#[tokio::test]
async fn summarize_writes_audit_chain() {
    let db = Arc::new(Database::new(":memory:").unwrap());

    // Seed a note so summarizer can fetch it
    let note = Note::new("Test Note".into(), "Hello world".into());
    db.create_note(&note).unwrap();

    let mock = Arc::new(MockBackend::new("This is a summary."));
    let audited = Arc::new(AuditedInference::new(mock, db.clone()));

    let summarizer = Summarizer::new(db.clone(), audited);
    let req = smriti::ai::summarizer::SummarizeRequest {
        note_ids: vec![note.id.clone()],
        max_tokens: 100,
        style: "brief".into(),
    };
    let res = summarizer.summarize(&req).await.unwrap();
    assert_eq!(res.summary, "This is a summary.");

    let conn = db.conn.lock().unwrap();
    let event_count: i64 = conn.query_row(
        "SELECT count(*) FROM events WHERE event_type='llm_call'",
        [], |r| r.get(0)).unwrap();
    assert_eq!(event_count, 1, "expected one llm_call event");

    let (audit_count, tool_name, outcome): (i64, String, String) = conn.query_row(
        "SELECT count(*), max(tool_name), max(outcome) FROM llm_audit",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(audit_count, 1, "expected one llm_audit row");
    assert_eq!(tool_name, "notes_summarize");
    assert_eq!(outcome, "success");
}
```

- [ ] **Step 2: Verify the public-API surface this test uses exists**

The test uses:
- `smriti::ai::Summarizer` — ✅ public in `src/ai/mod.rs`
- `smriti::inference::AuditedInference` — ✅ public via re-export
- `smriti::inference::mock::MockBackend` — ⚠ may be `#[cfg(test)]`-only. If the build fails here, change `mock` to `pub mod mock;` (NOT `#[cfg(test)] pub mod mock;`) in `src/inference/mod.rs` AND remove the `#![cfg(test)]` from the top of `src/inference/mock.rs` so it's available to integration tests. The `MockBackend` is a test helper but integration tests need it accessible.
- `smriti::models::Note` — verify `Note::new(title, content)` constructor exists; if not, look at how existing tests construct notes and mirror that pattern.
- `db.create_note(&note)` — verify; if not, look for the actual public API for creating notes from the existing `tests/` directory.

- [ ] **Step 3: Run the test**

```
cargo test --test llm_audit_chain
```

Expected: passes. If the test fails due to API mismatches (Step 2 caveats), fix those without changing the assertion logic.

- [ ] **Step 4: Commit**

```
git add tests/llm_audit_chain.rs
# Plus any cfg-attribute changes from Step 2:
git add src/inference/mod.rs src/inference/mock.rs
git commit -m "test: end-to-end audit chain integration test"
```

---

## Task 10: Verify nothing else regressed

- [ ] **Step 1: Full test suite**

```
cargo test --all
```

Expected: all pass. If anything fails, the failure must be a real regression — fix it before proceeding.

- [ ] **Step 2: Build with default features**

```
cargo build
```

Expected: clean.

- [ ] **Step 3: Build with the webui feature**

```
cargo build --features webui
```

Expected: clean. (Feature was visible in `lib.rs:27`.)

- [ ] **Step 4: Confirm `wiki_verify` still walks the events chain successfully with the new `llm_call` event types mixed in**

```
cargo test --lib features::verify
```

Expected: pass. Existing verify logic hashes events by `(id, event_type, payload)` — it does not care about the specific event type, so adding `'llm_call'` to the mix should walk fine. If `verify` has a hardcoded allowlist of event types, surface that and add `'llm_call'` to it.

- [ ] **Step 5: Final commit message** (if any small fixes were needed in Step 4)

```
git add -p
git commit -m "fix: ensure wiki_verify walks llm_call events"
```

---

## Self-review checklist (run before declaring alpha.1 done)

- [ ] Migration 010 only adds — no existing schema mutations
- [ ] `AuditedInference::generate` (the trait method) is unaudited; only `generate_audited` writes rows. This is intentional — old call sites pass through, new call sites opt in by using the audited method.
- [ ] All four feature modules (`Summarizer`, `RagEngine`, `AutoTagger`, `AiLinker`) take `Arc<AuditedInference>` (compile-time enforcement of the architecture).
- [ ] `wiki_verify --chain` still passes with `llm_call` events mixed into `events`.
- [ ] No new crate dependencies in `Cargo.toml`.
- [ ] Spec §0 (current-state audit) and §12 (rollout phases) match this plan's scope.
- [ ] No `unwrap()` introduced in non-test code (CLAUDE.md guardrail #5). The audit writes use `let _ = ...` for best-effort logging; this is intentional — failure to write audit must NOT fail the LLM call itself, but the failure should be `tracing::warn!`-logged.

If any item fails self-review, fix it before declaring alpha.1 done.

---

## What ships at end of alpha.1

- Every existing `/api/v1/ai/{summarize,query,tag,link}` REST call still works identically.
- Each call now writes one `events` row (event_type='llm_call') and one `llm_audit` row.
- `wiki_verify --chain` walks the new events without any change to the verify logic.
- Foundation in place for alpha.2 to add MCP tools, integrity-layer landing, and citation validation.

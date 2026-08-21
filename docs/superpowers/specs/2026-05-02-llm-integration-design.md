# Smriti LLM Integration — Design Spec (v0.3)

**Date:** 2026-05-02 *(revised after codebase audit)*
**Status:** Approved for implementation
**Target release:** v0.3.0 (alpha → beta → rc → stable)
**Driving customer:** Clinical-trials site coordinator persona

## 0. Current state audit (brownfield reality)

This spec is **not** greenfield. The inference backbone already exists. The integrity contract is what's missing.

**What's already implemented (no work needed):**

- `src/inference/` — full pluggable backend layer
  - `InferenceBackend` trait (≈ what spec calls `LlmClient`)
  - Adapters: `OllamaBackend`, `OpenAICompatibleBackend`, `LocalGemmaBackend` (in-binary llama-gguf — Approach 3 partially shipped)
  - `ModelManager`, request queue
- `src/ai/summarizer.rs` — feature **A** (98 lines)
- `src/ai/rag.rs` — feature **B** (275 lines, includes hybrid search + graph expansion)
- `src/ai/tagger.rs` — feature **C** (98 lines)
- `src/ai/linker.rs` — feature **E** (143 lines)
- All four wired to REST under `/api/v1/ai/*`
- `LinkType::AiSuggested` variant defined in `src/models/link.rs`

**What's missing (this spec's scope):**

| Gap | Why it matters |
|---|---|
| `AuditedInference` wrapper around the backend | The integrity contribution. Without this, no hash-chained audit of LLM calls. |
| Migration 010: `llm_audit` table + `llm_call` event_type in `events` | The audit denormalization layer. |
| Output-lands-via-integrity-layer discipline in A/B/C/E | Currently A/B return strings; C returns suggestions but doesn't persist; E uses `LinkType::Semantic` and `auto_link_all` writes without review. All four need to land via existing integrity primitives. |
| Citation validation in `RagEngine` (hallucination guard) | RAG can return note IDs not in retrieved context — must be stripped. |
| Feature module **D**: `src/ai/deviations.rs` (protocol deviation scan) | Doesn't exist at all. |
| MCP tool exposure for A/B/C/D/E + `llm_audit_query` | All AI is REST-only. None of the 18 existing MCP handlers are AI-powered. |

**Naming reconciliation:**

The spec uses `LlmClient` and `AuditedLlm` for clarity. In the code, these map to:

| Spec name | Code name | Status |
|---|---|---|
| `LlmClient` trait | `InferenceBackend` | Already exists |
| `LlmResponse`, `CompletionParams` | `GenerateResponse`, `GenerateRequest` | Already exist |
| `OllamaClient` | `OllamaBackend` | Already exists |
| `OpenAICompatibleClient` | `OpenAICompatibleBackend` | Already exists |
| `AuditedLlm` | `AuditedInference` (new) | To be implemented |

The remainder of this spec describes the *target state*. Where it would create new types that already exist (Sections 4 and 7), implementations should extend or wrap the existing types rather than duplicate them.

## 1. Goal

Add LLM-backed reasoning to Smriti for five clinical-trials capabilities — narrative drafting, RAG Q&A, auto-categorization, protocol deviation flagging, and link suggestions — without compromising Smriti's three load-bearing properties:

1. **Self-hosted, zero cloud required** (HIPAA-safe by default)
2. **Single binary deploy** (no new mandatory external services)
3. **Hash-chained, auditable provenance** (Part 11 / ICH E6(R3) defensible)

The integrity layer Smriti already shipped (provenance scoring, `wiki_transactions`, `contradiction_events`, hash-chained `events`, `LinkType::AiSuggested`) becomes the substrate that makes LLM use *safe* in regulated environments. This is the core wedge.

## 2. Scope

### In scope (v0.3)

- LLM client trait + two adapters (Ollama, OpenAI-compatible)
- Audited LLM wrapper that hash-chains every call into the existing `events` table
- Five feature modules + their MCP tools:
  - **A.** `notes_summarize` — monitor reports, SAE narratives
  - **B.** `notes_ask` — RAG Q&A with citations
  - **C.** `notes_categorize` — auto-tag suggestions with three-policy ladder
  - **D.** `protocol_deviations_scan` — flag candidates against bi-temporal protocol versions
  - **E.** `notes_suggest_links` — typed link suggestions
- One audit query tool: `llm_audit_query`
- Migration 010: `llm_audit` table for query-performance denormalization
- Per-tool config in `config.toml` under `[llm]` and `[llm.<tool>]`

### Out of scope (deferred)

- **F. `protocols_find_similar`** (structured-field protocol comparison) → v0.4. Customer pull required first.
- Streaming responses → v0.4 (most-requested for `notes_ask`)
- Direct Anthropic SDK adapter → v0.4 (covered via OpenAI-compat through proxy in v0.3)
- Function-calling APIs → not planned (JSON mode covers our needs)
- Multi-turn conversation state inside Smriti → never. Stateless calls only; agents own conversation memory.
- Fine-tuning, RLHF, or any model training → never. Smriti uses LLMs; it does not improve them.
- Embedded local-LLM runtime (`candle` / `llama.cpp` in-binary) → not planned. Defer until customers ask.
- Prompt-template editing via API → never. Templates live in source code (`src/llm/prompts/*.md`); changing one requires a release. Auditability requires this.

## 3. Architecture

Five layers, top-down:

```
MCP tools layer
  notes_summarize · notes_ask · notes_categorize
  protocol_deviations_scan · notes_suggest_links · llm_audit_query
        │
Feature modules (src/features/llm/)
  summarize.rs · ask.rs · categorize.rs · deviations.rs · link_suggest.rs
  Each: builds prompt → AuditedLlm.complete() → parses → lands result via integrity layer
        │
AuditedLlm wrapper (src/llm/audited.rs)
  Logs (prompt_hash, response_hash, model, ts) into events (existing hash chain)
  Also denormalizes into llm_audit for query performance
        │
LlmClient trait (src/llm/client.rs)
  async complete(messages, params) → LlmResponse
        │
Adapters (src/llm/adapters/)
  OllamaClient (default) · OpenAICompatibleClient
```

### Three architectural invariants

1. **Feature modules cannot construct a raw `LlmClient`.** They take `Arc<AuditedLlm>` from `AppState`. Constructor visibility is `pub(crate)`, called only from `AppState::new`. Enforced at compile time.

2. **All LLM-generated content goes through existing integrity primitives.** Output lands as:
   - `LinkType::AiSuggested` edges (with confidence stored)
   - Pending `wiki_transactions` for inbox review
   - Rows in `contradiction_events` (deviations)
   - Entries in `agent_memory` under `namespace: "ai_suggestions/*"` (categories)

   Never as direct writes to `notes` or canonical `links`.

3. **No LLM configured ⇒ features still load, just disabled.** Missing `[llm]` block in `config.toml` returns `{ error: "no_llm_configured", hint: "set [llm] in config.toml" }` from every LLM-backed tool. Server does not fail-start.

## 4. LlmClient trait

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        params: &CompletionParams,
    ) -> Result<LlmResponse, LlmError>;

    fn name(&self) -> &str;          // "ollama:llama3.1:8b"
    fn capabilities(&self) -> Capabilities;
}

pub struct Message { pub role: Role, pub content: String }
pub enum Role { System, User, Assistant }

pub struct CompletionParams {
    pub temperature: f32,             // default 0.0 for clinical use
    pub max_tokens: u32,
    pub seed: Option<u64>,            // reproducibility
    pub response_format: Option<ResponseFormat>,  // Json | Text
    pub timeout: Duration,
}

pub struct LlmResponse {
    pub content: String,
    pub model: String,                // exact version returned by provider
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub finish_reason: FinishReason,
}

pub struct Capabilities {
    pub json_mode: bool,
    pub max_context: u32,
    pub embeddings: bool,
}
```

### Adapters in v0.3

| Adapter | Default endpoint | Auth | Use case |
|---|---|---|---|
| `OllamaClient` | `http://localhost:11434` | none | HIPAA / clinical default. Local model on-box. |
| `OpenAICompatibleClient` | configurable | bearer via env var | OpenAI / Together / vLLM / LM Studio / Groq / Anthropic-via-proxy. De-facto API standard. |

**Reproducibility defaults:** `temperature = 0.0`, `seed = Some(42)`. Per-tool override allowed if a use case justifies it (none in v0.3 currently does).

**Health check on startup:** `LlmClient::probe()` runs once when the server boots. Failure logs a warning but does not crash. Health surfaced via `/api/v1/health`.

## 5. AuditedLlm wrapper

```rust
pub struct AuditedLlm {
    inner: Arc<dyn LlmClient>,
    db: Arc<Database>,
}

impl AuditedLlm {
    pub async fn complete(
        &self,
        messages: &[Message],
        params: &CompletionParams,
        ctx: &CallContext,
    ) -> Result<LlmResponse, LlmError> { /* hash, call, log to events + llm_audit, return */ }
}

pub struct CallContext {
    pub agent_id: String,
    pub tool_name: String,             // "notes_summarize"
    pub note_ids: Vec<String>,         // context notes the prompt was built from
    pub purpose: String,               // human-readable: "summarize patient timeline"
}
```

### What is and isn't stored in audit

| Stored | Not stored |
|---|---|
| Prompt hash (SHA-256) | Raw prompt text |
| Response hash (SHA-256) | Raw response text |
| Model name + version | — |
| Temperature, seed | — |
| Note IDs the prompt referenced | The note contents (already in `notes` — link by ID) |
| Token counts, duration | — |
| Outcome (success / error / timeout / invalid_json) | — |
| Prompt template version (e.g. `summarize@v1`) | — |

Rationale: prompt and response are recoverable on demand by replay (same notes + same prompt template version + same temperature + same seed + same model = same hash). Storing only hashes minimizes the PHI surface to certify.

If hash mismatch on replay → either tampering or silent model upgrade. Both are findings worth flagging.

### Hash-chain integration

Every LLM call writes one row to `events` with `event_type = 'llm_call'`. Reuses the existing `prev_hash → hash` chain logic. `wiki_verify --chain` automatically covers LLM events for free.

## 6. Database changes (Migration 010)

```sql
-- Denormalized for query performance — the canonical record is in events
CREATE TABLE llm_audit (
    id              TEXT PRIMARY KEY,
    event_id        TEXT NOT NULL REFERENCES events(id),
    agent_id        TEXT NOT NULL,
    tool_name       TEXT NOT NULL,           -- 'notes_summarize' etc.
    model           TEXT NOT NULL,           -- 'ollama:llama3.1:8b'
    prompt_hash     TEXT NOT NULL,
    response_hash   TEXT,                    -- NULL on error before LLM responded
    prompt_template_version TEXT NOT NULL,   -- 'summarize@v1'
    note_ids        TEXT NOT NULL,           -- JSON array
    temperature     REAL NOT NULL,
    seed            INTEGER,
    prompt_tokens   INTEGER,
    completion_tokens INTEGER,
    duration_ms     INTEGER NOT NULL,
    outcome         TEXT NOT NULL,           -- 'success'|'error'|'timeout'|'invalid_json'
    error_message   TEXT,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_llm_audit_agent_time ON llm_audit(agent_id, created_at DESC);
CREATE INDEX idx_llm_audit_tool_time  ON llm_audit(tool_name, created_at DESC);
```

**No other new tables.** Outputs land in existing tables:

| Output | Lands in |
|---|---|
| Narrative drafts (A) | Returned in response, optionally `wiki_transactions` (pending) |
| Q&A answers (B) | Read-only response. Optional `note_access_log` write for cited notes |
| Tag suggestions (C) | `agent_memory` namespace `ai_suggestions/tags` |
| Deviation candidates (D) | `contradiction_events` with `event_type = 'llm_protocol_deviation'` |
| Link suggestions (E) | `links` rows with `link_type = 'AiSuggested'` |

## 7. Feature modules

Common shape:

```rust
pub trait LlmFeature {
    type Input;
    type Output;
    async fn run(&self, ctx: &CallContext, input: Self::Input) -> Result<Self::Output>;
}
```

Each module: build prompt from graph → `AuditedLlm.complete()` → parse → land output through integrity layer.

### A. summarize.rs

- **Input:** `{ note_ids: [..], style: "monitor_report" | "sae_narrative" | "discharge_summary" | "free", submit_as_pending?: bool }`
- **Output:** `{ draft: string, citations: [note_id], audit_event_id }`
- **Lands as:** Returned draft string. No graph write by default. If `submit_as_pending: true`, wrapped as a `wiki_transaction` for inbox review.
- **Why no auto-write:** SAE narratives need human sign-off before they exist as official records (Part 11).

### B. ask.rs

- **Pipeline:** Hybrid search (FTS5 + sqlite-vec) over scope → top 8 notes → prompt LLM in JSON mode → validate citations actually exist in retrieved context → return.
- **Input:** `{ question: string, scope?: { tag?, agent_id?, note_ids? }, top_k?: 8 }`
- **Output:** `{ answer, citations: [note_id], confidence: 0-1, refused_reason?, audit_event_id }`
- **Lands as:** Read-only response. Cited notes get a `note_access_log` row (feeds Task 9 consolidation scoring).
- **Hallucinated-citation guard:** Strip any `[[note_id]]` not in retrieved context, log warning, reduce confidence. If all citations bad → return `confidence: 0.0, refused_reason: "no valid citations"`.

### C. categorize.rs

- **Input:** `{ note_id, taxonomy?: [..], policy?: "conservative"|"standard"|"aggressive" }`
- **Output:** `{ suggestions: [{tag, confidence}], applied: [string], pending_review: [string], audit_event_id }`
- **Three-policy ladder** (mirrors Task 9 consolidation):
  - **Conservative** (default for clinical): all suggestions stored as `agent_memory` under `ai_suggestions/tags`. Coordinator reviews & accepts via web UI / `notes_apply_suggestions`.
  - **Standard:** tags with confidence > `auto_apply_threshold` (default 0.85, configurable) auto-applied. Below threshold → review queue.
  - **Aggressive:** all suggestions auto-applied. Not recommended for regulated environments.

### D. deviations.rs

- **Pipeline:** Resolve active protocol via bi-temporal edges (`valid_from`/`valid_until` at note's `created_at`) → LLM extracts deviation candidates with severity + cited spans → write each to `contradiction_events`.
- **Input:** `{ note_id, protocol_note_id?: string }` (auto-resolves bi-temporal version if not given)
- **Output:** `{ candidates: [{kind, severity, span_in_note, protocol_clause_violated, confidence}], audit_event_id }`
- **Lands as:** Each candidate → row in `contradiction_events` with `event_type: "llm_protocol_deviation"`. Surfaces in same review inbox as `contradictions_detect`. **Never auto-resolved** — same gate as existing contradictions.

### E. link_suggest.rs

- **Pipeline:** Get note's nearest semantic neighbors via sqlite-vec → LLM ranks + suggests typed link kind (`relates_to`, `supports`, `refutes`, `replicates`, `revises`).
- **Input:** `{ note_id, top_k?: 5 }`
- **Output:** `{ suggestions: [{target_note_id, link_type, confidence, reason}], audit_event_id }`
- **Lands as:** `LinkType::AiSuggested` edges with confidence stored. **Hidden from default `notes_graph` BFS** unless caller passes `include_ai_suggested: true`. Queryable but doesn't pollute the canonical graph.

## 8. MCP tool surface

| Tool | Writes to graph? | Notes |
|---|---|---|
| `notes_summarize` | No by default; pending wiki_transaction if `submit_as_pending` | NEW |
| `notes_ask` | No (read-only; access_log only) | NEW |
| `notes_categorize` | Suggestions only (Conservative); auto-apply above threshold (Standard/Aggressive) | NEW |
| `protocol_deviations_scan` | Each candidate → `contradiction_events`; never auto-resolved | NEW |
| `notes_suggest_links` | `LinkType::AiSuggested` edges, hidden by default | NEW |
| `llm_audit_query` | No (read-only over `llm_audit`) | NEW |

### Conventions across all six

- **`audit_event_id`** in every response — hash-chained pointer for traceability.
- **JSON mode (`response_format: Json`)** for everything except `notes_summarize` (free-form prose). Server-side schema validation; on parse failure → `{ error: "llm_output_invalid", raw: "...", retry_suggested: true }`.
- **Uniform `{ error: "no_llm_configured" }`** when `[llm]` missing.
- **Per-tool timeouts** override global config: summarize 90s, ask 30s, categorize 15s, deviations 60s, link_suggest 20s.
- **No streaming in v0.3.**

### HTTP API mirror

```
POST /api/v1/llm/summarize
POST /api/v1/llm/ask
POST /api/v1/llm/categorize
POST /api/v1/llm/protocol-deviations
POST /api/v1/llm/suggest-links
GET  /api/v1/llm/audit?agent_id=&since=&limit=
```

One-to-one mapping with MCP tools.

### MCP contract notes (CLAUDE.md guardrail #4)

- All six tools above are **NEW** — no breaking-change flag needed.
- `notes_create` gains optional `auto_categorize: bool` (default `false`) — `// BREAKING MCP CONTRACT: optional field added`.
- `notes_search_semantic` gains optional `suggest_links: bool` (default `false`) — `// BREAKING MCP CONTRACT: optional field added`.

Both default `false` so existing callers see identical behavior.

## 9. Configuration (`config.toml`)

```toml
[llm]
provider = "ollama"                          # "ollama" | "openai_compatible"
endpoint = "http://localhost:11434"
model = "llama3.1:8b"
api_key_env = "SMRITI_LLM_API_KEY"           # only used by openai_compatible
default_temperature = 0.0
default_max_tokens = 2048
timeout_secs = 60

[llm.summarize]
timeout_secs = 90
max_tokens = 4096

[llm.ask]
timeout_secs = 30
top_k = 8

[llm.categorize]
timeout_secs = 15
default_taxonomy = ["visit","ae","protocol","monitoring","regulatory"]
auto_apply_threshold = 0.85

[llm.deviations]
timeout_secs = 60

[llm.link_suggest]
timeout_secs = 20
top_k = 5
```

The entire `[llm.*]` block is optional. Existing `config.toml` files continue to work unchanged.

## 10. Failure modes

| Failure | Behavior | Audit row written? |
|---|---|---|
| `[llm]` missing | `{ error: "no_llm_configured" }` | No (no call attempted) |
| Endpoint unreachable | Retry once with 2× backoff, then `error: "llm_unreachable"` | Yes, `outcome: "error"` |
| Timeout | Abort via `tokio::time::timeout`, return `error: "llm_timeout"` | Yes, `outcome: "timeout"` |
| Invalid JSON (when JSON mode requested) | One repair attempt with appended "Reply with valid JSON only" — then fail | Yes, `outcome: "invalid_json"` (both attempts) |
| Hallucinated citation (note_id not in retrieved context) | Strip bad citation, log warning. If all citations bad → `confidence: 0.0`, `refused_reason: "no valid citations"` | Yes, `outcome: "success"` with warning |
| Rate-limit / 429 | Exponential backoff 3 attempts, then surface to caller | Yes, `outcome: "error"` |
| Prompt exceeds context window | Truncate from middle of context (preserves first/last note for instruction-following), log warning | Yes, `outcome: "success"` with `truncated: true` |

## 11. Testing strategy

- **Unit tests on prompt builders** — snapshot tests assert prompt structure. Catches accidental prompt mutation.
- **`MockLlmClient`** for feature-module tests — returns canned responses. Tests parsing, validation, integrity-write paths without an LLM.
- **Integration test against real Ollama in CI** — one smoke test per feature module, gated by `SMRITI_TEST_OLLAMA=1`. Skipped by default; runs nightly. Smallest practical model (`qwen2.5:0.5b`); CI under 5 min.
- **Audit chain property test** — fuzz: 100 random LLM calls → walk events chain → assert `wiki_verify --chain` passes.
- **Hallucinated-citation guard test** — feed validator a fake `[[note_id]]` not in context; assert it's stripped and confidence reduced.
- **Failure-mode tests** — explicit test per row in §10's failure matrix.

## 12. Rollout phases

Phases revised given the brownfield reality from §0.

| Phase | Scope | Ships when |
|---|---|---|
| **0.3.0-alpha.1** | Migration 010 (`llm_audit` table + `llm_call` event_type). `AuditedInference` wrapper. `AiAppState` refactored to hold `Arc<AuditedInference>`. Existing A/B/C/E features routed through the wrapper. No new MCP tools yet. | After `wiki_verify --chain` walks `llm_call` events successfully |
| **0.3.0-alpha.2** | Integrity-layer landing for A/B/C/E: `Summarizer.submit_as_pending` flag wraps drafts as `wiki_transactions`; `RagEngine` adds citation validation; `AutoTagger` persists suggestions to `agent_memory` namespace `ai_suggestions/tags`; `AiLinker` switches from `LinkType::Semantic` to `LinkType::AiSuggested` and removes silent-write `auto_link_all`. MCP handlers for `notes_summarize`, `notes_ask`, `llm_audit_query`. | After alpha.1 dogfoodable for 1 week |
| **0.3.0-beta.1** | MCP handlers for `notes_categorize`, `notes_suggest_links`. Three-policy ladder for `categorize` (Conservative default). `notes_graph` extended with `include_ai_suggested` filter (default `false`). | After alpha.2 + clinical-trials customer validates A & B |
| **0.3.0-rc.1** | New module `src/ai/deviations.rs` for feature D. MCP handler `protocol_deviations_scan`. Each candidate writes to `contradiction_events` with `event_type='llm_protocol_deviation'`. | After beta.1 has 2 weeks of suggestion data |
| **0.3.0** | Stable release. Documentation + customer onboarding. | After rc.1 in production at ≥1 trial site for 2 weeks |
| **0.4.0** | F (`protocols_find_similar`), streaming for `notes_ask`, native Anthropic adapter | Customer demand pull |

## 13. Clinical-trials defensibility (the moat)

When an FDA / sponsor auditor asks: *"Why did the AI suggest protocol deviation X on Patient-14 on Mar 3?"*

1. Look up `events.llm_call` rows around that timestamp + Patient-14's `note_id` → find call(s) involving the deviation tool
2. Read `llm_audit` row: model + prompt template version + temperature + seed + note IDs that fed the prompt
3. Re-derive prompt from those notes + that template version
4. Re-run with same model + same seed
5. Compare response hash against stored `response_hash`

Match → reproducible decision. No match → either model upgraded silently, or chain tampered. Both are findings.

This pattern is the architectural reason Smriti can credibly claim "AI suggestions defensible under Part 11 / ICH E6(R3)" — competitors using Mem0 / Zep / LangMem cannot, because their data layer wasn't designed for it.

## 14. Open questions (none blocking)

None at design time. Implementation phases will surface details (specific prompt template wording, exact hallucination-detection heuristic, etc.) that get resolved in code-review during alpha.

## 15. Dependencies

New crate dependencies added in v0.3:

- `async-trait` — already in Cargo.toml
- `reqwest` — already in Cargo.toml (used for sync engine; reused for LLM HTTP)
- `sha2` — already in Cargo.toml
- `serde_json` — already in Cargo.toml

**Zero new dependencies.** Implementation reuses the existing crate set.

# Smriti — Claude Code Project Context
# Auto-loaded by Claude Code on every session start.
# Source: github.com/biosync-tech/smriti | Last updated: March 2026

---

## Project Identity

Smriti is a **self-hosted, Rust-based knowledge graph + agent memory layer**.
Single binary. SQLite only. MCP-native. Zero cloud dependencies.
Goal: become the canonical graph-native, local-first agent memory layer.

**Repository:** https://github.com/biosync-tech/smriti
**Crate:** https://crates.io/crates/smriti
**Stack:** Rust + Axum + SQLite (FTS5 + WAL) + petgraph + Tokio + clap

---

## Confirmed Cargo.toml (v0.1.0)

```toml
[dependencies]
axum              = { version = "0.7", features = ["json", "query"] }
tokio             = { version = "1",   features = ["full"] }
tower             = "0.4"
tower-http        = { version = "0.5", features = ["cors", "trace"] }
rusqlite          = { version = "0.31", features = ["bundled", "vtab"] }
clap              = { version = "4",   features = ["derive"] }
petgraph          = "0.6"
serde             = { version = "1",   features = ["derive"] }
serde_json        = "1"
toml              = "0.8"
uuid              = { version = "1",   features = ["v4", "serde"] }
chrono            = { version = "0.4", features = ["serde"] }
regex             = "1"
thiserror         = "1"
anyhow            = "1"
tracing           = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
reqwest           = { version = "0.12", features = ["json", "rustls-tls"],
                      default-features = false }
sha2              = "0.10"
base64            = "0.22"
async-trait       = "0.1"
hostname          = "0.4"
```

**CONFIRMED PRESENT (dev-dependencies):**
- ✅ criterion 0.5   (benchmarks live at benches/smriti_bench.rs)

**CONFIRMED PRESENT (additional):**
- ✅ sqlite-vec 0.1  (semantic search via notes_vec vtab — Migration 002)

**CONFIRMED ABSENT — do not assume these exist:**
- ❌ fastembed       (no local embeddings)
- ❌ usearch         (no HNSW index)

---

## Module Tree

```
src/
├── models/    — Note, Link, AgentMemory, ToolLog structs
├── storage/   — SQLite + FTS5 + WAL mode (rusqlite)
├── parser/    — [[wiki-link]] and #tag extraction via regex
├── graph/     — petgraph DiGraph, BFS traversal, GraphCache (Arc<RwLock>)
├── api/       — Axum REST API (CORS, tracing)
├── mcp/       — MCP JSON-RPC 2.0 server, stdio transport only
├── cli/       — clap CLI, 11 commands
├── sync/      — WebDAV + filesystem sync engine
└── features/  — Smart link suggestions, daily digest
```

---

## Core Types (verified against source — March 2026)

```rust
// src/models/note.rs
pub struct Note {
    pub id:             String,
    pub title:          String,
    pub content:        String,
    pub created_at:     DateTime<Utc>,
    pub updated_at:     DateTime<Utc>,
    pub tags:           Vec<String>,
    pub backlink_count: usize,
    pub wikilink_count: usize,
    // -- Task 9: Consolidation (CLS-inspired) --
    pub node_type:            NodeType,          // Episode | Schema
    pub consolidation_score:  f32,               // 0.0..1.0, normalised
    pub access_count:         u64,               // lifetime read/traversal hits
    pub last_accessed_at:     Option<DateTime<Utc>>,
    pub parent_schema_id:     Option<String>,    // set when episode is subsumed by a schema
}

pub enum NodeType {
    Episode,   // default — raw note
    Schema,    // compressed abstraction derived from ≥N episodes
}

// src/models/link.rs
pub struct Link {
    pub id:             String,
    pub source_note_id: String,
    pub target_note_id: String,
    pub link_type:      LinkType,           // WikiLink | Backlink | Tag | AiSuggested
    pub created_at:     DateTime<Utc>,
    pub valid_from:     Option<DateTime<Utc>>,  // bi-temporal: when relationship became valid
    pub valid_until:    Option<DateTime<Utc>>,  // bi-temporal: None = currently valid
}

// src/models/agent.rs
pub struct AgentMemory {
    pub id:          String,
    pub agent_id:    String,
    pub namespace:   String,
    pub key:         String,
    pub value:       serde_json::Value,
    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
    pub ttl_seconds: Option<i64>,
}

pub struct ToolLog {
    pub id:          String,
    pub agent_id:    String,
    pub tool_name:   String,
    pub input:       serde_json::Value,
    pub output:      serde_json::Value,
    pub status:      ToolStatus,           // Success | Error | Timeout
    pub duration_ms: Option<i64>,
    pub created_at:  DateTime<Utc>,
}
```

---

## SQLite Schema (verified against src/storage/db.rs — March 2026)

```sql
CREATE TABLE notes (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    -- Task 9: consolidation (CLS, McClelland 1995)
    node_type            TEXT    NOT NULL DEFAULT 'episode',  -- 'episode' | 'schema'
    consolidation_score  REAL    NOT NULL DEFAULT 0.0,
    access_count         INTEGER NOT NULL DEFAULT 0,
    last_accessed_at     TEXT,
    parent_schema_id     TEXT REFERENCES notes(id) ON DELETE SET NULL
);
CREATE INDEX idx_notes_consolidation ON notes(node_type, consolidation_score DESC);
CREATE VIRTUAL TABLE notes_fts USING fts5(
    title, content,
    content=notes, content_rowid=rowid,
    tokenize='porter unicode61'
);
CREATE TABLE tags (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    color      TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (note_id, tag_id)
);
CREATE TABLE links (
    id              TEXT PRIMARY KEY,
    source_note_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_note_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    link_type       TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    valid_from      TEXT,          -- bi-temporal (Zep arXiv:2501.13956)
    valid_until     TEXT,          -- NULL = currently valid
    UNIQUE(source_note_id, target_note_id, link_type)
);
CREATE TABLE agent_memory (
    id         TEXT PRIMARY KEY,
    agent_id   TEXT NOT NULL,
    namespace  TEXT NOT NULL DEFAULT 'default',
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    ttl_seconds INTEGER,
    UNIQUE(agent_id, namespace, key)
);
CREATE TABLE tool_logs (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL,
    tool_name   TEXT NOT NULL,
    input       TEXT NOT NULL,
    output      TEXT NOT NULL,
    status      TEXT NOT NULL,
    duration_ms INTEGER,
    created_at  TEXT NOT NULL
);

-- Task 9: access log feeds the consolidation score (CLS replay signal)
CREATE TABLE note_access_log (
    id             TEXT PRIMARY KEY,
    note_id        TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    accessed_at    TEXT NOT NULL,
    access_kind    TEXT NOT NULL,    -- 'read' | 'search_hit' | 'graph_traverse' | 'mcp_retrieve'
    query_context  TEXT,             -- raw query string (for context diversity)
    query_embedding BLOB,            -- optional (Task 6 dependency) — for semantic context diversity
    agent_id       TEXT
);
CREATE INDEX idx_access_note_time ON note_access_log(note_id, accessed_at DESC);

-- Task 9: lineage — which episodes a schema was abstracted from
CREATE TABLE schema_sources (
    schema_id        TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    source_note_id   TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    similarity_score REAL,
    consolidated_at  TEXT NOT NULL,
    PRIMARY KEY (schema_id, source_note_id)
);

-- Task 9: auditable consolidation decisions (required for ICH E6(R3) trail)
CREATE TABLE consolidation_events (
    id             TEXT PRIMARY KEY,
    note_id        TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    event_type     TEXT NOT NULL,    -- 'promoted_to_schema' | 'flagged_for_review' | 'archived' | 'score_recomputed'
    score_before   REAL,
    score_after    REAL,
    reason         TEXT NOT NULL,    -- human-readable rationale
    created_at     TEXT NOT NULL
);
```

---

## petgraph Graph Type + GraphCache

```rust
// src/graph/knowledge_graph.rs
pub struct KnowledgeGraph {
    graph: DiGraph<NodeInfo, EdgeInfo>,
    id_to_index: HashMap<String, NodeIndex>,
}
struct NodeInfo { id: String, title: String, tag_count: usize }
struct EdgeInfo { link_type: String }

// src/graph/cache.rs — SHIPPED
pub struct GraphCache {
    graph: KnowledgeGraph,
    dirty: bool,
    built_at: Option<DateTime<Utc>>,
}
// Wrapped as Arc<tokio::sync::RwLock<GraphCache>> in AppState.
// invalidate() called on notes_create/update/delete.
// ensure_fresh(&db) rebuilds only when dirty.
// Research ref: MAGMA arXiv:2601.03236 §3.2
```

---

## MCP Tools (18 confirmed, stdio + HTTP transport)

| Tool                         | Description                                                              |
|------------------------------|--------------------------------------------------------------------------|
| notes_create                 | Create note; auto-detects [[wiki-links]] and #tags                       |
| notes_read                   | Read note by ID or title (instruments access log for CLS replay signal)  |
| notes_search                 | FTS5 full-text search (instruments search hits for CLS replay signal)    |
| notes_list                   | List notes, filter by tag                                                |
| notes_graph                  | Full graph or subgraph around a note (BFS), optional link_type filter    |
| notes_search_semantic        | Semantic search via sqlite-vec; optional hybrid FTS5+cosine with RRF     |
| memory_store                 | KV store with namespace, TTL, and AGM conflict_policy (arXiv:2603.17244) |
| memory_retrieve              | Get entry by agent_id + namespace + key                                  |
| memory_list                  | List all memory entries for an agent                                     |
| memory_history               | List superseded values (version_and_keep / invalidate policies)          |
| wiki_transaction_submit      | Atomic multi-write with enforced provenance (FACTUM arXiv:2601.05866)    |
| wiki_transaction_commit      | Commit a pending transaction                                             |
| wiki_transaction_reject      | Reject a pending transaction with reason                                 |
| wiki_transaction_list_pending| List pending transactions awaiting review                                |
| wiki_verify                  | Full integrity sweep: referential, provenance, hash chain, orphans       |
| contradictions_detect        | Scan notes for candidate contradictions (MemoTime arXiv:2510.13614)      |
| contradictions_list          | List open contradiction candidates                                       |
| notes_consolidate            | CLS-inspired consolidation pass; scores, flags, archives. Never deletes. |
| ingest_document              | Chunk a .txt/.md file into the KG — parent doc note + chunk notes + ChunkOf links. No LLM. |
| retrieve_context             | Query → hybrid search + BFS graph expansion + context assembly → context string for local LLM. |

**⚠️ MCP transport: stdio is primary.** HTTP endpoint exists at `POST /mcp` via `dispatch_http`.

---

## REST API Endpoints

```
POST   /api/v1/notes
GET    /api/v1/notes              ?limit=N&tag=X
GET    /api/v1/notes/:id
PUT    /api/v1/notes/:id
DELETE /api/v1/notes/:id
GET    /api/v1/notes/search       ?q=...
GET    /api/v1/notes/:id/backlinks
GET    /api/v1/notes/:id/links
GET    /api/v1/graph
GET    /api/v1/graph/:id          ?depth=N
GET    /api/v1/stats
POST   /api/v1/agent/:id/memory
GET    /api/v1/agent/:id/memory   ?namespace=default
GET    /api/v1/agent/:id/memory/:namespace/:key
POST   /api/v1/agent/:id/tool-logs
GET    /api/v1/agent/:id/tool-logs  ?limit=50

# Task 9 — Consolidation
POST   /api/v1/consolidation/run          # trigger a consolidation pass (dry_run=true by default)
GET    /api/v1/consolidation/events       # audit log of promotions/archives/flags
GET    /api/v1/consolidation/proposals
POST   /api/v1/consolidation/proposals/:id/accept
POST   /api/v1/consolidation/proposals/:id/reject
GET    /api/v1/notes/:id/lineage          # for schemas: list source episodes + similarity
POST   /api/v1/notes/:id/access           # instrument an external access hit (feeds replay signal)
```

---

## Research Anchors (verified arXiv IDs — use these for feature justifications)

| Paper                          | arXiv ID    | Key Finding for Smriti                                   |
|-------------------------------|-------------|----------------------------------------------------------|
| Zep / Graphiti                 | 2501.13956  | Bi-temporal edges (valid_from/valid_until) improve LongMemEval 18.5% |
| MAGMA multi-graph              | 2601.03236  | Typed graph layers (semantic/temporal/causal) reduce tokens 95% |
| Graph-Native Belief Revision   | 2603.17244  | AGM conflict resolution → ConflictPolicy on memory_store |
| Graph-Based Memory Survey      | 2602.05665  | Graph+BM25 hybrid beats pure vector for multi-hop tasks  |
| Complementary Learning Systems | McClelland 1995 / Kumaran 2016 TiCS | Hippocampal episodes → neocortical schemas via replay; basis for consolidation + forgetting curve (Task 9). NOTE: not arXiv. |
| WikiSkill                      | 2608.27454  | Persistent wiki between traces and procedures; isolate proposals from inference until accepted. Architecture reference — not Trace2Skill/EvoSkill/SkillOpt. |

---

## Shipped (completed this sprint)

| # | Item                           | Location                          |
|---|-------------------------------|-----------------------------------|
| 1 | GraphCache (lazy rebuild)      | src/graph/cache.rs + api/server.rs |
| 2 | Temporal edge columns          | valid_from, valid_until on links table |
| 3 | CI/CD pipeline                 | .github/workflows/ci.yml          |
| 4 | Docker                         | Dockerfile + docker-compose.yml   |
| 5 | Criterion benchmarks           | benches/smriti_bench.rs           |
| 6 | sqlite-vec semantic search     | notes_vec vtab + note_embeddings_meta (Migration 002) |
| 7 | memory_history (AGM)           | Migration 003 — belief revision archive  |
| 8 | Provenance layer (FACTUM)      | sources + claim_spans (Migration 004)    |
| 9 | Wiki transactions              | wiki_transactions (Migration 005)        |
| 10| Contradiction events           | contradiction_events (Migration 006)     |
| 11| Append-only event log          | events table, hash-chained (Migration 007) |
| 12| Agent grants (ACL)             | agent_grants (Migration 008)             |
| 13| Consolidation foundation       | **Task 9 Phase 1** — Migration 009 + src/features/consolidation.rs |
| 14| notes_consolidate MCP tool     | **Task 9 Phase 2** — src/mcp/handlers.rs + server.rs |
| 15| Access-log instrumentation     | MCP read/search/graph handlers instrument note_access_log |
| 16| unwrap() cleanup               | MutexPoisoned variant + 13 lock().unwrap() → map_err across db.rs, verify.rs, contradiction.rs |
| 17| Note struct consolidation fields | NodeType enum, consolidation_score, access_count, last_accessed_at, parent_schema_id |
| 18| **Path A: Local KG for local LLMs** | `ChunkOf` LinkType + `src/ai/document_ingest.rs` + `ingest_document` MCP tool + `retrieve_context` MCP tool + REST `POST /api/v1/ingest/document` + `POST /api/v1/retrieve` |
| 19| WikiSkill schema formation (Task 9 Phase 3) | `src/features/schema_formation.rs` — proposals are events until accept; Conservative never auto-commits |
| 20| Retrieve-context proxy gate | Standard/Aggressive may auto-commit only on held-out `query_context` lift; audit says `not WikiSkill task-accuracy` |
| 21| Human review CLI + REST + MCP | `smriti proposals/approve/reject`, `/api/v1/consolidation/proposals`, `accept_proposal_id` / `reject_proposal_id` |

## Known Gaps (prioritised — work top-to-bottom)

| # | Gap                            | Severity | Fix Location              | Effort |
|---|-------------------------------|----------|---------------------------|--------|
| 1 | No labelled WikiSkill task-accuracy gate (`y_i`) | MODERATE | docs/consolidation-proxy.md — proxy is retrieval-only by design | — |
| 2 | No GitHub topics set           | HIGH     | GitHub UI                 | XS     |
| 3 | No screenshots/GIFs in README  | HIGH     | README.md                 | S      |
| 4 | No HTTP MCP transport          | MODERATE | src/mcp/                  | M      |
| 5 | No graph viz dashboard         | MODERATE | src/api/ + static/        | L      |
| 6 | API routes not instrumented for access logging | MODERATE | src/api/routes/notes.rs | S |
| 7 | Path A — PDF ingestion (Path B) | LOW     | Add `pdf-extract` crate + `src/ai/document_ingest.rs` | S |
| 8 | Path B — entity extraction (LLM-driven) | LOW | `src/ai/document_ingest.rs` + Ollama backend | L |

> Note: Gap #2 is the positioning wedge vs Mem0/Zep/LangMem — they all grow
> unboundedly. See Task 9 in the Priority Queue.

---

## Absolute Coding Constraints (ALWAYS FOLLOW — never override)

1. **Single binary** — zero new mandatory external processes. Smriti ships as one binary.
2. **SQLite only** — never suggest Neo4j, Qdrant, Redis, or any server-mode database.
3. **No cloud required** — every feature must work fully offline.
4. **Flag MCP breaking changes** — any change to an existing tool's JSON-RPC contract
   must be marked `// BREAKING MCP CONTRACT` and discussed before implementation.
5. **Error handling via thiserror** — extend `SmritiError` enum, never use `unwrap()`
   in library code.
6. **Serde on all public types** — every new public struct/enum derives
   `serde::Serialize + serde::Deserialize`.
7. **Graph type is DiGraph** — do NOT switch to StableGraph without an explicit
   migration plan discussed first.
8. **vtab feature is available** — rusqlite already has `features = ["bundled", "vtab"]`,
   use it for any virtual table work (sqlite-vec, FTS5 triggers, etc.).
9. **Async via Tokio** — do not introduce a second async runtime.
10. **No Python or Node tooling** — all tooling (benchmarks, codegen, CI) stays in Rust.

---

## Do NOT Do (common failure modes to avoid)

- Do not suggest adding Neo4j, FalkorDB, or any graph database server
- Do not add Python scripts to the build process
- Do not create a second SQLite connection pool (reuse the existing one)
- Do not break the `[[wiki-link]]` regex contract in src/parser/
- Do not add `unwrap()` in any file under src/ (use `?` and thiserror)
- Do not add new Tokio runtimes (one global runtime via `#[tokio::main]`)
- Do not propose HTTP MCP transport before stdio is fully stable
- Do not claim vector search capability until sqlite-vec is actually integrated
- Do not let consolidation (Task 9) DELETE any note — demotion routes through `memory_history` only. Compliance (ICH E6(R3)) requires immutable audit trail.
- Do not auto-promote a note to `Schema` without generating an abstraction + populating `schema_sources` lineage. Pinning ≠ consolidation.
- Do not recompute `consolidation_score` inside request handlers — it runs in a background task or explicit `notes_consolidate` call. Request path stays p50-friendly.
- Do not frame consolidation as "immune system memory" in external comms. Mechanism is hippocampal-neocortical (CLS, McClelland 1995). Clonal selection ≠ replay consolidation. Pick the right metaphor in public materials.

---

## Vector Search Decision (resolved — use when implementing)

**Chosen approach: sqlite-vec**

```toml
# Add to Cargo.toml when implementing vector search
sqlite-vec = "0.1"
```

```rust
// Activation pattern (rusqlite 0.31 + vtab feature — already enabled)
use sqlite_vec::sqlite3_vec_init;
use rusqlite::ffi::sqlite3_auto_extension;
unsafe {
    sqlite3_auto_extension(Some(std::mem::transmute(
        sqlite3_vec_init as *const ()
    )));
}
```

Rationale: zero new processes, same WAL, same connection pool, same .db file.
Embedding generation is CONDITIONAL: Ollama API (local, user-provided) or
POST /api/v1/notes/:id/embed (external embedding, stored only).

---

## Benchmark Baseline (March 2026, Apple Silicon, in-memory SQLite)

[QUICK-WIN] — latency/throughput only, NOT recall quality.

| Benchmark                        | p50       |
|----------------------------------|-----------|
| insert/1                         | 32.5 µs   |
| insert/100                       | 2.0 ms    |
| insert/1000                      | 23.1 ms   |
| fts5_search/1k_notes             | 331 µs    |
| fts5_search/10k_notes            | 2.86 ms   |
| graph_ops/build_1k               | 216 µs    |
| graph_ops/bfs_d2                 | 235 ns    |
| graph_ops/bfs_d3                 | 410 ns    |
| memory_kv/store (100 keys)       | 513 µs    |
| memory_kv/retrieve_hit           | 2.48 µs   |
| memory_kv/retrieve_miss          | 2.25 µs   |

Run: `cargo bench` — HTML reports in `target/criterion/`.

---

## Priority Task Queue (next sprint — research features)

Work on these in order. Do not skip ahead without completing the previous item.

### TASK 6 — Vector/Semantic Search (sqlite-vec integration)
```
Problem: No semantic search — only FTS5 keyword matching.
Fix: Add sqlite-vec = "0.1" to Cargo.toml.
     Create embeddings table (note_id, embedding BLOB).
     Add POST /api/v1/notes/:id/embed endpoint (accepts external embedding).
     Add notes_search_semantic MCP tool (hybrid: FTS5 + cosine similarity).
     Embedding generation: Ollama API (local, user-provided) — NOT built-in.
Research: Graph-Based Memory Survey arXiv:2602.05665 — hybrid beats pure vector.
Constraint: sqlite-vec via vtab feature (already enabled in rusqlite).
```

### TASK 7 — Conflict Resolution / Belief Revision on memory_store
```
Problem: memory_store blindly overwrites on key conflict (last-write-wins).
Fix: Add ConflictPolicy enum { LastWriteWins, Merge, Reject, Archive }.
     Accept optional conflict_policy field on memory_store MCP tool.
     // BREAKING MCP CONTRACT: new optional field 'conflict_policy'
     Archive policy: move old value to memory_history table before overwrite.
Research: Graph-Native Belief Revision arXiv:2603.17244 — AGM conflict resolution.
Constraint: Default to LastWriteWins for backward compat.
```

### TASK 8 — Typed Graph Layers (semantic/temporal/causal)
```
Problem: All links live in one flat graph — no separation of concerns.
Fix: Extend LinkType enum with Semantic, Temporal, Causal variants.
     Add graph layer filtering to notes_graph MCP tool and /api/v1/graph.
     Allow BFS traversal restricted to specific link types.
Research: MAGMA arXiv:2601.03236 — typed layers reduce token usage 95%.
Constraint: Same DiGraph, filter at query time — no separate graph instances.
```

### TASK 9 — Memory Consolidation + Schema Formation (CLS-inspired)
```
Problem: All notes persist equally. No mechanism to promote frequently-replayed
         knowledge into durable abstractions, or gracefully degrade orphan nodes.
         Graph grows unboundedly (same failure mode as Mem0/Zep/LangMem).
Positioning: First agent memory that gets CLEANER over time, not just bigger.

Depends on: Task 6 (embeddings — needed for context-diversity signal in replay)
            Task 7 (memory_history — required as archive target; deletion is forbidden)

Instrumentation (add FIRST, can ship incrementally from Task 6 onward):
  - note_access_log table: log every read/search_hit/graph_traverse/mcp_retrieve
  - Hook into api::notes::get, notes_read MCP, graph BFS visits, FTS5 hits.
  - Async write — do NOT block request path (tokio::spawn + bounded channel).

Scoring (runs in background, not request path):
  consolidation_score = sigmoid(
        w1 * log1p(access_count)                 // replay frequency
      + w2 * log1p(degree)                       // structural centrality
      + w3 * context_diversity                   // unique semantic contexts (cosine on query_embedding clusters; needs Task 6)
      + w4 * exp(-Δt / τ)                        // recency (τ ~ 30 days, tuneable)
  )
  // NO division by age. Old + replayed + central = schema. That's the point.
  // Weights default w1=0.35, w2=0.25, w3=0.30, w4=0.10. Expose via config.toml.

Policy:
  ConsolidationPolicy enum {
      Conservative,  // never auto-promote, only flag; default
      Standard,      // promote on threshold, flag low-score for review
      Aggressive,    // auto-archive flagged items after grace period
  }
  Default = Conservative for any healthcare/compliance profile.

Promotion to Schema (the actual abstraction step):
  When ≥ N episodes (default 3) cluster by embedding similarity AND their
  combined consolidation_score exceeds threshold:
    1. Call user-configured LLM (Ollama by default) to generate abstracted summary.
    2. Create new note with node_type='schema'.
    3. Populate schema_sources table with lineage + similarity scores.
    4. Set parent_schema_id on source episodes (they remain queryable).
    5. Write consolidation_events row with reason.
  If no LLM configured → skip abstraction, only flag cluster (no silent failure).

Demotion (forgetting):
  - Below threshold + no recent access → flag_for_review state (not deleted).
  - After configurable grace period + user opt-in → archive into memory_history.
  - NEVER hard-delete. ICH E6(R3) trail must remain reconstructable.

MCP: notes_consolidate { dry_run: bool, policy: ConsolidationPolicy, agent_id?: str }
     Returns: { promoted: [...], flagged: [...], archived: [...], reasons: {...} }
     // BREAKING MCP CONTRACT check: this is a NEW tool, no contract change to existing.

Observability:
  - Every promotion/flag/archive writes a consolidation_events row with reason.
  - GET /api/v1/consolidation/events is the audit trail endpoint.
  - `smriti consolidate --explain <note_id>` CLI command shows the score breakdown.

Research: CLS — McClelland, McNaughton, O'Reilly 1995 Psych Review;
          Kumaran, Hassabis, McClelland 2016 Trends in Cognitive Sciences.
          Episodes (hippocampus) → replay → schemas (neocortex) with abstraction,
          NOT immune clonal selection (do not conflate in docs/marketing).

Non-goals (explicit):
  - Do NOT build embedding generation into Smriti (stays Ollama/external).
  - Do NOT run consolidation on every write (periodic or manual only).
  - Do NOT conflate this with TTL — TTL is hard expiry, consolidation is soft.

Clinical trials unlock (validates priority):
  - Protocol amendment notes = low-frequency-access but legally required.
    → Conservative policy + memory_history archive = zero data loss.
  - Bi-temporal edges (Task 7) + schema lineage = full reconstructable audit trail.
  - Matches ICH E6(R3) §4.1 (essential records) and §8 (data integrity).
```

---

## Session Startup Checklist

When starting a new Claude Code session on this project, confirm:
1. Which task from the Priority Task Queue are we working on?
2. Are there any open TODOs or ASSUMES comments from the last session?
3. Has any struct/schema changed that invalidates the types above?
4. Run `cargo test --all` to confirm green baseline.

---

## Commit Message Convention

```
feat(graph): add GraphCache to stop full DiGraph rebuilds on every query
fix(storage): correct FTS5 trigger on note update
feat(mcp): add notes_search_semantic tool (sqlite-vec hybrid)
  BREAKING MCP CONTRACT: new required field 'embedding_model'
chore(ci): add GitHub Actions workflow
docs(readme): add Research Foundation section with arXiv citations
bench: add criterion suite for insert/search/graph ops
```

---

## Quick Reference: How to Start Each Task

```bash
# Task 6: Vector search
# Edit: Cargo.toml — add sqlite-vec = "0.1"
# Edit: src/storage/db.rs — register sqlite-vec extension, add embeddings table
# Create: src/api/routes/embed.rs — POST /api/v1/notes/:id/embed
# Edit: src/mcp/ — add notes_search_semantic tool
# Test: cargo test storage, cargo test mcp

# Task 7: Conflict resolution
# Edit: src/models/agent.rs — add ConflictPolicy enum
# Edit: src/storage/operations.rs — update store_memory with policy logic
# Create: migration for memory_history table
# Edit: src/mcp/ — add conflict_policy param to memory_store
# Test: cargo test storage::operations

# Task 8: Typed graph layers
# Edit: src/models/link.rs — extend LinkType with Semantic, Temporal, Causal
# Edit: src/graph/knowledge_graph.rs — add filter_by_type() on traversals
# Edit: src/api/routes/graph.rs — accept ?layer= query param
# Edit: src/mcp/ — add layer param to notes_graph tool
# Test: cargo test graph, cargo test mcp

# Task 9: Consolidation + Schema Formation (CLS)
# Phase 1 — Instrument (can ship with Task 6):
#   Migration: add node_type/consolidation_score/access_count/last_accessed_at/parent_schema_id to notes
#   Migration: create note_access_log, schema_sources, consolidation_events
#   Edit: src/storage/operations.rs — log access on every read path (async, non-blocking)
#   Edit: src/api/routes/notes.rs — bump access_count on GET, expose POST /:id/access
# Phase 2 — Scoring (requires Task 6 for context_diversity):
#   Create: src/features/consolidation.rs — scoring fn + background tokio task
#   Edit: src/features/mod.rs — register module
# Phase 3 — Promotion (requires Task 7 memory_history):
#   Create: src/features/schema_formation.rs — embedding cluster → LLM abstraction
#   Edit: src/mcp/ — add notes_consolidate tool
#   Edit: src/cli/ — add `smriti consolidate` + `smriti consolidate --explain <id>`
# Test: cargo test features::consolidation, integration test the full lifecycle
```

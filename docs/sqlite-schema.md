# SQLite Schema

Smriti stores all data in a single SQLite file with foreign keys enabled and WAL mode for concurrent reads.

## Core Tables

### `notes`

Primary content store.

```sql
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  -- Task 9: Consolidation (CLS-inspired)
  node_type TEXT NOT NULL DEFAULT 'episode',  -- 'episode' | 'schema'
  consolidation_score REAL NOT NULL DEFAULT 0.0,
  access_count INTEGER NOT NULL DEFAULT 0,
  last_accessed_at TEXT,
  parent_schema_id TEXT REFERENCES notes(id) ON DELETE SET NULL
);

CREATE INDEX idx_notes_consolidation ON notes(node_type, consolidation_score DESC);
```

**Fields:**
- `node_type`: `episode` (default, raw note) or `schema` (consolidated abstraction)
- `consolidation_score`: 0.0–1.0, computed from cascade salience + degree + context diversity
- `access_count`: Lifetime read/traversal hits (denormalized from `note_access_log`)
- `last_accessed_at`: ISO 8601 timestamp of most recent access
- `parent_schema_id`: For episodes subsumed into a schema; `NULL` for standalone notes

---

### `notes_fts`

Full-text search index (FTS5, Porter stemming, Unicode61 tokenizer).

```sql
CREATE VIRTUAL TABLE notes_fts USING fts5(
  title, content,
  content=notes, content_rowid=rowid,
  tokenize='porter unicode61'
);
```

Automatically synchronized with `notes` via triggers.

---

### `tags`

Tag registry.

```sql
CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  color TEXT,
  created_at TEXT NOT NULL
);
```

---

### `note_tags`

Many-to-many join table.

```sql
CREATE TABLE note_tags (
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (note_id, tag_id)
);
```

---

### `links`

Typed directed edges between notes.

```sql
CREATE TABLE links (
  id TEXT PRIMARY KEY,
  source_note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  target_note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  link_type TEXT NOT NULL,  -- 'wikilink' | 'semantic' | 'causal' | 'temporal' | ...
  created_at TEXT NOT NULL,
  -- Bi-temporal edges (Zep/Graphiti arXiv:2501.13956)
  valid_from TEXT,    -- When the relationship became valid
  valid_until TEXT,   -- NULL = currently valid
  UNIQUE(source_note_id, target_note_id, link_type)
);
```

**Bi-temporal semantics:**
- `valid_from`: Relationship valid-time start (e.g., "treatment started on 2026-01-15")
- `valid_until`: Relationship valid-time end (e.g., "treatment stopped on 2026-02-20"); `NULL` means still valid
- Research ref: Zep/Graphiti arXiv:2501.13956 §3.2 — improves LongMemEval by 18.5%

---

## Agent Memory

### `agent_memory`

Key-value store with namespaces and TTL.

```sql
CREATE TABLE agent_memory (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  namespace TEXT NOT NULL DEFAULT 'default',
  key TEXT NOT NULL,
  value TEXT NOT NULL,  -- JSON serialized
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  ttl_seconds INTEGER,
  UNIQUE(agent_id, namespace, key)
);
```

---

### `memory_history`

Superseded values (AGM belief revision, arXiv:2603.17244).

```sql
CREATE TABLE memory_history (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  namespace TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  superseded_at TEXT NOT NULL,
  superseded_by TEXT NOT NULL,  -- ID of the new entry
  created_at TEXT NOT NULL
);

CREATE INDEX idx_memory_history_lookup
  ON memory_history(agent_id, namespace, key, superseded_at DESC);
```

**Conflict policies:**
- `overwrite`: Replace value, no history (default)
- `reject`: Fail if key exists
- `version_and_keep`: Archive old value to `memory_history`, then overwrite
- `invalidate`: Same as `version_and_keep`

---

### `tool_logs`

Agent tool invocation log.

```sql
CREATE TABLE tool_logs (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  input TEXT NOT NULL,   -- JSON
  output TEXT NOT NULL,  -- JSON
  status TEXT NOT NULL,  -- 'success' | 'error' | 'timeout'
  duration_ms INTEGER,
  created_at TEXT NOT NULL
);
```

---

## Semantic Search (Migration 002)

### `notes_vec`

sqlite-vec virtual table for vector embeddings.

```sql
CREATE VIRTUAL TABLE notes_vec USING vec0(
  note_id TEXT PRIMARY KEY,
  embedding float[384]
);
```

Dimensionality is configurable. Smriti does NOT generate embeddings — use Ollama or an external service.

---

### `note_embeddings_meta`

Tracks embedding model and dimensions.

```sql
CREATE TABLE note_embeddings_meta (
  note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
  dimensions INTEGER NOT NULL,
  model TEXT,
  created_at TEXT NOT NULL
);
```

---

## Integrity Layer (Migrations 004–008)

### `sources`

Provenance sources (FACTUM arXiv:2601.05866).

```sql
CREATE TABLE sources (
  id TEXT PRIMARY KEY,
  uri TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  title TEXT,
  excerpt TEXT,
  ingested_at TEXT NOT NULL,
  UNIQUE(uri, content_hash)
);
```

---

### `claim_spans`

Structural overlap between claims and sources.

```sql
CREATE TABLE claim_spans (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  claim_start INTEGER NOT NULL,
  claim_end INTEGER NOT NULL,
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE RESTRICT,
  source_span_start INTEGER,
  source_span_end INTEGER,
  verification_score REAL NOT NULL,
  verified_at TEXT NOT NULL,
  method TEXT NOT NULL
);

CREATE INDEX idx_claim_spans_note ON claim_spans(note_id);
CREATE INDEX idx_claim_spans_source ON claim_spans(source_id);
```

**Verification methods:**
- `exact_substring`: Source span exactly matches claim span
- `fuzzy_overlap`: Jaccard similarity above threshold
- `semantic_cosine`: Embedding cosine similarity (requires embeddings)

---

### `wiki_transactions`

Atomic multi-write transactions (GitHub-for-memory pattern).

```sql
CREATE TABLE wiki_transactions (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  status TEXT NOT NULL,  -- 'pending' | 'committed' | 'rejected'
  operations TEXT NOT NULL,  -- JSON array
  rationale TEXT,
  created_at TEXT NOT NULL,
  committed_at TEXT,
  rejected_at TEXT,
  rejected_by TEXT,
  rejection_reason TEXT
);

CREATE INDEX idx_wiki_tx_status ON wiki_transactions(status, created_at DESC);
CREATE INDEX idx_wiki_tx_agent ON wiki_transactions(agent_id, created_at DESC);
```

**Operations:**
- `create`: New note with optional claim_spans
- `update`: Patch existing note
- `link`: Create typed edge
- `source`: Register provenance source

All operations execute inside a SQLite `SAVEPOINT`. Rejected transactions roll back atomically.

---

### `contradiction_events`

Contradiction candidates for human review (MemoTime arXiv:2510.13614).

```sql
CREATE TABLE contradiction_events (
  id TEXT PRIMARY KEY,
  note_id_a TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  note_id_b TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  semantic_score REAL NOT NULL,
  recency_score REAL NOT NULL,
  authority_score REAL NOT NULL,
  combined_score REAL NOT NULL,  -- w1·semantic + w2·recency + w3·authority
  status TEXT NOT NULL DEFAULT 'open',  -- 'open' | 'resolved' | 'false_positive'
  detected_at TEXT NOT NULL,
  resolved_at TEXT,
  resolution TEXT,
  UNIQUE(note_id_a, note_id_b)
);

CREATE INDEX idx_contradiction_status
  ON contradiction_events(status, combined_score DESC);
```

**Smriti never auto-resolves.** All candidates land in a review inbox.

---

### `events`

Append-only event log with hash chain (Zep T/T′ semantics).

```sql
CREATE TABLE events (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  payload TEXT NOT NULL,
  created_at TEXT NOT NULL,
  prev_hash TEXT,
  event_hash TEXT NOT NULL
);

CREATE INDEX idx_events_entity ON events(entity_type, entity_id, created_at DESC);
CREATE INDEX idx_events_created ON events(created_at DESC);
```

**Hash chain:**
- `prev_hash`: SHA-256 of previous event (or NULL for genesis)
- `event_hash`: SHA-256(prev_hash || event_type || entity_id || payload || created_at)

Verified by `wiki_verify` / `smriti verify` CLI.

---

### `agent_grants`

ACL for multi-agent systems.

```sql
CREATE TABLE agent_grants (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  resource_type TEXT NOT NULL,  -- 'note' | 'tag' | 'source'
  resource_id TEXT NOT NULL,
  permission TEXT NOT NULL,     -- 'read' | 'write' | 'delete'
  granted_at TEXT NOT NULL,
  granted_by TEXT NOT NULL,
  UNIQUE(agent_id, resource_type, resource_id, permission)
);
```

---

## Consolidation (Migration 009, Task 9)

### `note_access_log`

Feeds the consolidation replay signal (CLS, McClelland 1995).

```sql
CREATE TABLE note_access_log (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  accessed_at TEXT NOT NULL,
  access_kind TEXT NOT NULL,  -- 'read' | 'search_hit' | 'graph_traverse' | 'mcp_retrieve'
  query_context TEXT,         -- Raw query string (for context diversity)
  query_embedding BLOB,       -- Optional: semantic context diversity (Task 6 dependency)
  agent_id TEXT
);

CREATE INDEX idx_access_note_time ON note_access_log(note_id, accessed_at DESC);
```

**Instrumentation:**
- `notes_read` MCP tool → `read`
- `notes_search` hits → `search_hit`
- `notes_graph` BFS visits → `graph_traverse`
- `retrieve_context` → `mcp_retrieve`

---

### `schema_sources`

Lineage: which episodes a schema was abstracted from.

```sql
CREATE TABLE schema_sources (
  schema_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  source_note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  similarity_score REAL,
  consolidated_at TEXT NOT NULL,
  PRIMARY KEY (schema_id, source_note_id)
);
```

---

### `consolidation_events`

Auditable consolidation decisions (ICH E6(R3) trail).

```sql
CREATE TABLE consolidation_events (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,  -- 'promoted_to_schema' | 'flagged_for_review' | 'archived' | 'score_recomputed'
  score_before REAL,
  score_after REAL,
  reason TEXT NOT NULL,  -- Human-readable rationale
  created_at TEXT NOT NULL
);
```

---

### `cascade_state` (Migration 011)

Benna-Fusi multi-timescale cascade synapses (Nature Neuroscience 19, 1697–1706).

```sql
CREATE TABLE cascade_state (
  note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
  levels_json TEXT NOT NULL,  -- JSON array of u_k values
  last_updated TEXT NOT NULL
);
```

**Cascade levels (K=10):**
- `u_0`: Fast synapse (τ ≈ minutes)
- `u_1` to `u_9`: Progressively slower timescales (τ up to years)

Salience readout: weighted sum `Σ w_k * u_k`.

---

## LLM Audit (Migration 010)

### `llm_audit`

Tracks all LLM calls (required for clinical compliance).

```sql
CREATE TABLE llm_audit (
  id TEXT PRIMARY KEY,
  backend TEXT NOT NULL,  -- 'ollama' | 'gemma' | 'mock'
  model TEXT NOT NULL,
  prompt TEXT NOT NULL,
  response TEXT,
  outcome TEXT NOT NULL,  -- 'success' | 'error' | 'timeout'
  error_message TEXT,
  duration_ms INTEGER,
  created_at TEXT NOT NULL,
  agent_id TEXT
);

CREATE INDEX idx_llm_audit_backend ON llm_audit(backend, created_at DESC);
CREATE INDEX idx_llm_audit_outcome ON llm_audit(outcome, created_at DESC);
```

---

## Timestamps

All timestamps are ISO 8601 strings in UTC:
```
2026-08-29T00:30:00Z
```

---

## Indexes

Indexes are created for:
- Foreign key columns (automatic with `PRAGMA foreign_keys=ON`)
- Common query patterns (tag lookups, temporal queries, consolidation score ranking)
- Event log lookups (entity_type + entity_id + created_at)

---

## Transactions

Smriti uses SQLite's default isolation level (`SERIALIZABLE`). All multi-step operations (e.g., `wiki_transaction_submit`) execute inside a `SAVEPOINT` for atomicity.

---

## WAL Mode

Write-Ahead Logging is enabled for concurrent read performance:
```sql
PRAGMA journal_mode=WAL;
```

---

## Foreign Keys

```sql
PRAGMA foreign_keys=ON;
```

Cascading deletes ensure referential integrity. `ON DELETE RESTRICT` on `claim_spans.source_id` prevents accidental source deletion.

---

## Backup

The entire database is one file. Back up with:
```bash
cp smriti.db smriti-backup-$(date +%Y%m%d).db
```

For online backup (safe while Smriti is running):
```bash
sqlite3 smriti.db ".backup smriti-backup.db"
```

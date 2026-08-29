# MCP Tools Reference

Smriti provides 18 MCP tools over JSON-RPC 2.0 (stdio or HTTP). All tools return structured JSON responses.

## Setup

### Claude Desktop (stdio)

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "smriti": {
      "command": "smriti",
      "args": ["mcp", "--db", "/absolute/path/to/notes.db"]
    }
  }
}
```

Run `smriti init` to print this config for your database path.

### Remote MCP (HTTP)

```bash
smriti serve --port 3000
```

Point your MCP client to `http://localhost:3000/mcp`.

---

## Core Note Operations

### `notes_create`

Create a new note. `[[wiki-links]]` and `#tags` are auto-extracted from content.

**Parameters:**
- `title` (string, required): Note title
- `content` (string, required): Note body (markdown)
- `tags` (array\<string\>, optional): Tags to apply

**Example:**
```json
{
  "title": "Protocol Amendment Log",
  "content": "Amendment 03 submitted. Related to [[rel:causal|Inclusion Criterion Change]]. #protocol #amendment",
  "tags": ["regulatory"]
}
```

**Returns:** Note object with `id`, `title`, `content`, `created_at`, `tags`, `backlink_count`.

---

### `notes_read`

Read a note by ID or exact title. Instruments an access event for consolidation.

**Parameters:**
- `id` (string, required): Note ID or exact title

**Example:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Returns:** Note object.

---

### `notes_search`

Full-text BM25 search via SQLite FTS5. Instruments search hits for consolidation.

**Parameters:**
- `query` (string, required): Search query (supports FTS5 operators: `AND`, `OR`, `NOT`, `NEAR`)
- `limit` (number, optional): Max results (default 10)

**Example:**
```json
{
  "query": "protocol amendment",
  "limit": 5
}
```

**Returns:** Array of notes ranked by BM25 score.

---

### `notes_search_semantic`

Hybrid semantic + BM25 search via sqlite-vec. Requires embeddings stored via `store_embedding` (external).

**Parameters:**
- `query` (string, required): Query text
- `embedding` (array\<number\>, optional): Pre-computed embedding (384-dim float32)
- `limit` (number, optional): Max results (default 10)
- `fts_weight` (number, optional): BM25 weight in RRF (default 0.4)
- `vec_weight` (number, optional): Cosine weight in RRF (default 0.6)

**Example:**
```json
{
  "query": "adverse event aspirin",
  "limit": 5
}
```

**Returns:** Array of notes with `score` (hybrid RRF rank).

---

### `notes_list`

List recent notes, optionally filtered by tag.

**Parameters:**
- `limit` (number, optional): Max notes (default 20)
- `tag` (string, optional): Filter by tag name

**Example:**
```json
{
  "limit": 10,
  "tag": "protocol"
}
```

**Returns:** Array of notes.

---

### `notes_graph`

Retrieve a subgraph around a note via BFS traversal. Supports typed edge filtering.

**Parameters:**
- `center` (string, optional): Note ID to center on (omit for full graph)
- `depth` (number, optional): BFS depth (default 2)
- `link_type` (string, optional): Filter edges by type (`wikilink`, `semantic`, `causal`, `temporal`)

**Example:**
```json
{
  "center": "550e8400-e29b-41d4-a716-446655440000",
  "depth": 2,
  "link_type": "causal"
}
```

**Returns:** `{ nodes: [...], edges: [...] }` in Cytoscape JSON format.

---

## Agent Memory (KV Store)

### `memory_store`

Store a key-value pair for an agent. Supports TTL and conflict resolution policies.

**Parameters:**
- `agent_id` (string, required): Agent identifier
- `namespace` (string, optional): Namespace (default `"default"`)
- `key` (string, required): Key
- `value` (any, required): JSON value
- `ttl_seconds` (number, optional): Time-to-live
- `conflict_policy` (string, optional): `overwrite` (default), `reject`, `version_and_keep`, `invalidate`

**Example:**
```json
{
  "agent_id": "trial-coordinator-bot",
  "namespace": "protocol",
  "key": "current_version",
  "value": "v2.3",
  "conflict_policy": "version_and_keep"
}
```

**Returns:** `{ success: true }` or error if `reject` policy blocks overwrite.

---

### `memory_retrieve`

Retrieve a value by agent_id + namespace + key.

**Parameters:**
- `agent_id` (string, required)
- `namespace` (string, optional): Default `"default"`
- `key` (string, required)

**Example:**
```json
{
  "agent_id": "trial-coordinator-bot",
  "key": "current_version"
}
```

**Returns:** `{ value: <JSON> }` or `null` if not found.

---

### `memory_list`

List all memory entries for an agent.

**Parameters:**
- `agent_id` (string, required)
- `namespace` (string, optional): Filter by namespace

**Example:**
```json
{
  "agent_id": "trial-coordinator-bot",
  "namespace": "protocol"
}
```

**Returns:** Array of `{ namespace, key, value, created_at, updated_at, ttl_seconds }`.

---

### `memory_history`

Retrieve superseded values for a key (versioned memory, AGM belief revision).

**Parameters:**
- `agent_id` (string, required)
- `namespace` (string, optional): Default `"default"`
- `key` (string, required)

**Example:**
```json
{
  "agent_id": "trial-coordinator-bot",
  "key": "protocol_version"
}
```

**Returns:** Array of superseded entries with `value`, `superseded_at`, `superseded_by`.

---

## Wiki Integrity Layer

### `wiki_transaction_submit`

Submit an atomic multi-write transaction. All operations execute inside a SQLite `SAVEPOINT`. Enforces provenance: every claim must cite a source span.

**Parameters:**
- `agent_id` (string, required)
- `operations` (array, required): Array of `{ op: "create"|"update"|"link"|"source", ... }` objects
- `rationale` (string, optional): Human-readable reason

**Example:**
```json
{
  "agent_id": "protocol-writer",
  "operations": [
    {
      "op": "create",
      "title": "Adverse Event Report",
      "content": "Patient reported mild nausea.",
      "claim_spans": [
        {
          "claim_start": 0,
          "claim_end": 31,
          "source_id": "src-12345",
          "source_span_start": 120,
          "source_span_end": 151
        }
      ]
    }
  ],
  "rationale": "Logged AE from site visit"
}
```

**Returns:** `{ transaction_id: "<uuid>", status: "pending" }`

---

### `wiki_transaction_commit`

Commit a pending transaction. Verifies all claim spans before applying.

**Parameters:**
- `transaction_id` (string, required)

**Returns:** `{ committed: true, created_note_ids: [...], verified_claim_count: N }`

---

### `wiki_transaction_reject`

Reject a pending transaction with a reason.

**Parameters:**
- `transaction_id` (string, required)
- `rejected_by` (string, optional): Default `"mcp-client"`
- `reason` (string, required)

**Returns:** `{ rejected: true }`

---

### `wiki_transaction_list_pending`

List pending transactions awaiting review.

**Parameters:**
- `limit` (number, optional): Default 50

**Returns:** Array of pending transactions.

---

### `wiki_verify`

Run full integrity sweep: referential integrity, provenance re-verification, event log hash chain, orphan detection. Never mutates.

**Parameters:** (none)

**Returns:**
```json
{
  "ok": true,
  "stats": { "notes": 128, "links": 47, "sources": 12, "claim_spans": 94, "events": 203, "grounded_notes": 89 },
  "referential_errors": [],
  "provenance_failures": [],
  "event_chain_errors": [],
  "orphan_notes": []
}
```

---

## Contradiction Detection

### `contradictions_detect`

Scan recent notes for contradiction candidates using MemoTime-style weighted scoring (semantic + recency + authority). Candidates land in a review inbox. Smriti never auto-resolves.

**Parameters:**
- `scan_limit` (number, optional): Max notes to scan pairwise (default 50)

**Returns:** Array of detected candidates with `combined_score`, `note_id_a`, `note_id_b`.

---

### `contradictions_list`

List open contradiction candidates for human review.

**Parameters:**
- `limit` (number, optional): Default 50

**Returns:** Array of open contradictions.

---

## Consolidation (CLS-Inspired Schema Formation)

### `notes_consolidate`

Run a memory consolidation pass. Scores episode notes based on cascade salience + degree + context diversity, then promotes eligible clusters to extractive schemas (Standard/Aggressive policy) or flags for review (Conservative, the default).

**Parameters:**
- `policy` (string, optional): `conservative` (default), `standard`, `aggressive`
- `dry_run` (boolean, optional): Compute scores but don't persist (default `true`)
- `agent_id` (string, optional): Agent identifier for audit trail

**Example:**
```json
{
  "policy": "standard",
  "dry_run": false
}
```

**Returns:**
```json
{
  "dry_run": false,
  "policy": "standard",
  "scanned": 42,
  "promoted": ["schema-id-1"],
  "flagged": ["note-id-2", "note-id-3"],
  "archived": [],
  "reasons": {
    "schema-id-1": "extractive schema over 3 episodes: Schema: AE aspirin (+2 more)",
    "note-id-2": "score 0.123 < flag_below 0.150 (salience=0.0124 degree=1)"
  }
}
```

**CLI alternative:** `smriti consolidate --policy conservative --apply`

Conservative never writes schema notes. Standard/Aggressive may auto-commit only if the retrieve-context proxy improves held-out `query_context` samples. That proxy is **not** WikiSkill task-accuracy. Optional MCP fields: `accept_proposal_id`, `reject_proposal_id`.

---

## Local-First LLM Retrieval (Path A)

### `ingest_document`

Chunk a `.txt` or `.md` file into the knowledge graph. Creates a parent document note + chunk notes + `ChunkOf` typed links. No LLM required.

**Parameters:**
- `path` (string, required): File path
- `chunk_size` (number, optional): Max chars per chunk (default 512)

**Returns:** `{ parent_note_id: "<uuid>", chunk_count: N }`

---

### `retrieve_context`

Query → hybrid search + BFS graph expansion + context assembly → context string for local LLM.

**Parameters:**
- `query` (string, required)
- `top_k` (number, optional): Search results to expand from (default 5)
- `graph_depth` (number, optional): BFS depth (default 1)

**Returns:** `{ context: "<assembled markdown>", source_note_ids: [...] }`

---

## Research References

Every feature cites the arXiv paper that grounds its design:

- **Bi-temporal edges** — Zep/Graphiti, arXiv:2501.13956
- **Provenance enforcement** — FACTUM, arXiv:2601.05866
- **Contradiction detection** — MemoTime, arXiv:2510.13614
- **Belief revision** — AGM postulates, arXiv:2603.17244
- **Hybrid search** — Graph-Based Memory Survey, arXiv:2602.05665
- **Consolidation** — CLS (McClelland 1995), Benna & Fusi 2016 (cascade synapses)
- **Schema formation** — WikiSkill, arXiv:2608.27454 (architecture reference)

---

## Usage Notes

1. **Isolation**: Each team runs `smriti init` to create their own `.db` file. No shared state.
2. **Embeddings**: Smriti does NOT generate embeddings. Use Ollama or an external embedding service, then store via direct SQLite or a future REST endpoint.
3. **Consolidation**: Run `smriti consolidate --dry-run` in CI as a memory hygiene check. Conservative policy (default) never auto-promotes.
4. **Provenance**: `wiki_transaction_submit` rejects writes without `claim_spans`. For non-grounded notes, use `notes_create` (no provenance enforcement).
5. **Healthcare compliance**: Conservative policy + bi-temporal edges + event log hash chain + `wiki_verify` = ICH E6(R3) §4.1 audit trail.

---

**Full documentation:** https://github.com/biosync-tech/smriti

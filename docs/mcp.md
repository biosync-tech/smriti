# MCP tools

Works with any MCP client (Claude Desktop, Cursor, Codex, custom). Stdio: `smriti mcp`. Optional override: `--db <path>` or `SMRITI_DB`. Default file: `~/.smriti/smriti.db`. HTTP: `POST /mcp` when `smriti serve` is running.

Daily agent loop: `smriti_status` → `notes_create` / `notes_search` / `retrieve_context` → `memory_store` if you need scratch. Do not send embeddings unless you already have them.

Optional fields on existing tools are additive. A net-new tool is not a breaking change.

## Status

### `smriti_status`

```json
{}
```

Returns note count and which tools to use. Call first.

## Knowledge graph

### `notes_create`

```json
{ "title": "Acme Q2", "content": "Owner: [[Sarah Chen]]. #client", "tags": ["client"] }
```

Wiki-links and `#tags` are extracted automatically.

### `notes_read`

```json
{ "id": "Acme Q2" }
```

Accepts id or exact title. Writes `note_access_log` (`mcp_retrieve`).

### `notes_search`

```json
{ "query": "Acme", "limit": 10 }
```

FTS5 / BM25. Search hits are logged for consolidation.

### `notes_list`

```json
{ "limit": 20, "tag": "client" }
```

### `notes_graph`

```json
{ "center": "<note_id>", "depth": 2, "link_type": "causal" }
```

`link_type` is optional. BFS over currently-valid edges.

### `notes_search_semantic`

```json
{ "query": "budget approval", "embedding": [0.1, 0.2], "top_k": 10, "fts_weight": 0.5 }
```

Hybrid sqlite-vec + FTS5 with reciprocal rank fusion. Embedding is supplied by the caller (Ollama or external). Smriti does not embed on the request path unless you configured a local backend.

### `notes_consolidate`

```json
{ "dry_run": true, "policy": "conservative" }
```

Scores episodes. Conservative (default) flags schema proposals for human review — it never auto-promotes.

```json
{ "accept_proposal_id": "<uuid>", "approved_by": "you" }
```

```json
{ "reject_proposal_id": "<uuid>", "reject_reason": "too vague", "approved_by": "you" }
```

`accept_proposal_id` / `reject_proposal_id` are optional new fields, not a contract break.

## Agent memory

### `memory_store`

```json
{
  "agent_id": "claude",
  "namespace": "default",
  "key": "current_focus",
  "value": { "project": "Smriti" },
  "ttl_seconds": 86400,
  "conflict_policy": "version_and_keep"
}
```

`conflict_policy`: `overwrite` (default), `reject`, `version_and_keep`, `invalidate`.

### `memory_retrieve`

```json
{ "agent_id": "claude", "namespace": "default", "key": "current_focus" }
```

### `memory_list`

```json
{ "agent_id": "claude", "namespace": "default" }
```

### `memory_history`

```json
{ "agent_id": "claude", "namespace": "default", "key": "current_focus" }
```

## Wiki transactions

### `wiki_transaction_submit`

Atomic multi-write inside a SQLite `SAVEPOINT`. Content writes should carry `claim_spans`.

### `wiki_transaction_commit` / `wiki_transaction_reject`

```json
{ "transaction_id": "<id>" }
```

```json
{ "transaction_id": "<id>", "by": "human", "reason": "missing source" }
```

### `wiki_transaction_list_pending`

```json
{ "limit": 50 }
```

### `wiki_verify`

Referential integrity + provenance overlap + event-log hash chain. Never mutates.

## Contradictions

### `contradictions_detect`

```json
{ "scan_limit": 50 }
```

Candidates only. Smriti never auto-resolves.

### `contradictions_list`

```json
{ "limit": 50 }
```

## Local KG (Path A)

### `ingest_document`

```json
{ "path": "/abs/path/protocol.md", "tags": ["protocol"], "chunk_size": 1200 }
```

No LLM. Parent document note + chunk notes + `ChunkOf` links.

### `retrieve_context`

```json
{ "query": "inclusion criteria", "top_k": 10, "graph_depth": 1 }
```

Assembles a context string. The calling LLM owns generation.

Pass `embedding` for hybrid search. Committed schema notes may be ranked first. **Pending schema proposals are not notes** and cannot leak into this tool.

# REST API Reference

Smriti exposes a REST API at `http://localhost:3000/api/v1/` when running `smriti serve`.

## Authentication

None. Smriti is designed for single-user or small-team local deployments. For production use, deploy behind a reverse proxy with authentication.

## Base URL

```
http://localhost:3000/api/v1/
```

## Notes

### `POST /api/v1/notes`

Create a new note.

**Request:**
```json
{
  "title": "Protocol Amendment Log",
  "content": "Amendment 03 submitted for [[Inclusion Criterion Change]].",
  "tags": ["protocol", "regulatory"]
}
```

**Response:** `201 Created`
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Protocol Amendment Log",
  "content": "Amendment 03 submitted for [[Inclusion Criterion Change]].",
  "created_at": "2026-08-29T00:00:00Z",
  "updated_at": "2026-08-29T00:00:00Z",
  "tags": ["protocol", "regulatory"],
  "backlink_count": 0,
  "wikilink_count": 1,
  "node_type": "episode",
  "consolidation_score": 0.0,
  "access_count": 0,
  "last_accessed_at": null,
  "parent_schema_id": null
}
```

---

### `GET /api/v1/notes`

List notes with optional filtering.

**Query Parameters:**
- `limit` (number): Max notes to return (default 20)
- `tag` (string): Filter by tag name

**Example:**
```
GET /api/v1/notes?limit=10&tag=protocol
```

**Response:** `200 OK`
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "Protocol Amendment Log",
    ...
  }
]
```

---

### `GET /api/v1/notes/:id`

Read a single note by ID or exact title.

**Response:** `200 OK` or `404 Not Found`

---

### `PUT /api/v1/notes/:id`

Update a note.

**Request:**
```json
{
  "title": "Updated Title",
  "content": "Updated content",
  "tags": ["updated"]
}
```

**Response:** `200 OK`

---

### `DELETE /api/v1/notes/:id`

Delete a note. **Warning:** Hard delete, not compatible with ICH E6(R3) audit trail requirements. Use `wiki_transactions` for clinical use cases.

**Response:** `204 No Content`

---

### `GET /api/v1/notes/search`

Full-text search via SQLite FTS5.

**Query Parameters:**
- `q` (string, required): Search query
- `limit` (number): Max results (default 10)

**Example:**
```
GET /api/v1/notes/search?q=protocol+amendment&limit=5
```

**Response:** `200 OK`
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "Protocol Amendment Log",
    "snippet": "Amendment 03 submitted...",
    "rank": 0.95
  }
]
```

---

## Links

### `GET /api/v1/notes/:id/backlinks`

Get notes that link TO this note.

**Response:** `200 OK`
```json
[
  {
    "id": "link-id",
    "source_note_id": "other-note-id",
    "target_note_id": "550e8400-e29b-41d4-a716-446655440000",
    "link_type": "wikilink",
    "created_at": "2026-08-29T00:00:00Z",
    "valid_from": null,
    "valid_until": null
  }
]
```

---

### `GET /api/v1/notes/:id/links`

Get notes this note links TO.

**Response:** Same structure as backlinks.

---

## Graph

### `GET /api/v1/graph`

Retrieve the full knowledge graph or a subgraph.

**Query Parameters:**
- `limit` (number): Max nodes (default 100)
- `title_filter` (string): Filter nodes by title substring

**Response:** `200 OK`
```json
{
  "nodes": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Protocol Amendment Log",
      "tag_count": 2
    }
  ],
  "edges": [
    {
      "source": "550e8400-e29b-41d4-a716-446655440000",
      "target": "other-note-id",
      "link_type": "wikilink"
    }
  ]
}
```

---

### `GET /api/v1/graph/:id`

Subgraph centered on a note via BFS.

**Query Parameters:**
- `depth` (number): BFS depth (default 2)

**Response:** Same structure as `/graph`.

---

## Agent Memory (KV Store)

### `POST /api/v1/agent/:id/memory`

Store a key-value pair.

**Request:**
```json
{
  "namespace": "protocol",
  "key": "current_version",
  "value": "v2.3",
  "ttl_seconds": 86400
}
```

**Response:** `201 Created`

---

### `GET /api/v1/agent/:id/memory`

List all memory entries for an agent.

**Query Parameters:**
- `namespace` (string): Filter by namespace

**Response:** `200 OK`
```json
[
  {
    "namespace": "protocol",
    "key": "current_version",
    "value": "v2.3",
    "created_at": "2026-08-29T00:00:00Z",
    "updated_at": "2026-08-29T00:00:00Z",
    "ttl_seconds": 86400
  }
]
```

---

### `GET /api/v1/agent/:id/memory/:namespace/:key`

Retrieve a specific value.

**Response:** `200 OK`
```json
{
  "value": "v2.3"
}
```

---

## Tool Logs

### `POST /api/v1/agent/:id/tool-logs`

Log a tool invocation.

**Request:**
```json
{
  "tool_name": "search_protocol",
  "input": {"query": "amendment"},
  "output": {"count": 5},
  "status": "success",
  "duration_ms": 42
}
```

**Response:** `201 Created`

---

### `GET /api/v1/agent/:id/tool-logs`

List tool logs.

**Query Parameters:**
- `limit` (number): Max logs (default 50)

**Response:** `200 OK`

---

## Statistics

### `GET /api/v1/stats`

Database statistics.

**Response:** `200 OK`
```json
{
  "notes": 128,
  "links": 47,
  "tags": 12,
  "agents": 3
}
```

---

## Consolidation (Task 9)

### `POST /api/v1/consolidation/run`

Trigger a consolidation pass.

**Request:**
```json
{
  "policy": "conservative",
  "dry_run": true
}
```

**Response:** `200 OK`
```json
{
  "dry_run": true,
  "policy": "conservative",
  "scanned": 42,
  "promoted": [],
  "flagged": ["note-id-1", "note-id-2"],
  "archived": [],
  "reasons": {
    "note-id-1": "score 0.123 < flag_below 0.150"
  }
}
```

---

### `GET /api/v1/consolidation/events`

Audit log of consolidation events.

**Query Parameters:**
- `limit` (number): Max events (default 100)

**Response:** `200 OK`
```json
[
  {
    "id": "event-id",
    "note_id": "note-id",
    "event_type": "flagged_for_review",
    "score_before": 0.0,
    "score_after": 0.123,
    "reason": "below threshold",
    "created_at": "2026-08-29T00:00:00Z"
  }
]
```

---

### `GET /api/v1/notes/:id/lineage`

For schema notes: list source episodes with similarity scores.

**Response:** `200 OK`
```json
{
  "schema_id": "schema-id",
  "sources": [
    {
      "note_id": "episode-1",
      "similarity_score": 0.89,
      "consolidated_at": "2026-08-29T00:00:00Z"
    }
  ]
}
```

---

### `POST /api/v1/notes/:id/access`

Instrument an external access event (feeds consolidation replay signal).

**Request:**
```json
{
  "access_kind": "read",
  "query_context": "protocol version",
  "agent_id": "trial-coordinator"
}
```

**Response:** `204 No Content`

---

## Document Ingestion (Path A)

### `POST /api/v1/ingest/document`

Chunk a .txt/.md file into the knowledge graph.

**Request:** `multipart/form-data`
```
file: <file binary>
chunk_size: 512
```

**Response:** `200 OK`
```json
{
  "parent_note_id": "doc-note-id",
  "chunk_count": 12
}
```

---

### `POST /api/v1/retrieve`

Query → hybrid search + BFS graph expansion → context string for local LLM.

**Request:**
```json
{
  "query": "protocol amendment process",
  "top_k": 5,
  "graph_depth": 1
}
```

**Response:** `200 OK`
```json
{
  "context": "# Protocol Amendment Log\n\nAmendment 03 submitted...",
  "source_note_ids": ["note-1", "note-2"]
}
```

---

## Error Responses

All endpoints return JSON errors:

```json
{
  "error": "Note not found: 550e8400-e29b-41d4-a716-446655440000",
  "status": 404
}
```

**Status codes:**
- `400` Bad Request
- `404` Not Found
- `409` Conflict
- `500` Internal Server Error

---

## CORS

CORS is enabled for `localhost` origins only. For production, configure a reverse proxy.

---

## Rate Limiting

None. Deploy behind a reverse proxy for production rate limiting.

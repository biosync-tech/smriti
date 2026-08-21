# Smriti: Complete Platform Capabilities

> **The self-hosted knowledge graph and memory layer purpose-built for AI agents.**
> Single binary. SQLite only. MCP-native. Zero cloud dependencies.

---

## Executive Summary

Smriti is a Rust-based, self-hosted knowledge infrastructure platform that gives AI agents persistent memory, structured knowledge, and graph-native reasoning capabilities. It ships as a single binary with an embedded SQLite database, requiring zero external services, zero cloud dependencies, and zero configuration.

**Why it matters:** As enterprises deploy AI agents across their operations, those agents need a way to remember context, share knowledge, and reason over relationships between concepts. Today, most agent memory solutions are cloud-hosted (vendor lock-in), unstructured (key-value only), or require complex infrastructure (graph databases, vector stores, message queues). Smriti eliminates all three problems.

**One binary. One file. Full stack.**

---

## Platform Architecture

```
                    +---------------------------+
                    |      Client Interfaces     |
                    +---------------------------+
                    |  MCP (stdio + HTTP)        |
                    |  REST API                  |
                    |  React Web Dashboard       |
                    |  Interactive CLI            |
                    +---------------------------+
                              |
                    +---------------------------+
                    |      Intelligence Layer    |
                    +---------------------------+
                    |  Knowledge Graph (petgraph) |
                    |  Smart Link Discovery       |
                    |  Daily Digest / Insights    |
                    |  Hybrid Search (FTS + KNN)  |
                    +---------------------------+
                              |
                    +---------------------------+
                    |      Storage Engine        |
                    +---------------------------+
                    |  SQLite (WAL mode)         |
                    |  FTS5 (full-text search)   |
                    |  sqlite-vec (embeddings)   |
                    |  Bi-temporal link model     |
                    +---------------------------+
```

---

## Core Capabilities

### 1. Knowledge Graph with Typed Relationships

Smriti builds and maintains a directed knowledge graph where every note is a node and every relationship is a typed, bi-temporal edge.

**Relationship Types:**

| Category | Types | Use Case |
|----------|-------|----------|
| **Core** | WikiLink, Backlink, Tag | Standard note linking via `[[wiki-links]]` and `#tags` |
| **Semantic Layers** | Semantic, Causal, Temporal | MAGMA-inspired typed layers for structured reasoning |
| **Domain: Healthcare** | Treats, Contraindicts, Interacts, Indicates, Causes | Clinical knowledge graphs, drug interaction mapping |
| **AI-Generated** | AiSuggested | Automatically discovered relationships |
| **Custom** | Any string | Extensible to any domain vocabulary |

**Bi-Temporal Edges (Zep/Graphiti model, arXiv:2501.13956):**
Every link carries `valid_from` and `valid_until` timestamps, enabling:
- Point-in-time graph queries ("What did we know in Q1?")
- Relationship lifecycle tracking
- Temporal reasoning for agents

**Graph Operations:**
- Full graph export (nodes + edges + statistics)
- Subgraph extraction around any node (configurable depth)
- Shortest path computation between any two nodes
- Layer-filtered traversal (e.g., "only follow causal edges")
- Orphan detection (unlinked notes)
- In-memory cache with lazy invalidation for sub-microsecond reads

**Performance:**
- Graph build (1,000 nodes): 216 us
- BFS traversal (depth 2): 235 ns
- BFS traversal (depth 3): 410 ns

---

### 2. Agent Memory Layer with Belief Revision

A namespaced key-value memory system designed for multi-agent architectures, with formal conflict resolution based on AGM belief revision theory (arXiv:2603.17244).

**Core Features:**
- **Namespaced storage:** Isolate memory by domain (`default`, `chat`, `research`, etc.)
- **TTL support:** Auto-expiring memory entries (checked at retrieval time)
- **Multi-agent:** Each agent gets its own isolated memory space

**Conflict Resolution Policies:**

| Policy | Behavior | Use Case |
|--------|----------|----------|
| `overwrite` | Last write wins (default) | Ephemeral state, session data |
| `reject` | Error if key exists | Immutable facts, audit logs |
| `version_and_keep` | Archive old value, store new | Decision tracking, belief evolution |
| `invalidate` | Mark old as superseded, store new | Knowledge correction, retractions |

**Memory History:** When using `version_and_keep` or `invalidate` policies, all previous values are archived in a history table with timestamps, enabling:
- Full audit trail of belief changes
- "What did the agent believe at time T?"
- Debugging agent decision-making

**Performance:**
- Store (100 keys): 513 us
- Retrieve (cache hit): 2.48 us
- Retrieve (cache miss): 2.25 us

---

### 3. Triple Search Engine (FTS5 + Semantic + Hybrid)

Three search modes, all running inside a single SQLite database:

**Full-Text Search (FTS5):**
- Porter stemming + unicode61 tokenization
- BM25-based ranking
- Sub-millisecond queries at 1,000+ notes
- Handles CJK, accented characters, and all Unicode

**Semantic Search (sqlite-vec):**
- Cosine distance KNN via sqlite-vec virtual table
- Accepts pre-computed embedding vectors (any dimensionality)
- Embedding generation via external provider (Ollama for local, any API for cloud)
- Zero additional processes required

**Hybrid Search (Reciprocal Rank Fusion):**
- Combines FTS5 keyword results with semantic vector results
- Configurable weighting (`fts_weight` parameter, default 0.5)
- RRF with k=60 for robust fusion
- Returns match source attribution (`fts`, `semantic`, or `both`)

**Performance:**
- FTS5 search (1K notes): 331 us
- FTS5 search (10K notes): 2.86 ms

**Research basis:** Graph-Based Memory Survey (arXiv:2602.05665) demonstrates that graph+BM25 hybrid beats pure vector for multi-hop tasks.

---

### 4. MCP Server (Model Context Protocol)

Native MCP integration makes Smriti a first-class memory provider for any MCP-compatible AI client (Claude Desktop, Claude Code, Cursor, etc.).

**Transports:**
- **stdio** (standard MCP): `smriti mcp --db /path/to/db`
- **HTTP JSON-RPC**: `POST /mcp` on the web server

**10 MCP Tools:**

| Tool | Description |
|------|-------------|
| `notes_create` | Create note with auto-detected `[[wiki-links]]` and `#tags` |
| `notes_read` | Read note by ID or title (fuzzy resolution) |
| `notes_search` | Full-text keyword search |
| `notes_list` | List/filter notes by tag |
| `notes_graph` | Full graph, subgraph, or shortest path queries |
| `notes_search_semantic` | Semantic or hybrid search with embedding vectors |
| `memory_store` | KV store with namespace, TTL, and conflict policy |
| `memory_retrieve` | Get memory entry by agent/namespace/key |
| `memory_list` | List all memory for an agent |
| `memory_history` | Retrieve superseded values (audit trail) |

**MCP Resources:** Notes exposed as `note://{id}` resources with markdown MIME type, enabling AI clients to browse and read notes directly.

**Protocol Version:** 2025-03-26 (latest MCP specification)

---

### 5. Web Dashboard (Embedded React SPA)

A full-featured web UI compiled into the binary. No separate frontend deployment needed.

**Pages:**

| Page | Features |
|------|----------|
| **Dashboard** | Today bar with current focus, quick note capture, note grid with load-more pagination, live statistics |
| **Search** | Unified search across FTS/semantic/hybrid modes, debounced autocomplete (200ms), keyboard navigation |
| **Note Detail** | Markdown rendering, inline CodeMirror editor, linked notes panel, tag management |
| **KV Store** | Table view of agent memory entries, inline editing, prefix filtering, TTL display |
| **Graph Explorer** | D3 force-directed graph, SVG for small graphs / Canvas for 150+ nodes, zoom/drag/pin, tag-based coloring, node inspector |

**Tech Stack:** React 18 + TypeScript + TanStack React Query + D3.js + CodeMirror

**Security:** Localhost-only CORS (rejects non-localhost origins with 403)

---

### 6. Interactive CLI

A full terminal interface for power users and scripting.

**13 Commands:**

| Command | Description |
|---------|-------------|
| `smriti new` | Interactive guided note creation (title, content, tags, linking) |
| `smriti create <title>` | Create note with flags (`-c content`, `--file`, `-t tags`) |
| `smriti link <source> <target>` | Create typed link between notes (`--type causal`) |
| `smriti read <id>` | Read note (text or `--json`) |
| `smriti list` | List notes with filters (`--tag`, `--limit`) |
| `smriti search <query>` | Full-text search |
| `smriti graph` | Export graph (`--format json/dot/text`, `--center`, `--depth`) |
| `smriti stats` | Database statistics + smart link suggestions |
| `smriti serve` | Start web server (`--host`, `--port`) |
| `smriti mcp` | Start MCP server (stdio) |
| `smriti import <dir>` | Bulk import markdown files (recursive) |
| `smriti export <dir>` | Bulk export with optional YAML frontmatter |
| `smriti sync` | WebDAV or filesystem sync |
| `smriti completions` | Generate shell completions (bash/zsh/fish/powershell/elvish) |

---

### 7. Smart Features

**Smart Link Suggestions:**
- Detects when note content mentions another note's title without a `[[wiki-link]]`
- Computes keyword overlap (Jaccard similarity) between note pairs
- Returns ranked suggestions with confidence scores (0.0-1.0)
- Auto-link capability for high-confidence matches (>= 0.7)

**Daily Digest:**
- Notes created/modified in the last 24 hours
- New links created today
- Word count for the day
- Top 5 link suggestions
- Trending topics (rising/stable/declining tags over 7-day window)
- Orphan notes needing attention
- "On this day" historical notes

---

### 8. Sync Engine

Cross-device synchronization with two backends:

**WebDAV Sync:**
- Compatible with Synology NAS, Nextcloud, any WebDAV server
- Content-hash based change detection (SHA256)
- Per-note `.md` files + JSON manifest
- Push/pull/bidirectional modes
- Auth via environment variables (`SYNC_USER`, `SYNC_PASS`)

**Filesystem Sync:**
- Mount NAS shares or use Synology Drive synced folders
- Exports as markdown with YAML frontmatter (id, tags, timestamps, device metadata)
- Conflict resolution: last-modified-wins

---

### 9. Tool Logging & Observability

Built-in tool execution logging for agent debugging and compliance:

- Log every tool call with input, output, status, and duration
- Status tracking: `Success`, `Error`, `Timeout`
- Per-agent log isolation
- Query logs by agent with pagination
- Full tracing via `tower-http` and `tracing-subscriber` (JSON or pretty output)

---

### 10. Wiki-Link & Markdown Intelligence

**Wiki-Link Syntax:**
- `[[Note Title]]` — standard link
- `[[Note Title|Display Text]]` — aliased link
- `[[Note Title#Section]]` — section-specific link
- `[[rel:causal|Note Title]]` — typed relationship link
- `[[rel:semantic|Note Title|Display]]` — typed + aliased

**Tag Extraction:**
- `#tag` syntax in note content, auto-detected on create/update
- YAML frontmatter tag parsing
- Nested tags supported (`#category/subcategory`)

**Frontmatter Support:**
- Tags, aliases, date, status, and arbitrary extra fields
- Automatic parsing on import, optional generation on export

---

## Deployment Options

| Method | Command | Notes |
|--------|---------|-------|
| **Cargo install** | `cargo install smriti` | Single binary, instant |
| **Docker** | `docker compose up` | Persistent volume, healthcheck |
| **From source** | `cargo build --release` | LTO-optimized, stripped binary |
| **MCP client config** | Add to `claude_desktop_config.json` | Zero-config agent integration |

**Runtime requirements:** None. The binary includes SQLite (bundled), the web UI (embedded), and all dependencies. No Python, no Node, no external databases.

---

## Performance Benchmarks

Measured on Apple Silicon, in-memory SQLite (Criterion):

| Operation | p50 Latency |
|-----------|-------------|
| Insert 1 note | 32.5 us |
| Insert 100 notes | 2.0 ms |
| Insert 1,000 notes | 23.1 ms |
| FTS5 search (1K notes) | 331 us |
| FTS5 search (10K notes) | 2.86 ms |
| Graph build (1K nodes) | 216 us |
| BFS depth-2 | 235 ns |
| BFS depth-3 | 410 ns |
| Memory store (100 keys) | 513 us |
| Memory retrieve (hit) | 2.48 us |
| Memory retrieve (miss) | 2.25 us |

**Comparison with alternatives:**

| Platform | KV Retrieve Latency | Deployment |
|----------|-------------------|------------|
| **Smriti** | **2.5 us** | Single binary + SQLite |
| Mem0 | 50-200 ms | Cloud API |
| Letta | 10-50 ms | Python server + Postgres |
| Zep | 5-20 ms | Go server + Postgres |

---

## Research Foundations

Smriti is built on peer-reviewed research in agent memory and knowledge representation:

| Paper | arXiv | Applied In Smriti |
|-------|-------|-------------------|
| Zep / Graphiti | 2501.13956 | Bi-temporal edges (`valid_from`/`valid_until`) — 18.5% improvement on LongMemEval |
| MAGMA | 2601.03236 | Typed graph layers (semantic/temporal/causal) — 95% token reduction |
| Graph-Native Belief Revision | 2603.17244 | AGM conflict policies on `memory_store` |
| Graph-Based Memory Survey | 2602.05665 | Hybrid FTS+vector search beats pure vector for multi-hop tasks |

---

## Live System Verification (April 2026)

Tested against running Smriti v0.2.0 instance:

| Test | Result |
|------|--------|
| 71 notes, 156 edges in knowledge graph | Verified |
| FTS5 search with CJK and Unicode content | Passed |
| MCP HTTP initialize (protocol 2025-03-26) | Passed |
| 10 MCP tools registered and callable | Verified |
| Note preview truncation with multibyte chars | Passed (no panic) |
| Graph export with typed edges and bi-temporal fields | Verified |
| Web dashboard serving embedded React SPA | Verified |
| Localhost-only CORS enforcement | Verified |

---

## Domain-Specific Applications

### Healthcare / Life Sciences
- Drug interaction graphs with `Treats`, `Contraindicts`, `Interacts` edge types
- Patient knowledge timelines with bi-temporal validity
- Clinical decision support with causal reasoning chains
- HIPAA-friendly: fully self-hosted, no data leaves the machine

### Enterprise Knowledge Management
- Board decision tracking with `version_and_keep` audit trail
- Cross-team knowledge graphs linking strategy, engineering, and sales
- Competitive intelligence with temporal market snapshots
- Smart link suggestions surfacing hidden connections

### AI Agent Infrastructure
- Persistent memory across agent sessions
- Multi-agent knowledge sharing via namespaced memory
- Tool execution logging for compliance and debugging
- Graph-native context retrieval reducing token usage by up to 95%

### Research & Academia
- Literature knowledge graphs with citation relationships
- Lab notebook with temporal experiment tracking
- Grant management with linked deliverables
- PI-specific agent workflows (multi-subagent research orchestration)

---

## Security & Compliance

- **Data sovereignty:** All data stays on your machine. No telemetry, no cloud calls, no external dependencies.
- **Localhost-only web UI:** CORS policy rejects all non-localhost origins.
- **Single-file database:** Easy to backup, encrypt, and audit. SQLite WAL mode for safe concurrent access.
- **No credentials stored:** Sync auth via environment variables only.
- **MIT licensed:** Full source available, no vendor lock-in.

---

## Future Roadmap (Informed by SOTA Agentic Infrastructure)

### Near-Term (Next Quarter)

**1. Agentic RAG with Graph-Guided Retrieval**
Instead of naive vector similarity, use the knowledge graph to guide retrieval:
- Walk graph edges to find contextually related notes before falling back to vector search
- Prioritize notes with causal/temporal links to the query context
- Research basis: GraphRAG (Microsoft, 2024) shows 30-70% improvement over naive RAG on multi-hop queries

**2. Embedding Generation via Local Models**
- Integrate Ollama API for on-device embedding generation
- Support multiple embedding models (nomic-embed-text, all-MiniLM, etc.)
- Auto-embed on note create/update
- Zero-config: detect local Ollama instance, fallback to manual embedding

**3. MCP Prompts & Sampling**
- Expose pre-built prompts via MCP (e.g., "summarize recent changes", "find contradictions")
- Enable MCP sampling for agent-initiated queries
- Template library for common knowledge operations

### Mid-Term (Next Two Quarters)

**4. Multi-Agent Collaboration Protocol**
- Shared knowledge namespaces across agents with RBAC
- Agent-to-agent message passing via the knowledge graph
- Conflict resolution when multiple agents update the same knowledge
- Research basis: CAMEL (2023), AutoGen (Microsoft), CrewAI collaboration patterns

**5. Temporal Reasoning Engine**
- Query: "What changed between Q1 and Q2?"
- Diff two temporal snapshots of the knowledge graph
- Timeline visualization in the web dashboard
- Temporal logic queries (Allen's interval algebra) over bi-temporal edges

**6. Incremental Graph Embeddings**
- Graph neural network embeddings (node2vec / GraphSAGE) computed incrementally
- Enable graph-aware semantic search (notes similar in graph topology, not just text)
- Research basis: Dynamic GNN literature (2024-2025) for online graph updates

**7. Structured Output / Tool-Use Memory**
- Store and index tool call patterns (which tools succeed for which queries)
- Enable agents to learn from past tool executions
- Auto-suggest tool chains based on historical success patterns
- Research basis: Toolformer (Meta), ToolLLM (2024)

### Long-Term (6-12 Months)

**8. Federated Knowledge Graphs**
- Sync knowledge graphs across multiple Smriti instances
- Selective sharing (share specific subgraphs, not full database)
- CRDT-based conflict resolution for distributed graph updates
- Enable team-scale knowledge without centralized servers

**9. Causal Inference Engine**
- Formal causal reasoning over causal-typed edges
- Intervention queries ("If we change X, what happens to Y?")
- Counterfactual reasoning for decision support
- Research basis: DoWhy (Microsoft), CausalNex, Pearl's do-calculus

**10. Autonomous Knowledge Curation Agent**
- Background agent that continuously improves the knowledge graph
- Detects stale information, suggests updates
- Identifies contradictions between notes
- Proposes new causal/temporal links based on content analysis
- Research basis: Reflexion (2023), self-improving agent loops

**11. Plugin / Extension Architecture**
- Domain-specific plugins (healthcare, legal, finance)
- Custom link type registries with validation rules
- Webhook integrations for external event sources
- WASM-based plugin sandbox for safe extensibility

**12. Compliance & Audit Module**
- Complete audit trail of all knowledge changes
- Role-based access control for multi-user deployments
- Data retention policies with automatic expiration
- Export to compliance formats (SOC 2, HIPAA audit logs)

---

## Competitive Positioning

| Capability | Smriti | Mem0 | Zep | Letta | LangGraph |
|-----------|--------|------|-----|-------|-----------|
| Self-hosted | Single binary | Cloud-first | Server + Postgres | Server + Postgres | Python lib |
| Knowledge graph | Native (petgraph) | None | Graphiti (Neo4j) | None | State machine |
| Typed relationships | 12+ types + custom | None | 2 types | None | Edges only |
| Bi-temporal edges | Native | None | Partial | None | None |
| Conflict resolution | 4 AGM policies | Overwrite only | Overwrite only | Overwrite only | None |
| Full-text search | FTS5 (sub-ms) | Cloud API | Postgres FTS | Postgres FTS | None |
| Semantic search | sqlite-vec | Cloud API | pgvector | pgvector | None |
| Hybrid search (RRF) | Native | None | None | None | None |
| MCP native | stdio + HTTP | None | None | None | None |
| Web dashboard | Embedded React | Cloud UI | None | Cloud UI | None |
| Offline capable | Full | No | No | No | Yes |
| External dependencies | Zero | Cloud API | Neo4j + Postgres | Postgres | None |
| Agent memory | Native with TTL | Native | Native | Native | Checkpoints |
| Tool logging | Native | None | None | Partial | None |
| Latency (KV retrieve) | 2.5 us | 50-200 ms | 5-20 ms | 10-50 ms | N/A |

---

## Key Differentiators for Decision Makers

1. **Zero infrastructure cost.** No databases to provision, no cloud services to manage, no DevOps overhead. One binary, one file, done.

2. **Data never leaves your network.** Full data sovereignty by default. Critical for regulated industries (healthcare, finance, government).

3. **Graph-native, not graph-bolted.** The knowledge graph is the primary data structure, not an afterthought. Every query can leverage relationship context.

4. **Research-backed architecture.** Built on published, peer-reviewed research (Graphiti, MAGMA, AGM belief revision), not ad-hoc design.

5. **MCP-native integration.** Works out of the box with Claude, Cursor, and any MCP-compatible AI tool. No custom integration code needed.

6. **1000x faster than cloud alternatives.** Microsecond-level memory retrieval vs. cloud round-trips. Agents think faster when memory is local.

7. **Typed relationships enable structured reasoning.** Agents can follow causal chains, temporal sequences, or semantic clusters instead of flat keyword matching.

8. **Audit trail built in.** Belief revision with memory history means you always know what an agent knew and when it changed its mind.

---

*Document generated April 2026. Based on Smriti v0.2.0, 71 notes, 156 edges, verified against running instance.*

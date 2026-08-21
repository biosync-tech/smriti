<p align="center">
  <a href="https://github.com/biosync-tech/smriti/actions"><img src="https://img.shields.io/github/actions/workflow/status/biosync-tech/smriti/ci.yml?branch=main&label=build" alt="Build"></a>
  <a href="https://crates.io/crates/smriti"><img src="https://img.shields.io/crates/v/smriti.svg" alt="crates.io"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT"></a>
</p>

# Smriti

**A self-hosted knowledge graph and AI agent memory layer — one binary, one SQLite file, zero cloud dependencies.**

**Git for LLM wikis.** Atomic multi-write transactions, enforced provenance on every claim, append-only event log with a hash chain, and an integrity verifier — so agent-authored knowledge is auditable by construction.

`2.5µs` KV retrieval · `235ns` graph traversal · `0` cloud dependencies

```bash
cargo install smriti
```

## Why Smriti (vs. Obsidian / Zep / Mem0 / Letta / Neo4j)

| Property                                      | Obsidian | Zep | Mem0 / Letta | Neo4j / Graphiti | **Smriti** |
|-----------------------------------------------|:--------:|:---:|:------------:|:----------------:|:----------:|
| Single binary, single file, no server         | ✓*       | ✗   | ✗            | ✗                | ✓          |
| Fully local / offline                         | ✓        | ✗   | ✗            | partial          | ✓          |
| Bi-temporal edges (valid_from / valid_until)  | ✗        | ✓   | ✗            | ✓                | ✓          |
| **Atomic multi-write transactions (SAVEPOINT)** | ✗      | ✗   | ✗            | ✓ (server)       | **✓**      |
| **Enforced provenance on every claim**        | ✗        | ✗   | ✗            | ✗                | **✓**      |
| **Append-only event log + hash chain**        | ✗        | ✓   | ✗            | ✗                | **✓**      |
| **`smriti verify` integrity sweep**           | ✗        | ✗   | ✗            | ✗                | **✓**      |
| Contradiction inbox (never auto-resolves)     | ✗        | partial | ✗        | ✗                | ✓          |
| MCP-native for agents                         | plugin   | ✗   | ✗            | ✗                | ✓          |

\* Obsidian is a filesystem with no transactional guarantees. Smriti's moat isn't novelty — it's **write-time discipline in a local-first Rust runtime**.

## Research foundation

Every integrity feature cites an arXiv paper so you can trace the design back to the literature:

- **Bi-temporal edges & event log T / T′** — Zep / Graphiti, arXiv:2501.13956
- **Structural overlap verification (claim ↔ source)** — FACTUM, arXiv:2601.05866 and Citation-Grounded Code Comprehension, arXiv:2512.12117
- **Contradiction confidence scoring** — MemoTime, arXiv:2510.13614 and EvoReasoner / EvoKG, arXiv:2509.15464
- **Belief revision & conflict policy on memory_store** — AGM postulates, arXiv:2603.17244
- **Graph + BM25 hybrid retrieval** — Graph-Based Memory Survey, arXiv:2602.05665
- **Typed graph layers (semantic/temporal/causal)** — MAGMA, arXiv:2601.03236
- **Zettelkasten-style agent memory** — A-MEM, arXiv:2502.12110 (NeurIPS 2025)
- **Hallucination grounding requirements** — arXiv:2510.24476

## Integrity layer (v0.2)

Four MCP tools turn Smriti from a CRUD store into a wiki with invariants:

- `wiki_transaction_submit` — batch of create/update/link/source ops applied atomically inside a SQLite `SAVEPOINT`. Every content write must carry a `claim_spans` array or be rejected (provenance enforced by default).
- `wiki_verify` — runs referential integrity + re-verifies every stored claim's overlap score + walks the event-log hash chain. Returns pass/fail. Never mutates.
- `contradictions_detect` — pairwise scan over recent notes using *w1·semantic + w2·recency + w3·authority* weighted scoring. Candidates land in a review inbox — Smriti never auto-resolves.
- `contradictions_list` — the review inbox.

CLI mirrors all of it:

```bash
smriti verify                    # integrity sweep
smriti pending-tx                # list transactions awaiting review
smriti commit-tx <id>            # commit a pending transaction
smriti reject-tx <id> -r "..."   # reject with reason
smriti detect-contradictions     # scan for candidates
smriti contradictions            # show review inbox
```

---

## What is Smriti?

Smriti (Sanskrit: स्मृति, *memory*) is a single Rust binary that runs a knowledge graph, a full-text + semantic search index, and an MCP server on top of one SQLite file. It is designed for two users: a knowledge worker who wants a private second brain their AI assistant can read, and a developer who needs a persistent, structured memory layer for AI agents.

Notes connect to each other through typed wiki-links — write `[[rel:causal|Decision X]]` in a note and Smriti records a directed `causal` edge in the knowledge graph automatically. Agents can then traverse that graph to answer questions like "what led to this decision?" without re-reading every note.

---

## Quick start

```bash
# Install
cargo install smriti

# Create your first notes — links and tags are extracted automatically
smriti new                    # interactive guided prompt
smriti create "Acme Corp" --content "Key client. Met via [[Sarah Chen]]." --tags client

# Search
smriti search "Acme"

# Open the web dashboard
smriti serve
# → http://localhost:3000
```

Your notes, graph, and search index live in `~/.local/share/smriti/smriti.db`. Back up with `cp`.

---

## Use cases

### Client knowledge graph

Track every client, contact, and engagement as linked notes. When you brief Claude before a call, it reads the full context — history, decisions, open items — without you re-explaining anything.

```bash
smriti create "Acme Corp Q2 Review" \
  --content "Next steps: [[rel:temporal|Budget approval]] by June. Owner: [[Sarah Chen]]." \
  --tags client decision
```

### Decision log

Record decisions with context and consequences. The `rel:causal` link type lets agents trace why something was decided.

```bash
smriti create "Switched to Rust" \
  --content "Replaced Python service. Reason: [[rel:causal|Memory leak in prod]]." \
  --tags decision
```

### Daily AI context

Store your current focus in the KV store. Claude reads it at the start of every session through MCP.

```bash
smriti serve   # then ask Claude: "what's my current focus?" — Smriti answers via MCP
```

### SOPs and playbooks

Document repeatable processes as linked notes. Import existing markdown files in one command.

```bash
smriti import ./playbooks --recursive
```

---

## MCP integration

Smriti runs as an MCP server over stdio. Add it to Claude Desktop in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "smriti": {
      "command": "smriti",
      "args": ["mcp", "--db", "/path/to/smriti.db"]
    }
  }
}
```

For claude.ai remote MCP, start `smriti serve` and point the MCP client at `http://localhost:3000/mcp`.

### MCP tools

| Tool | What it does |
|------|-------------|
| `notes_create` | Create a note; `[[wiki-links]]` and `#tags` are auto-extracted |
| `notes_read` | Read a note by ID or title |
| `notes_search` | Full-text BM25 search across all notes |
| `notes_list` | List recent notes, filter by tag |
| `notes_graph` | Return a subgraph (BFS, typed edge filter) around a note |
| `notes_search_semantic` | Vector + FTS5 hybrid search with reciprocal rank fusion |
| `memory_store` | Store a key-value pair; supports TTL and conflict policy |
| `memory_retrieve` | Retrieve a stored value by agent ID + key |
| `memory_list` | List all memory entries for an agent |
| `memory_history` | Retrieve superseded values for a key (versioned memory) |

Full MCP reference: [`docs/mcp.md`](docs/mcp.md)

---

## Performance

Measured on Apple Silicon, in-memory SQLite, using [Criterion](https://github.com/bheisler/criterion.rs). Run: `cargo bench`

| Operation | p50 |
|-----------|-----|
| Insert 1 note | 32.5 µs |
| Insert 100 notes | 2.0 ms |
| Insert 1,000 notes | 23.1 ms |
| FTS5 search — 1k notes | 331 µs |
| FTS5 search — 10k notes | 2.86 ms |
| Graph build — 1k nodes | 216 µs |
| BFS depth-2 (cached) | 235 ns |
| BFS depth-3 (cached) | 410 ns |
| Memory KV store — 100 keys | 513 µs |
| Memory KV retrieve (hit) | 2.48 µs |
| Memory KV retrieve (miss) | 2.25 µs |

### Smriti vs alternatives

| | Smriti | Mem0 | Letta | Zep |
|---|---|---|---|---|
| Self-hosted | Yes | No | Yes | Partial |
| Knowledge graph | Yes (petgraph) | No | No | Yes (Neo4j) |
| Typed edges | Yes | No | No | Yes |
| Bi-temporal edges | Yes | No | No | Yes |
| Belief revision | Yes (AGM) | No | No | No |
| MCP native | Yes | No | No | No |
| Full-text search | FTS5 (BM25) | Vector only | Vector only | Vector + keyword |
| Hybrid search | Yes (RRF) | No | No | No |
| KV memory + TTL | Yes | No | Yes | Yes |
| Language | Rust | Python | Python | Python/Go |
| Deployment | Single binary | SaaS | Docker + Postgres | Docker + Neo4j + Redis |
| KV retrieval latency | ~2.5 µs | ~50–200 ms | ~10–50 ms | ~5–20 ms |

---

## Architecture

```
src/
├── models/     Note, Link, AgentMemory, ToolLog — Serde on every type
├── storage/    SQLite + FTS5 + sqlite-vec; WAL mode; single connection pool
├── parser/     [[wiki-link]] and #tag extraction via regex; no runtime deps
├── graph/      petgraph DiGraph; lazy GraphCache (Arc<RwLock>); typed BFS
├── mcp/        JSON-RPC 2.0 over stdio; dispatches to same handlers as REST
├── web/        Axum router; localhost-only CORS; embedded React SPA
├── cli/        clap v4 derive; 11 commands; shell completions; interactive new
├── sync/       WebDAV + filesystem sync with per-device conflict tracking
└── features/   Smart link suggestions; daily digest
```

### Design decisions

**Why SQLite, not Postgres.** A knowledge base for one person or a small team should not require a running database server. SQLite in WAL mode handles hundreds of concurrent reads per second — more than enough for any personal knowledge graph. The entire database is one file: backup is `cp`, migration is `mv`.

**Why Rust, not Python.** Agent memory sits in the critical path of every tool call. Python MCP servers typically add 50–200 ms per round-trip from serialization overhead and GIL contention. Smriti's Rust implementation retrieves a KV entry in 2.5 µs, keeping memory operations invisible to the agent's response latency.

**Why FTS5 + vector, not one or the other.** Keyword search (BM25) is precise for known terms; vector search recalls semantically related content the user didn't think to search for. Neither is sufficient alone. Smriti combines both with reciprocal rank fusion, weighted at query time — matching the finding in [arXiv:2602.05665](https://arxiv.org/abs/2602.05665) that hybrid retrieval outperforms pure vector on multi-hop reasoning tasks.

**Belief revision on `memory_store`.** When an agent stores a key that already exists, naive overwrite discards history. Smriti implements four AGM conflict resolution policies ([arXiv:2603.17244](https://arxiv.org/abs/2603.17244)): `overwrite` (default), `reject` (fail if exists), `version_and_keep` (archive old value), and `invalidate` (mark old as superseded). Superseded values are queryable via `memory_history`.

### Research basis

| Paper | arXiv | What it grounds in Smriti |
|-------|-------|--------------------------|
| Zep / Graphiti | [2501.13956](https://arxiv.org/abs/2501.13956) | Bi-temporal edges on `links` table; 18.5% LongMemEval improvement |
| MAGMA | [2601.03236](https://arxiv.org/abs/2601.03236) | Typed graph layers; BFS filtered by `link_type`; 95% token reduction |
| Graph-Native Belief Revision | [2603.17244](https://arxiv.org/abs/2603.17244) | `ConflictPolicy` enum on `memory_store` |
| Graph-Based Memory Survey | [2602.05665](https://arxiv.org/abs/2602.05665) | FTS5 + sqlite-vec hybrid with reciprocal rank fusion |

---

## Contributing

```bash
git clone https://github.com/biosync-tech/smriti.git
cd smriti
cargo test --all-features   # should be green
cargo bench                 # performance baseline
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the development workflow. Issues labelled [`good first issue`](https://github.com/biosync-tech/smriti/labels/good%20first%20issue) are self-contained storage or CLI changes that don't require understanding the full codebase.

Before opening a PR: `cargo clippy --all-features -- -D warnings` and `cargo fmt --check`.

---

## License

[MIT](LICENSE)

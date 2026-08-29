<p align="center">
  <a href="https://github.com/biosync-tech/smriti/actions"><img src="https://img.shields.io/github/actions/workflow/status/biosync-tech/smriti/ci.yml?branch=main&label=build" alt="Build"></a>
  <a href="https://crates.io/crates/smriti"><img src="https://img.shields.io/crates/v/smriti.svg" alt="crates.io"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT"></a>
</p>

# Smriti

**A self-hosted knowledge graph and AI agent memory layer — one binary, one SQLite file, zero cloud dependencies.**

**Git for LLM wikis.** Atomic multi-write transactions, enforced provenance on every claim, append-only event log with a hash chain, and an integrity verifier — so agent-authored knowledge is auditable by construction.

`2.5µs` KV retrieval · `235ns` graph traversal · `0` cloud dependencies

## Quick start (< 5 minutes)

```bash
# 1. Install
cargo install smriti

# 2. Initialize a new database
smriti init

# 3. Create your first note (wiki-links and tags auto-extracted)
smriti create "Protocol Amendment Log" -c "Amendment 03 submitted for [[Inclusion Criterion Change]]." -t protocol

# 4. Search
smriti search "roadmap"

# 5. Connect to Claude Desktop — add this to claude_desktop_config.json:
# {
#   "mcpServers": {
#     "smriti": {
#       "command": "smriti",
#       "args": ["mcp", "--db", "/absolute/path/to/notes.db"]
#     }
#   }
# }
```

Your notes live in `notes.db` (current directory). Back up with `cp`. No server, no cloud.

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

## Use cases

### Grounded research memory

Every claim in your draft cites a source. When your advisor asks "where did you get this?" at 11pm, the answer is one command away.

```bash
# Ingest sources
smriti ingest ~/papers/foster-wilson-2006.pdf --uri "doi:10.1038/nature04587"

# Write a grounded note (via MCP or CLI)
smriti wiki-tx submit --require-provenance \
  --op create_note \
  --title "Hippocampal replay mechanisms" \
  --content "Replay events are biased toward rewarded trajectories." \
  --claim "biased toward rewarded trajectories" \
    --source doi:10.1038/nature04587 \
    --span "...replay events were biased toward trajectories...associated with reward..."

# If the claim doesn't overlap the source, the transaction rolls back.
# Verify your entire vault any time:
smriti verify   # → notes=412  sources=89  claim_spans=1,204  events=3,712  OK
```

### Clinical trial amendment ledger

Which protocol version was active when Subject 14 was screened? Bi-temporal edges answer in milliseconds.

```bash
# Link protocol versions with valid-from dates
smriti link add \
  --from "Trial-A-Protocol-v2.1" \
  --to "Trial-A-Protocol-v2.3" \
  --type amended_by \
  --valid-from 2026-03-14

# Query the active version on any date
smriti graph --as-of 2026-03-10 trial-A
# → Returns v2.1, not v2.3 (which took effect 4 days later)

# Before every monitor visit:
smriti verify --trial Trial-A --since 2026-03-01
# → 47 claims rechecked, 1 FAIL flagged before the monitor sits down
```

### Senescence biomarker consolidation

Three IPF cohorts cite overlapping markers. Which are replicated science vs. spurious? Consolidation promotes the durable pattern into a schema, lineage intact.

```bash
# Score notes by replay frequency + structural centrality + context diversity
smriti consolidate --dry-run --policy conservative
# → 3 episodes cluster (p16INK4a, MMP-7 replicated; IL-6 disputed)

# Review flagged clusters
smriti proposals
# → schema_candidate_001: 3 episodes, similarity 0.86, IL-6 flagged

# Human approves → schema formed with full lineage
smriti approve schema_candidate_001
# → Created schema_ipf_panel_v1
#    Source episodes preserved (not deleted)
#    Audit event written: promoted_to_schema
```

### IND dose synthesis (CB-209)

100 mg is safe but sub-efficacious. 300 mg triggers DLT. The recommendation must reconstruct in front of an FDA reviewer.

```bash
# Multi-hop traversal returns the evidence chain
smriti graph traverse CB-209 --depth 2
# → [compound] CB-209
#     ├─ [dosage 100mg] safe; 18% CRP < 30% endpoint
#     └─ [dosage 300mg] efficacious; 29% Tn-T > 0.10 (DLT)

# Replay the model call that recommended 200 mg MTD
smriti audit replay call_7d3a
# → Output: 200 mg once daily (MTD for Phase III)
#    Verified spans: 2 of 2 → trial protocol + Troponin-T ref
#    Stored hash: 2e8b…7c4b
#    Re-run hash:  2e8b…7c4b  ✓ bit-identical
```

---

## MCP integration

Smriti ships as an MCP server (JSON-RPC 2.0 over stdio). Claude Desktop configuration is printed by `smriti init`. For remote MCP (claude.ai), start `smriti serve` and point the MCP client at `http://localhost:3000/mcp`.

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

# Why Local-First?

Smriti is designed as a **local-first knowledge graph** — one binary, one SQLite file, zero cloud dependencies. This document explains why.

## The Problem with Cloud-First Agent Memory

Most agent memory systems (Mem0, Letta, Zep) require:
1. **Always-on internet** — agents can't function offline
2. **External database servers** — Neo4j, PostgreSQL, Redis, Qdrant
3. **SaaS APIs** — embedding generation, vector search, LLM calls
4. **Unbounded growth** — no consolidation, memory grows forever

This creates five failure modes:

### 1. **Deployment Complexity**

To run Zep locally, you need:
- Docker + Docker Compose
- PostgreSQL (TimescaleDB)
- Neo4j (for graph operations)
- Redis (for caching)
- Python runtime + dependencies
- API keys for embedding services

Smriti: `cargo install smriti && smriti init`. One binary. No servers.

### 2. **Latency**

Cloud-first systems add 50–200ms per memory retrieval from:
- Network round-trips
- Python GIL contention
- JSON serialization overhead
- External database queries

Smriti retrieves KV entries in **2.5 µs** (in-process, SQLite). Graph traversal: **235 ns** (petgraph, cached).

### 3. **Privacy & Compliance**

Sending clinical trial data, patient notes, or proprietary research to a cloud embedding API violates:
- HIPAA (protected health information)
- ICH E6(R3) (clinical data sovereignty)
- GDPR (personally identifiable data)
- Corporate IP policies

Smriti: all data stays on your machine. No cloud, no API keys, no data exfiltration.

### 4. **Cost**

Cloud embedding APIs charge per token:
- OpenAI `text-embedding-3-small`: $0.02 / 1M tokens
- For 100k notes (avg 500 tokens each): **$1,000** just to embed once
- Re-embedding after updates: more cost
- Vector database hosting: $100–$500/month

Smriti: zero recurring cost. Embeddings are optional, generated locally via Ollama if needed.

### 5. **Unbounded Memory Growth**

Mem0, Letta, and Zep have no consolidation mechanism. Every note persists equally. A year of daily notes = 365 entries, forever. Retrieval degrades as the graph grows.

Smriti: **WikiSkill-inspired consolidation** (Task 9). Frequently-accessed, structurally-central episodes are promoted to schemas. Rarely-accessed notes are flagged for review. The graph gets **cleaner over time**, not just bigger.

---

## Local-First Principles

Smriti follows the [local-first software principles](https://www.inkandswitch.com/local-first/):

### 1. **No Spinners**

Your data is on disk. Reads are synchronous, microsecond-latency. No waiting for cloud APIs.

### 2. **Works Offline**

Agent memory available on a plane, in a hospital with restricted internet, or at a remote clinical trial site.

### 3. **Owns the Data**

The SQLite file is **yours**. Back up with `cp`. Migrate with `mv`. Inspect with `sqlite3`. No vendor lock-in, no API rate limits, no SaaS shutdown risk.

### 4. **Fast**

SQLite in WAL mode handles hundreds of concurrent reads per second. FTS5 search is faster than Elasticsearch for single-user workloads. petgraph BFS traversal is **sub-microsecond**.

### 5. **Durable**

SQLite is the [most deployed database engine in the world](https://www.sqlite.org/mostdeployed.html). It's in your phone, your browser, your car. Smriti's data format will outlive any SaaS API.

---

## Why SQLite, Not Neo4j?

Neo4j is excellent for multi-terabyte graphs with complex Cypher queries. But for personal knowledge graphs (100–100k notes):

| Property | SQLite + petgraph | Neo4j |
|----------|-------------------|-------|
| **Installation** | Bundled in Smriti binary | Separate Java process |
| **Startup time** | Instant | 10–30 seconds |
| **Memory footprint** | <100 MB | 1–4 GB |
| **Backup** | `cp smriti.db backup.db` | `neo4j-admin backup` |
| **Graph traversal (1k nodes)** | 235 ns (cached) | 5–50 ms (network + parsing) |
| **Full-text search** | FTS5 (BM25) | Requires Elasticsearch plugin |

For small-to-medium graphs, SQLite + petgraph is faster, simpler, and more portable.

---

## Why Rust, Not Python?

Python MCP servers are slow:
- GIL locks prevent true parallelism
- JSON serialization overhead (50–100 ms for large graphs)
- Startup time (500 ms–2 seconds for import resolution)

Rust MCP servers:
- True concurrency (tokio async runtime)
- Zero-copy deserialization (serde)
- Instant startup

**Benchmark:** Smriti's `memory_retrieve` is **20,000× faster** than a typical Python KV lookup (2.5 µs vs 50 ms).

---

## Hybrid Approach: Local Graph + Optional Cloud LLM

Smriti doesn't reject cloud services — it just doesn't **require** them.

### What Stays Local

- All notes, links, tags, agent memory
- Full-text search (FTS5)
- Graph traversal (petgraph)
- Consolidation scoring (cascade salience + degree + diversity)

### What's Optional (User-Configured)

- **Embeddings:** Generate with Ollama (local) or an external API (user's choice)
- **LLM schema formation:** Use Ollama (local) or OpenAI/Anthropic (user's choice)
- **Sync:** WebDAV to a NAS, S3, or filesystem

The user controls every cloud call. No hardcoded API keys, no surprise costs.

---

## Clinical Trial Use Case: Why Local-First Wins

**Scenario:** A clinical trial coordinator needs an AI assistant to track protocol amendments, adverse events, and regulatory submissions.

### With Cloud-First (Mem0 / Letta)

1. **Compliance risk:** Patient data sent to OpenAI/Anthropic embedding API → HIPAA violation
2. **Connectivity:** Trial site in rural hospital → no internet → agent can't function
3. **Cost:** 10k notes × 500 tokens × $0.02/1M = $100 just to embed, plus monthly SaaS fees
4. **Audit trail:** No guarantees that deleted notes stay deleted (cloud provider controls data)

### With Smriti (Local-First)

1. **Compliance:** All data stays on-premises → ICH E6(R3) compliant
2. **Connectivity:** Works offline → assistant functional in any site
3. **Cost:** Zero recurring fees
4. **Audit trail:** `events` table + hash chain + `wiki_verify` = reconstructable audit trail

---

## Team Collaboration: Local-First + Sync

Each team member runs their own Smriti instance. Sync via:

### Option 1: Shared Filesystem

```bash
smriti sync --remote /mnt/shared-drive --direction both
```

Conflict resolution: per-device tracking. Smriti never overwrites another device's changes.

### Option 2: WebDAV (Synology NAS, Nextcloud)

```bash
export SYNC_USER=admin
export SYNC_PASS=...
smriti sync --remote https://nas.local:5006/smriti --direction both
```

### Option 3: Git

The SQLite file is binary, but the `export` command creates markdown files:

```bash
smriti export ./notes --frontmatter
git add notes/
git commit -m "Daily sync"
git push
```

Another team member:

```bash
git pull
smriti import ./notes --recursive
```

---

## When to Use Cloud Services

Smriti's local-first design doesn't mean cloud is always wrong. Use cloud when:

1. **Team size > 50** — Centralized database (PostgreSQL/Neo4j) may scale better
2. **Global distribution** — Replicated cloud database reduces latency vs NAS sync
3. **Regulated SaaS requirement** — Some industries mandate cloud logging (e.g., fintech)

For those cases, Smriti's REST API can front a centralized PostgreSQL instance. But for **personal knowledge graphs** and **small-team agent memory**, local-first wins.

---

## Frequently Asked Questions

### Q: Isn't SQLite too slow for semantic search?

**A:** No. sqlite-vec (used by Smriti) benchmarks at 1–5ms for cosine similarity over 10k 384-dim vectors. That's faster than Qdrant or Pinecone for single-user workloads.

### Q: What if I want to use OpenAI embeddings?

**A:** Generate embeddings externally, then store them:

```bash
curl -X POST http://localhost:3000/api/v1/notes/NOTE_ID/embed \
  -H "Content-Type: application/json" \
  -d '{"embedding": [0.1, 0.2, ..., 0.384]}'
```

Smriti stores them, but never generates them without your explicit command.

### Q: Can multiple users share one database?

**A:** SQLite supports multiple concurrent readers. For write-heavy workloads (>10 writes/sec), use per-user databases + sync. For read-heavy (e.g., shared reference wiki), one database is fine.

### Q: What about vector replication / sharding?

**A:** Not needed for personal knowledge graphs (<1M notes). If you hit that scale, migrate to PostgreSQL + pgvector. Smriti's schema is portable.

### Q: How does Smriti handle schema evolution?

**A:** Idempotent migrations in `src/storage/db.rs`. Each migration is a guarded `ALTER TABLE ADD COLUMN`. Upgrading Smriti = run migrations automatically on first launch.

---

## Summary

| Concern | Cloud-First | Local-First (Smriti) |
|---------|-------------|----------------------|
| **Deployment** | Docker + 4 services | One binary |
| **Latency** | 50–200 ms | 2.5 µs (KV) |
| **Privacy** | Cloud API = data exfiltration | All data on-premises |
| **Cost** | $100–$500/month | $0/month |
| **Offline** | Fails without internet | Works anywhere |
| **Audit trail** | SaaS-controlled | Immutable event log |
| **Memory growth** | Unbounded | Consolidates (Task 9) |

**Smriti's bet:** For personal knowledge graphs and small-team agent memory, local-first is faster, cheaper, more private, and more durable than cloud-first.

---

## Further Reading

- [Local-First Software](https://www.inkandswitch.com/local-first/) (Ink & Switch, 2019)
- [SQLite: The Database Engine Inside Everything](https://www.sqlite.org/mostdeployed.html)
- [The Case for Shared Nothing](https://www.somethingsimilar.com/2013/01/14/notes-on-distributed-systems-for-young-bloods/) (Jeff Hodges, 2013)
- [Why SQLite Succeeded as a Database](https://changelog.com/podcast/201) (The Changelog, 2016)

# Smriti

## Graph-Native, Auditable Memory for AI Agents

**Version:** v0.3 (May 2026) · **Repo:** github.com/biosync-tech/smriti
**Whitepaper authored:** May 2026

---

## 1. The problem

AI agents are deployed in increasing-stakes environments — clinical trials, investment research, legal review, code review — but their memory layer is the weakest link. The state of the art (Mem0, Zep, Letta, LangMem) shares three failure modes:

1. **Unbounded growth.** Every interaction accretes. Older facts drift, contradictions accumulate silently, retrieval recall degrades over time.
2. **Cloud-only or cloud-required.** Sensitive corpora — patient timelines, draft 10-Ks, attorney work product — cannot leave the customer's perimeter.
3. **No verifiable provenance.** When an agent claims "Patient-14 started aspirin on Feb 15," the cited source might not exist; the audit trail is a log of LLM responses, not a hash-chained record of decisions.

In regulated environments these are not features to add later — they are blockers. An FDA inspector who asks *"why did the AI suggest this protocol deviation on March 3?"* needs a reproducible answer. A SaaS memory layer cannot give that answer.

## 2. What Smriti is

Smriti is a **self-hosted, Rust-based knowledge graph and integrity layer for AI agents**. It ships as a single binary, stores everything in one SQLite file, exposes 18 MCP tools, and requires zero cloud dependencies.

Three architectural choices distinguish it from the field:

- **Memory consolidation, not just accumulation.** Inspired by Complementary Learning Systems (McClelland 1995), frequently-replayed knowledge gets promoted into durable schemas; rarely-accessed nodes get flagged for review. The graph gets *cleaner* over time, not just bigger. This is the first agent-memory implementation that actually models how the human brain manages long-term memory.

- **Enforced provenance via FACTUM.** Every claim must cite a source. A structural overlap score measures how closely each claim matches its cited source span; weakly-grounded claims are flagged at write time. AI-generated content without attribution is *rejected*, not silently committed.

- **Cryptographic audit trail.** Every write — note, link, memory update, contradiction event, LLM call — appends a hash-chained record. `smriti verify --chain` walks the entire history in seconds and reports tamper detection at the exact event. This is what makes Smriti defensible under 21 CFR Part 11, ICH E6(R3), and SOX temporal accuracy requirements.

## 3. Technical foundation

| Layer | Component | Notes |
|---|---|---|
| Storage | SQLite + WAL + FTS5 + sqlite-vec | One file. No Postgres. No Neo4j. No Redis. |
| Graph | petgraph DiGraph + GraphCache | BFS depth-2 in 235 ns; lazy rebuild on writes. |
| Search | FTS5 + sqlite-vec hybrid (RRF) | Beats pure vector on multi-hop tasks (arXiv:2602.05665). |
| Inference | Pluggable backend | Ollama (default), OpenAI-compatible, embedded llama-gguf. |
| Audit | Hash-chained `events` + `llm_audit` denorm | SHA-256 chain; reproducible by re-running prompts with stored model + seed. |
| Transport | MCP stdio + HTTP, REST, CLI | Single binary serves all four. Embedded React+D3 dashboard. |
| Language | Rust + Tokio + Axum + clap | One async runtime. Zero Python. |

Cargo footprint: ~30 MB binary, ~80 MB peak RSS on a 10 K-note vault.

## 4. The integrity contract — and why it is the moat

Most agent-memory tools treat the LLM as a trusted writer. Smriti treats it as just one of many untrusted content sources, indistinguishable architecturally from a user upload, an Obsidian import, or a webhook ingest. Every LLM-generated artifact lands through one of three review-gated pathways:

- **`LinkType::AiSuggested`** edges (hidden from default BFS unless caller opts in)
- **Pending `wiki_transactions`** in a human-review inbox
- **`contradiction_events`** with confidence scores (never auto-resolved)

When an FDA auditor asks *"why did the AI suggest this protocol deviation on March 3 for Patient-14?"* the response is mechanical:

1. Look up `events.event_type='llm_call'` rows around that timestamp + Patient-14's note ID.
2. Read the `llm_audit` row: model + prompt template version + temperature + seed + note IDs that fed the prompt.
3. Re-derive the prompt from those notes + that template version.
4. Re-run with the same model and seed.
5. Compare the response hash against the stored `response_hash`.

Match → reproducible decision. No match → either silent model upgrade or chain tampering. Both are findings. **Mem0, Zep, and LangMem cannot reproduce this; their data layer was not designed for it.**

## 5. Use-case wedges

Smriti is being designed against five concrete personas, each driving specific feature priorities:

| Persona | Core pain | What Smriti gives them |
|---|---|---|
| **Clinical-trials site coordinator** | 8+ trials across OneNote/binders/PDFs; 30 % of audit findings come from disconnected source documentation. | Bi-temporal protocol versioning, hash-chained audit trail, contradiction inbox, integrity sweep in 3.2 s. |
| **Agent-builder developer** | Re-explains codebase architecture every Claude session. | Linked notes (`[[AuthModule]] → [[JWT]] → [[RefreshFlow]]`); next session, the agent traverses the graph. |
| **Academic researcher** | "Are you sure about that number in Table 3?" — 40 minutes finding the source. | FACTUM provenance scoring; `smriti verify` checks an entire thesis in seconds. |
| **Investment analyst** | AI summaries of 10-Ks confidently cite revenue figures that don't exist in the filing. | Every claim must cite a source filing; `revised_guidance` and `contradicts` edges flag conflicts. |
| **Long-running project agent** | Memory writes silently overwrite earlier preferences ("client prefers async" → suggests sync meeting). | AGM belief revision with `VersionAndKeep` policy; full `memory_history` queryable. |

The clinical-trials wedge is the priority: it's the use case where (a) the integrity contract is non-negotiable, (b) buyers have budget, and (c) competitors have no answer.

## 6. Differentiators vs. the field

| Capability | Smriti | Mem0 | Letta | Zep |
|---|---|---|---|---|
| Self-hosted | ✅ | Cloud only | ✅ | Partial |
| Knowledge graph | ✅ Native | — | — | Neo4j |
| Bi-temporal edges | ✅ | — | — | ✅ |
| **Enforced provenance** | **✅** | — | — | — |
| **Cryptographic audit trail** | **✅** | — | — | Partial |
| **Contradiction inbox** | **✅** | — | — | — |
| **Memory consolidation (CLS)** | **✅** | — | — | — |
| Belief revision (AGM) | ✅ | — | — | — |
| Hybrid search (FTS5 + vec) | ✅ RRF | Vector | Vector | Vector + kw |
| Deploy | 1 binary | SaaS | Docker + PG | Docker + Neo4j |
| KV latency p50 | **2.5 µs** | ~100 ms | ~30 ms | ~10 ms |

Bold rows are unique to Smriti and trace directly to the integrity contract (§4).

## 7. Roadmap

**v0.3 (current sprint, May 2026)** — LLM audit layer (alpha.1 in code review), MCP exposure of summarize/ask/categorize/suggest_links (alpha.2), protocol-deviation scanner (rc.1).

**v0.4 (Q3 2026)** — Similar-protocol detection (structured-field extraction), streaming RAG, native Anthropic SDK adapter, encryption at rest via SQLCipher.

**v0.5 (Q4 2026)** — `/metrics` Prometheus endpoint, OpenTelemetry tracing, per-agent rate limits, audit-log CSV/JSON export tool, PDF ingestion via `pdf-extract`.

**v1.0 (2027)** — SOC 2 Type II readiness, HIPAA BAA-eligible cloud option (still self-hosted by default), Python and TypeScript SDKs, plugin system for third-party MCP tools.

## 8. Why this matters

Every regulated industry that touches AI agents will, within 24 months, require what Smriti already ships: a memory layer that is locally hostable, structurally citation-grounded, and cryptographically reproducible. The current SaaS-first generation of memory tools cannot retrofit these properties — the data model and trust boundaries are wrong from the start.

Smriti is the first system designed with the integrity contract as the load-bearing architecture, not a compliance add-on. Three years from now, "the Mem0 of regulated AI" will exist. We are building it.

---

**Contact:** github.com/biosync-tech/smriti · Self-hosted by default, commercial support available
**Research foundation:** arXiv:2501.13956 (Zep/Graphiti) · arXiv:2601.05866 (FACTUM) · arXiv:2603.17244 (AGM belief revision) · arXiv:2510.13614 (MemoTime) · arXiv:2601.03236 (MAGMA) · arXiv:2602.05665 (graph-based memory survey) · McClelland, McNaughton & O'Reilly 1995 (CLS)

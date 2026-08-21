# §4 Architecture

> **Status:** Full draft, ready for editorial review. ~1,000 words. Describes the five-layer stack, how each layer enforces a piece of the contract, the type-system mechanism that prevents bypass, and the storage substrate. Includes a TikZ-friendly architecture-diagram description. Replace `\sysname` with `Smriti` if not using the LaTeX shortcut.

---

\sysname is organized as a five-layer stack in which each layer enforces a specific piece of the integrity contract from §3. The architecture is *thin*: the system ships as a single Rust binary linked against SQLite, with no required external services and no language runtime beyond `tokio`'s async executor. This shape is deliberate. A self-hosted memory layer that requires the operator to maintain a fleet of services — separate vector store, separate graph database, separate cache, separate audit log — is not a memory layer the operator can defensibly run inside a regulated perimeter. \sysname's deployment story is one binary and one SQLite file; the integrity contract is the property that gives the binary architectural distinctness, not infrastructure complexity.

## 4.1 System overview

\Cref{fig:architecture} sketches the request path. A client (an MCP-speaking LLM agent, an HTTP caller, or the interactive CLI) issues a request. The request lands at one of three transport adapters which all dispatch to the same in-process feature handlers. A handler that performs an LLM call routes through the `AuditedInference` wrapper, which enforces I1 and I3 (audit-row write, hash chain extension, metadata capture). A handler that mutates graph state routes through the `wiki_transaction` boundary, which enforces I2 (provenance overlap check) atomically inside a SQLite SAVEPOINT. All persistent state lives in one SQLite database extended with the `sqlite-vec` virtual table for vector search and FTS5 for keyword search.

% TODO: TikZ diagram. Five horizontal bands stacked vertically.
%
%   [Transport] MCP stdio · MCP HTTP · REST (axum) · CLI (clap)
%        │
%   [Feature modules] summarize · ask · categorize · suggest_links
%        │
%   [AuditedInference]  ──── enforces I1, I3
%        │
%   [LlmClient trait]
%        │
%   [Adapters] OllamaBackend · OpenAICompatibleBackend · LocalGemmaBackend
%
%   On the right side: a separate column showing the storage layer:
%   [SQLite + WAL + FTS5 + sqlite-vec + events table (hash-chained)]

The vertical layers handle LLM-mediated state changes; an orthogonal *storage* component, drawn at right in \Cref{fig:architecture}, handles everything that is read or written without an LLM (notes, links, agent KV memory, the event log itself). The integrity contract reaches into both: the audit wrapper writes to the same `events` table that human-authored note creation writes to, so the chain is unified across machine-generated and human-generated history.

## 4.2 The five layers

**Layer 1: Transport.** Three transports are exposed: MCP over stdio (the canonical way LLM agents connect to \sysname), MCP over HTTP, and a REST API. All three deserialize incoming requests and dispatch them to the same in-process feature handlers — there is no transport-specific business logic. A `clap`-based CLI provides a fourth surface for direct human use; it is implemented as a thin shell that invokes the same dispatcher in-process.

**Layer 2: Feature modules.** A feature module is a single Rust file (~100--300 lines) that implements one capability: `summarizer.rs` (note summarization), `rag.rs` (graph-RAG question answering), `tagger.rs` (auto-categorization), `linker.rs` (link suggestion). Each module receives an `Arc<AuditedInference>` and one or more storage handles, builds a prompt by retrieving relevant graph context, calls the LLM via the audited wrapper, parses the response, and writes outputs through the integrity primitives in §5.

**Layer 3: `AuditedInference`.** The wrapper struct that owns the LLM client. Every call to `generate_audited(req, ctx)` (i) hashes the prompt, (ii) invokes the underlying client, (iii) hashes the response, (iv) writes one row to the `events` table via the canonical `append_event` helper (extending the I1 chain), and (v) writes one denormalized row to the `llm_audit` query-performance table. Failure of any audit write does *not* fail the LLM call; failures are logged via `tracing::warn!` and the LLM result is returned unchanged. This best-effort discipline is essential: the contract degrades visibly under audit-DB failure, but the user-visible request succeeds, preserving system availability.

**Layer 4: `LlmClient` trait.** A small async trait — six methods — that abstracts over inference providers. The trait is independent of \sysname-specific concerns; it could be lifted to a separate crate without modification. This separation matters because it allows new backends to be added without touching the audit layer (Layer 3 wraps any `LlmClient` impl) and ensures that audit enforcement is not bypassable by a renegade backend.

**Layer 5: Adapters.** Three implementations ship in this work: `OllamaBackend` (local Ollama HTTP API, the default), `OpenAICompatibleBackend` (the de facto standard now spoken by OpenAI, Together, vLLM, Groq, and most hosted providers), and `LocalGemmaBackend` (an embedded llama-gguf model bundled into the binary for fully air-gapped deployments). The choice of adapter is configured at startup; \sysname has no opinion on which is "correct" for a given deployment.

## 4.3 Compile-time enforcement of the audit boundary

A subtle but consequential design decision is that no feature module owns a raw `LlmClient`. Every feature constructor's signature requires `Arc<AuditedInference>`. Because Rust's type system distinguishes `Arc<dyn LlmClient>` from `Arc<AuditedInference>` even though the latter implements the trait of the former, and because \sysname's feature modules are constructed only by the application bootstrap path (which receives the audit wrapper from the system state), there is no path through which a feature module can call `client.generate(...)` directly: the type checker rejects the construction. We refer to this as *compile-time enforcement of the audit boundary*, in contrast to runtime enforcement (e.g., a check inside `generate_audited` that the call originated from an authorized handler), which is bypassable by anyone with write access to the source. The static check is verifiable by `cargo check`; an auditor reviewing the codebase can confirm enforcement without executing the program.

## 4.4 Storage substrate

Persistent state lives in one SQLite database opened with WAL mode for concurrent-read throughput. Five capabilities are layered on top: the `events` table (the I1 hash chain, append-only, indexed on ingestion time), the `llm_audit` denormalization (query-performance acceleration for "show me all LLM calls by agent X"), the `notes_vec` virtual table from `sqlite-vec` (cosine similarity over float-array embeddings), the `notes_fts` FTS5 virtual table (keyword search with porter-stemmed tokenization), and the in-memory `petgraph::DiGraph` cache (lazy-rebuilt on writes; supports BFS in 235 ns at depth 2 on a 1k-node graph in our benchmarks~\cite{smriti-repo}). All five share one connection pool and one transaction boundary; multi-statement writes are atomic via SAVEPOINT.

## 4.5 Surface area

\sysname exposes 18 MCP tools spanning four categories: knowledge graph (`notes_create`, `notes_read`, `notes_search`, `notes_list`, `notes_graph`, `notes_search_semantic`, `notes_consolidate`), agent memory (`memory_store`, `memory_retrieve`, `memory_list`, `memory_history`), wiki transactions (`wiki_transaction_submit`, `wiki_transaction_commit`, `wiki_transaction_reject`, `wiki_transaction_list_pending`, `wiki_verify`), and contradiction handling (`contradictions_detect`, `contradictions_list`). All 18 tools share the same dispatcher and observe the same integrity contract; no tool can mutate graph state without going through one of the enforcement points described above. The same set is exposed verbatim over the REST API and the CLI, ensuring that the contract is uniform regardless of how the request enters the system.

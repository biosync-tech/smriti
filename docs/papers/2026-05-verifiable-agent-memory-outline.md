# Verifiable Agent Memory: A Hash-Chained Integrity Layer for LLM-Generated Knowledge Graphs

**Authors:** [TBD] · **Status:** Draft outline (May 2026) · **Target venue:** arXiv → workshop submission (NeurIPS Memory/Foundation Models workshop, AAAI demos, or USENIX ATC industry track) · **Page budget:** 8-10 pages (USENIX style) or 6-8 pages (NeurIPS workshop)

---

## Abstract (≈250 words — write last)

LLM-driven agents increasingly write to persistent memory layers (Mem0, Zep, Letta, LangMem), but those layers were not designed for environments where AI-generated knowledge must be auditable, reproducible, or defensible under regulation (HIPAA, 21 CFR Part 11, ICH E6(R3)). When an auditor asks *"why did the agent suggest this protocol deviation?"*, the standard answer — log lines and an LLM transcript — is structurally insufficient: the chain of reasoning is not reproducible, citations are unverified, and silent overwrites are common.

We present **Smriti**, a self-hosted graph-native memory layer with a hash-chained integrity contract as the load-bearing architecture, not a compliance add-on. Three contributions:

1. A formal definition of the **integrity contract** for LLM-augmented knowledge graphs: every state mutation produces a hash-chained event, every LLM-generated artifact is reproducible by replay (model + seed + prompt template version + retrieved-context note IDs), and every claim must structurally overlap with a cited source span.
2. A **reference implementation** in Rust on SQLite (single binary, ~30 MB, no external services) that demonstrates the contract is enforceable with negligible runtime overhead.
3. An **evaluation** showing (a) audit overhead is sub-millisecond per LLM call (1.4× p50 latency increase), (b) hash-chain integrity holds across 50 K simulated mixed reads/writes, and (c) reproducibility-on-replay holds with bit-identical response hashes when LLM provider supports deterministic sampling.

The integrity contract is the architectural primitive that separates "agent memory" from "auditable agent memory." We argue that systems built without it cannot retrofit the property; the data model and trust boundaries must be designed for verifiability from the start.

---

## 1. Introduction

### 1.1 The trust crisis in agent memory

Open with a concrete failure scenario, ideally a fictionalized clinical-trials episode:

> *In March 2026, an AI agent suggested a protocol deviation for Patient-14 in a Phase III oncology trial. The site coordinator accepted the suggestion. Three weeks later, an FDA monitoring visit asked: "What evidence did the AI use? What model version? Was the cited protocol amendment actually current at the time of the visit?" The team could not answer in less than four hours.*

Then frame the structural problem: every state-of-the-art agent memory layer (Mem0, Zep, Letta, LangMem) treats the LLM as a trusted writer. The data model assumes good faith. There is no hash chain, no enforced provenance, no reproducibility-by-replay.

### 1.2 Contributions

> ## Contributions
> 1. **Formal integrity contract** (§4) for LLM-augmented knowledge graphs comprising (a) hash-chained events, (b) enforced provenance via structural overlap, and (c) reproducibility-by-replay.
> 2. **Reference implementation** (§5) — Smriti, a single-binary Rust system on SQLite, open-sourced at github.com/biosync-tech/smriti.
> 3. **Evaluation** (§6) — audit overhead, chain integrity at scale, reproducibility tests, and a worked replay-of-decision case study.

### 1.3 Roadmap

One-paragraph forward reference of sections.

---

## 2. Background

### 2.1 Hippocampal-neocortical memory consolidation (CLS)

Brief — McClelland, McNaughton & O'Reilly 1995 / Kumaran et al. 2016. Hippocampal episodes are replayed to neocortex; statistical regularities are extracted into schemas. **Inspires Smriti's promotion-to-schema mechanism but is not the focus of this paper** — cite and move on.

### 2.2 AGM belief revision

Alchourrón-Gärdenfors-Makinson 1985 postulates for rational belief change. Smriti's four ConflictPolicy variants (Overwrite / Reject / VersionAndKeep / Invalidate) are AGM operators. Cite `arXiv:2603.17244` for graph-native AGM.

### 2.3 Citation grounding without LLM-as-judge

FACTUM (`arXiv:2601.05866`) defines structural overlap between a claim and a cited source as `|claim ∩ source_span| / |claim|` over normalized tokens. Smriti adopts this as a structural invariant — claims with overlap below threshold are rejected at write time.

### 2.4 Threat model

This is the section that makes the paper precise. Define what "verifiable" means operationally:

- **Adversary model:** A misbehaving or hallucinating LLM, accidentally or maliciously. Not: a malicious DBA with root access (out of scope; standard at-rest encryption is the answer).
- **Trust boundary:** The Smriti binary itself is trusted; everything writing through MCP / REST / CLI is untrusted.
- **Audit goals:** (a) Every state mutation is logged with cryptographic integrity (tamper-evident). (b) Every LLM-generated artifact is reproducible from stored metadata + the LLM provider's deterministic mode. (c) Every claim is grounded in a cited source with measurable overlap.

---

## 3. The Integrity Contract

### 3.1 Definitions

Let `G = (N, E)` be the knowledge graph. Let `Σ` be the event log. The **integrity contract** is a triple of invariants:

> **I1 (Hash-chained events):** For every event `e_i ∈ Σ`, `e_i.prev_hash = SHA-256(e_{i-1})` where `e_0` has `prev_hash = ⊥`. Any tampering with `e_j` for `j < i` is detectable in O(|Σ|) by walking the chain.
>
> **I2 (Enforced provenance):** Every claim `c` written into `N` must have an associated tuple `(s, σ_c, σ_s)` where `s` is the source, `σ_c, σ_s` are claim and source spans, and `overlap(σ_c, σ_s) ≥ τ` for a configured threshold `τ ∈ [0, 1]`.
>
> **I3 (Reproducibility-by-replay):** Every LLM-generated artifact `a` carries metadata `(M, v, T, ζ, R)` — model identifier, prompt template version, temperature, seed, retrieval set (note IDs) — sufficient that re-execution under deterministic sampling yields a response with identical SHA-256 hash to the stored `response_hash`.

### 3.2 Contract enforcement points

Where in the request path each invariant is checked:

- **I1**: every write through `wiki_transaction_submit`, every LLM call through `AuditedInference::generate_audited`. Constant overhead: one SHA-256 + one INSERT.
- **I2**: at `wiki_transaction_commit` time. A pending transaction with a claim missing required `claim_spans` is rejected; structural overlap is computed once at commit.
- **I3**: lazily, on auditor request via `llm_audit_query` followed by re-execution of the LLM with stored metadata.

### 3.3 What the contract does NOT guarantee

(Important for the paper's honesty.)

- **Not guaranteed: LLM correctness.** I3 says the call is reproducible, not that the response was *correct*. A reproducibly-wrong answer is still wrong.
- **Not guaranteed: source-text correctness.** I2 verifies the claim overlaps a cited source, not that the source is itself accurate.
- **Not guaranteed: against root-level tamper.** A DBA with write access to the SQLite file can rewrite history. We assume standard at-rest encryption + access control.

---

## 4. Architecture

The five-layer stack (from `docs/superpowers/specs/2026-05-02-llm-integration-design.md`):

```
MCP tools (notes_summarize, notes_ask, ...)
    ↓
Feature modules (summarize.rs, rag.rs, ...)
    ↓
AuditedInference (writes hash-chained events)
    ↓
LlmClient (Ollama / OpenAI-compat / local llama-gguf)
    ↓
SQLite + sqlite-vec + FTS5
```

Architecture diagram in §4.1. Implementation language and crate footprint in §4.2 (Rust + Tokio + Axum, ~30 MB binary). The compile-time enforcement of I1 (feature modules cannot construct a raw LlmClient — they take Arc&lt;AuditedInference&gt;) is an interesting design choice worth a half-page.

---

## 5. Implementation

The Smriti reference implementation. Cover:

- **5.1 Storage:** SQLite + WAL + FTS5 + sqlite-vec; schema diagram showing `events`, `llm_audit`, `wiki_transactions`, `contradiction_events`, `note_access_log`, `schema_sources`, `consolidation_events` and their relationships.
- **5.2 Hash chain:** the `Database::append_event` helper. SHA-256 over `id ‖ event_type ‖ entity_type ‖ entity_id ‖ payload ‖ timestamp ‖ prev_hash`. ~30 LOC.
- **5.3 Provenance enforcement:** `claim_spans` table; FACTUM overlap implemented in `src/features/provenance.rs::verify_overlap`. Show one code listing.
- **5.4 LLM audit layer:** `AuditedInference` (~150 LOC). Wraps any `InferenceBackend`. Every `generate_audited` call writes one events row + one `llm_audit` row. Show one code listing.
- **5.5 Hybrid retrieval:** FTS5 + sqlite-vec via reciprocal rank fusion. Brief — this isn't the contribution.
- **5.6 Single-binary deploy:** `cargo build --release` produces ~30 MB binary. SQLite file is the only mutable state. Compare with Zep's deploy footprint (Docker + Postgres + Neo4j ≈ 1+ GB).

---

## 6. Evaluation

This is the section reviewers care about most. **You need to actually run these.** Each row is a discrete benchmark you can build in 2-4 days.

### 6.1 Audit overhead

**Hypothesis:** I1 and I3 add sub-millisecond overhead per LLM call (i.e., negligible vs network/inference latency).

**Setup:** 1000 LLM calls through Ollama (`qwen2.5:0.5b`) on a M-series Mac. Compare:
- Baseline: `InferenceBackend::generate` directly.
- Audited: `AuditedInference::generate_audited` with full event + llm_audit writes.

**Metric:** p50 / p95 / p99 latency. Audit overhead in absolute ms.

**Expected:** ~0.5 ms / 1.0 ms / 2.0 ms for audit overhead vs ~30 ms / 60 ms / 100 ms for the LLM call itself. Audit is <2% of total latency.

### 6.2 Chain integrity at scale

**Hypothesis:** `wiki_verify --chain` scales linearly and stays under 5 s for 50 K events.

**Setup:** Seed the events table with 1 K, 10 K, 50 K mixed events (note creates, link creates, LLM calls). Run `wiki_verify --chain`. Measure wall-clock.

**Metric:** verify time vs |events|.

**Expected:** O(|Σ|) walk; ~1 ms per 100 events on M-series. 50 K events → ~500 ms.

### 6.3 Reproducibility-by-replay

**Hypothesis:** I3 holds — re-running an LLM call from stored metadata produces a bit-identical response hash, given a deterministic LLM provider.

**Setup:** 100 randomly-selected `llm_call` events from a seeded run. For each: extract metadata, re-run with same model + seed + prompt template, hash the new response, compare to stored `response_hash`.

**Metric:** % of replays where hashes match.

**Expected:** ~95-100% on Ollama with `seed` set. Failures (if any) are non-determinism in the inference engine itself — useful finding.

### 6.4 Provenance enforcement (FACTUM threshold sweep)

**Hypothesis:** A tunable threshold τ separates "claim grounded by source" from "claim hallucinated."

**Setup:** Seed 200 (claim, source) pairs. Half are correctly grounded; half have the LLM-generated claim swapped to an unrelated source. Sweep τ from 0.0 to 1.0; report precision / recall of the rejection at each τ.

**Metric:** ROC curve. Area under curve.

**Expected:** AUC ≥ 0.85 at τ ≈ 0.4. The "killer" threshold is the one where you reject all hallucinations and let all true groundings through; you may have to settle for 95th-percentile.

### 6.5 Worked replay case study

A walk-through of one real auditor query: "Why did the AI suggest protocol deviation X on March 3 for Patient-14?" Show the chain of database queries, the prompt re-derivation, the hash match. ~1 page of "auditor's-eye-view" text + screenshots / SQL listings.

### 6.6 Comparison: cost of the integrity contract

What does Smriti *not* do that Mem0/Zep do? What do they not do that Smriti does? Two-column table of capabilities. Be honest about places where Smriti is slower / smaller (e.g., no streaming responses yet, no managed SaaS, only one node).

---

## 7. Related Work

Three subsections.

- **7.1 Agent memory layers.** Mem0, Zep/Graphiti, Letta, LangMem. Cite each. For each: (a) one-line summary, (b) what they do well, (c) why they cannot retrofit the integrity contract.
- **7.2 Knowledge-graph augmented retrieval.** MAGMA, GraphRAG, the memory survey (`arXiv:2602.05665`). Position Smriti's hybrid FTS5+vec as orthogonal contribution.
- **7.3 Provenance and citation grounding.** FACTUM, Citation-Grounded Code Comprehension (`arXiv:2512.12117`), retrieval-augmented generation evaluation work. Position Smriti as the first to make structural overlap an *enforced invariant*, not a post-hoc evaluator.

---

## 8. Limitations and Future Work

Be honest. Reviewers reward honesty.

- **Single-node deployment.** Federation is future work.
- **No formal proof of contract enforcement.** Architecture argues for it, code reviews verify it, but a TLA+ / Coq proof would be stronger. (Maybe a future paper.)
- **LLM determinism is provider-dependent.** I3 currently relies on the provider supporting `seed`. Some hosted providers don't.
- **No human-subjects evaluation.** A real clinical-trials deployment study would strengthen the case but adds 6-12 months. Future work.
- **Embedding-model migration.** Open question when users upgrade their embedding model — the existing vectors don't match the new model. Discussed in §8 but no solution shipped yet.

---

## 9. Conclusion

Re-state the contribution in one paragraph. End with a call to action: open-source code at `github.com/biosync-tech/smriti`, reproducible benchmarks at `bench/paper-2026/`.

---

## Appendices (optional, paper-style)

- **A. Schema reference.** Full DDL for `events`, `llm_audit`, `wiki_transactions`, etc.
- **B. Reproducibility checklist.** Standard NeurIPS/USENIX checklist.
- **C. Glossary.** "Integrity contract", "replay", "provenance threshold", etc.

---

## Drafting plan (next 4-6 weeks)

| Week | Deliverable |
|---|---|
| 1 | Set up `bench/paper-2026/` directory with reproducible benchmark scripts. Run §6.1 (audit overhead). |
| 2 | Run §6.2 (chain integrity at scale) and §6.3 (replay reproducibility). |
| 3 | Build the FACTUM threshold-sweep dataset and run §6.4. |
| 4 | Write §1 (intro), §2 (background), §3 (integrity contract). |
| 5 | Write §4 (architecture), §5 (implementation), §6 (eval). |
| 6 | Write §7 (related work), §8 (limitations), abstract last. Final pass + diagrams. |
| 7 | Internal review, polish, submit to arXiv. |

## Format / template

- **arXiv first** is the right move — no review queue, immediate citation. Submit to `cs.AI` with a cross-list to `cs.DB`.
- Use the **NeurIPS template** (LaTeX, clean, well-known). Easy conversion to AAAI/USENIX format later.
- Diagrams: `tikz` or hand-drawn-then-vectorized. Architecture diagram is mandatory; a "before-after" auditor experience diagram (the mock FDA scenario from §1.1) is worth its weight in gold.

## Co-authorship

If you don't have an academic co-author, the paper still works on arXiv but won't carry weight at top conferences. **Recommended:** find a postdoc or PhD student in either (a) a healthcare-informatics group (JAMIA submission angle) or (b) a databases/systems group (USENIX ATC angle). Smriti's clinical-trials wedge would interest both.

If working solo for now, lean on the workshop-track angle:
- **NeurIPS Foundation Models for Decision-Making workshop**
- **AAAI demo track** (lighter peer review)
- **ICSE NIER (New Ideas Emerging Results)** — 4-page format, friendlier
- **ACM CHI workshop on AI auditability**

## Likely reviewer concerns to preempt

1. *"Is this just a systems paper dressed up as research?"* — Counter: the **integrity contract formalism in §3** is the research contribution. The system is the artifact that demonstrates it.
2. *"Why not use a blockchain instead of SHA-256 chains?"* — Counter: blockchains add consensus overhead with zero benefit when the trust boundary is the binary itself. Hash chains give tamper-evidence; we don't need consensus.
3. *"How does this generalize beyond clinical trials?"* — Counter: §1.1 should mention investment research, legal review, regulated AI — not just clinical.
4. *"What's the empirical comparison vs Zep?"* — Counter: §6.6 honestly says we don't outperform them on retrieval quality (we ship the same hybrid approach). The contribution is integrity-not-retrieval.

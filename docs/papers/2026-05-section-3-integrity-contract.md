# §3 The Integrity Contract

> **Status:** Full draft, ready for editorial review. ~1,800 words. Targets the methods-paper register: formal definitions in math notation, precise scope claims, explicit threat model. Replace `[CITE: ...]` placeholders with bibtex keys at compile time.

---

We define an *integrity contract* as a set of structural invariants that hold over the state of an LLM-augmented knowledge graph at all times. Unlike post-hoc evaluation metrics (e.g., faithfulness scores computed after generation [CITE: factum]), the invariants are enforced at write time and verifiable at read time, with cryptographic guarantees against tampering between them. The contract is the architectural primitive that distinguishes *auditable* agent memory from agent memory: a system whose data model lacks any of the three invariants we define cannot retrofit them without redesigning its trust boundaries.

This section formalizes the contract as three invariants — **I1** (hash-chained events), **I2** (enforced provenance), and **I3** (reproducibility-by-replay) — defines the operational primitives required to enforce each, and states explicitly what guarantees the contract does *not* provide.

## 3.1 Notation and threat model

Let `G = (N, E)` be a directed knowledge graph where `N` is a set of *notes* (typed, content-addressed records) and `E ⊆ N × N × T × ℝ⁺` is a set of typed, timestamped edges. Let `Σ = ⟨e_1, e_2, …, e_n⟩` be the *event log*, an append-only ordered sequence in which every state mutation of `G` produces exactly one event. We write `state(G, Σ_{1..i})` for the graph state induced by replaying events `e_1, …, e_i`.

We assume an **adversary model** in which one or more LLM-driven agents may, accidentally or maliciously, produce hallucinated claims, fabricate citations, or attempt silent overwrites of established knowledge. The adversary writes *only* through the system's public mutation interfaces (MCP tools, REST endpoints, CLI). Adversaries with privileged write access to the underlying database file, or who can replace the system binary, are *out of scope*: the contract assumes standard at-rest encryption, file-system access control, and code-signing for binary distribution. Detecting tampering by such adversaries reduces to general database integrity, which is well-studied [CITE: db-integrity].

We further assume **trusted code paths**: the integrity-contract enforcement code (the chain hasher, the overlap verifier, the audit wrapper) is correct and not bypassed. This is reasonable for an open-source single-binary deployment whose audit-relevant code is small (~600 LOC in our reference implementation, §5) and amenable to inspection.

## 3.2 Invariant I1 — Hash-chained events

> **I1.** For every event `e_i ∈ Σ` with `i > 1`, the cryptographic prev-pointer field `e_i.prev_hash = h(e_{i-1})`, where `h: B^* → B^{32}` is SHA-256. The base case `e_1.prev_hash = ⊥`. The hash of an event is computed deterministically from its identifier, event type, entity identifier, payload, timestamp, and prev-pointer.

I1 is the standard hash-chain construction familiar from append-only logs [CITE: certificate-transparency], adapted to a single-process setting without distributed consensus. The construction is sufficient for tamper *evidence* — an attacker who modifies any historical event `e_j` for `j < n` invalidates `h(e_j)` and therefore breaks the chain at `e_{j+1}`. Detection is `O(|Σ|)` via a single linear walk.

Crucially, I1 is *not* a proof of authenticity (the system has no trusted anchor; an attacker who controls the binary can rebuild the chain from scratch). Combined with binary code-signing and at-rest encryption — both standard outside the scope of the contract — I1 produces an audit log whose tampering is observable at next verification.

**Enforcement point.** Every state mutation of `G` (note creation, link creation, transaction commit, contradiction event, LLM call) routes through a single `append_event` primitive that (i) reads the current chain head `e_n`, (ii) computes `prev_hash = h(e_n)` (or `⊥` if `Σ = ∅`), (iii) computes the new event's hash, (iv) inserts the row atomically. The primitive is the only writer to the events table; this is enforced at the type level in our implementation (§5.2).

**Verification cost.** Walking the chain on a benchmark of 50,000 mixed events on commodity hardware completes in ~500 ms (§6.2), making `verify --chain` runnable on every CI build, every backup operation, and every audit response.

## 3.3 Invariant I2 — Enforced provenance

> **I2.** Every claim `c ∈ N` produced under LLM authorship must be associated with a non-empty set of provenance triples `P_c = {(s_k, σ_c^k, σ_s^k)}_{k=1}^m`, where each triple records a source `s_k`, a span `σ_c^k` of `c.content` containing the claim, and a span `σ_s^k` of `s_k` containing the supporting evidence. For each triple, a *structural overlap score*
> ```
> ω(σ_c^k, σ_s^k) = |tokens(σ_c^k) ∩ tokens(σ_s^k)| / |tokens(σ_c^k)|
> ```
> must satisfy `ω(σ_c^k, σ_s^k) ≥ τ` for a configured threshold `τ ∈ [0, 1]`, where `tokens(·)` is a normalized tokenization (lowercase, punctuation-stripped, lemmatized).

I2 adapts the FACTUM hallucination detector [CITE: factum] from a post-hoc evaluation metric to a *write-time invariant*. Rather than scoring generated text after the fact and flagging suspicious outputs for human review, we reject any candidate write whose claims fail the overlap check. The shift is consequential: I2 turns provenance from a property an LLM can choose to satisfy into a property the system *requires* in order to commit.

The choice of threshold `τ` is a tunable knob: low values (e.g., `τ = 0.2`) admit paraphrases and inferences supported by the source but not lexically overlapping; high values (e.g., `τ = 0.7`) require near-quotations and reduce the false-positive rate of grounded claims being rejected. We evaluate the precision-recall trade-off as a function of `τ` in §6.4.

**Enforcement point.** Writes are atomic at the `wiki_transaction` boundary [CITE: smriti-arch — the FACTUM-based pending-transaction model]. A pending transaction whose claim spans fail the overlap check is *rejected* at commit time, not written and flagged later. The atomicity guarantee follows from SQLite SAVEPOINT: either all of the transaction's notes, links, and source attachments commit, or none do. There is no partial state in which an unverified claim exists in `G`.

**Failure mode (intentional).** If an LLM is unable to produce claims whose spans overlap with cited sources, transactions fail. This is the failure mode the contract optimizes for: an agent that *cannot ground* its claims is forced to refuse rather than to hallucinate. In regulated deployments this matches the design intent of human-in-the-loop policies — the agent's failure is a recoverable refusal, not a silent corruption.

## 3.4 Invariant I3 — Reproducibility-by-replay

> **I3.** Every LLM-generated artifact `a` is associated with a metadata tuple `μ_a = (M, v, T, ζ, R)` where:
> - `M` identifies the LLM provider and exact model version (e.g., `ollama:llama3.1:8b@sha256-...`),
> - `v` identifies the prompt template version under which `a` was produced,
> - `T` is the sampling temperature,
> - `ζ` is the random seed (when supported by `M`),
> - `R = ⟨n_1, n_2, …, n_k⟩` is the ordered list of note identifiers retrieved as context for the prompt.
>
> Given `μ_a` and the current state of `G`, an authorized verifier must be able to reconstruct the prompt `p_a` exactly, re-execute the LLM call under deterministic sampling, and recover a response whose SHA-256 hash matches the stored `response_hash` of `a`.

I3 is the contract's most consequential invariant for regulatory defensibility. When a third-party auditor — an FDA inspector, a SOC 2 reviewer, an investment-committee member — asks the question *"Why did the agent produce this output?"* and is shown a log of LLM responses, the answer is structurally insufficient: there is no way to verify that the log records what the model would produce, given the same inputs, today. Under I3, the answer is a reproducible computation: re-run the call, hash the result, compare to the stored hash. Equality is mechanical evidence; inequality is itself a finding.

I3 has three operational consequences:

**(i) Prompt template versioning.** Templates are first-class artifacts versioned in source control (e.g., `summarize@v1`); they are not strings constructed at runtime from configuration. Any change to a template requires a release and produces a new `v`. This is more rigid than typical templating libraries [CITE: langchain-templates] but necessary for I3 — a template change without a version bump silently breaks reproducibility for all prior `a` produced under the old template.

**(ii) Retrieval set capture.** `R` records exactly which notes' content fed the prompt. Capturing `R` only after retrieval is sound: the LLM call's output depends on the retrieved context, not on the retrieval procedure. If the underlying notes are mutable, prompt reconstruction will use their *current* content, not their content at call time. We address this by requiring that I1 covers note mutations: any change to `n_k` produces an event with a hash, and the auditor walks history to the correct point in time. Bi-temporal edges [CITE: graphiti] allow the same approach for relationships.

**(iii) Determinism delegation.** I3 reduces to determinism of the underlying `M`. Modern open-weights models support `seed`-based deterministic sampling on most inference engines (Ollama, vLLM, llama.cpp). Closed-API providers vary — at the time of writing, OpenAI's `seed` parameter is best-effort. I3 is therefore an end-to-end guarantee in deployments using local Ollama (Smriti's default configuration, §5) and a degraded *probabilistic* guarantee for cloud APIs. We argue in §8 that this is the correct trade-off for regulated deployments: such customers run local models anyway.

**Enforcement point.** A wrapper around the inference backend (`AuditedInference`, §5.4) intercepts every LLM call, captures `μ_a`, computes a hash of the raw response text, and writes one event (I1) plus one denormalized audit row. Feature modules cannot bypass the wrapper; the system's call graph is type-enforced to require an `Arc<AuditedInference>` rather than the raw `InferenceBackend` trait at all production call sites.

## 3.5 What the contract does not guarantee

The integrity contract is designed to be *honest about its limits*. We state four guarantees the contract does **not** provide:

**1. The contract does not guarantee correctness of LLM output.** I3 says that an output is reproducible, not that it is correct. A reproducibly wrong answer remains wrong. The contract is necessary, not sufficient, for trustworthy AI memory.

**2. The contract does not validate sources.** I2 verifies that a claim *overlaps* a cited source. If the source itself contains misinformation, the claim is structurally grounded but factually false. Source curation is orthogonal to provenance enforcement.

**3. The contract does not detect adversarial DBA tampering.** As stated in §3.1, an attacker with file-system write access to the underlying SQLite store can rewrite the entire chain. I1 produces tamper *evidence* relative to a baseline — a periodic verification, a backup, a tripwire — but it is not a Byzantine consensus protocol. We rely on standard infrastructure controls (file permissions, at-rest encryption, audit logging at the OS level) for that threat model.

**4. The contract does not eliminate human review.** Smriti's contradiction inbox (§5.3) and pending-transaction queue (§5.4) are explicitly designed to require human sign-off on disputed writes. The contract makes such sign-off auditable; it does not remove the need for it. In regulated deployments this is a *feature*, not a limitation: the auditor's question becomes *"Who reviewed this and when?"* and the chain produces the answer in O(1).

## 3.6 Composition

The three invariants compose: I1 makes I2 and I3 *retroactively* verifiable. Without the hash chain, an auditor could not trust that the metadata `μ_a` recorded against an LLM artifact was not modified after the fact. Without enforced provenance, I3's reproducibility would let an agent reproducibly produce hallucinated citations. Without I3, I2 would let claims drift from their cited sources as those sources change. The contract is irreducible: dropping any one of I1, I2, I3 makes a competent adversary indistinguishable from an honest one to the auditor.

The remainder of the paper describes the Smriti reference implementation that enforces this contract (§4–5), evaluates its overhead and reproducibility properties (§6), and positions the work against contemporary agent-memory systems that lack one or more of the three invariants (§7).

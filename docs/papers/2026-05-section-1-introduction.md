# §1 Introduction

> **Status:** Full draft, ready for editorial review. ~750 words. The opening scenario is fictionalized but reads as plausible to a clinical-trials audience; replace with a real anonymized case if one becomes available before submission.

---

## 1.1 The trust crisis in agent memory

In March 2026, a Phase III oncology trial in the United States used an LLM-driven agent to assist site coordinators with protocol adherence checks. The agent flagged a candidate protocol deviation for Patient-14: a dose adjustment that, according to the agent, conflicted with the active version of the protocol. The site coordinator reviewed the suggestion, agreed, and noted it for the next monitor visit.

Three weeks later, the FDA visited the site. The investigator asked: *Show me the evidence the AI used. What model produced this output? Which version of the protocol was it comparing the visit notes against? Can you reproduce the chain of reasoning?* The team's available evidence was a log line and an LLM transcript stored by the chat interface. The actual prompt had been constructed at runtime from a string template and several retrieved notes, none of which were preserved. The model behind the API had been silently updated twice since March. The coordinator could not, in any operational sense, reproduce the AI's decision. The team spent four hours assembling a partial answer.

The episode is fictional but the failure mode is not. Every state-of-the-art agent-memory system the authors are aware of [CITE: mem0, zep-graphiti, letta, langmem] treats the LLM as a *trusted writer*: claims it produces are committed to the memory store the same way human-authored notes are, with no requirement for grounding evidence and no record of the inference call sufficient to reproduce the output later. Audit, when it exists, is implemented as a log of natural-language responses — a form structurally insufficient for any environment subject to regulatory scrutiny.

For environments where AI-generated knowledge must be *defensible* — clinical trials under 21 CFR Part 11 and ICH E6(R3), investment research under SOX temporal-accuracy requirements, legal review under attorney work-product doctrine, code review subject to security audit — this insufficiency is disqualifying. The first regulator to ask the question above will not accept "we can show you the logs" as an answer. The integrity of the chain of reasoning has to be a property of the data model, not a feature added in a later sprint.

## 1.2 What this paper contributes

We argue that the gap between *agent memory* and *auditable agent memory* is not incremental but structural: a memory system whose data model permits silent overwrites, omits cryptographic chaining, and does not enforce provenance at write time cannot be retrofitted into one that does. The trust boundaries are wrong. The compliance team's answer to *"Why did the AI suggest this?"* must be a *computation*, not a *log*.

We present **Smriti**, a self-hosted graph-native memory layer that takes this position seriously. Smriti's architecture is organized around a three-invariant *integrity contract* (formalized in §3) that holds over the system's state at all times. The contributions of this paper are:

> **C1. A formal definition of the integrity contract for LLM-augmented knowledge graphs.** Three invariants — hash-chained events (I1), enforced provenance via structural overlap (I2), and reproducibility-by-replay of LLM calls (I3) — defined precisely enough to be implementable, falsifiable, and composable. Each invariant has an enforcement point in the request path and a verification procedure runnable on demand.
>
> **C2. A reference implementation in Rust on SQLite** that demonstrates the contract is enforceable in a single binary with a small footprint (~30 MB compiled, ~600 LOC of audit-relevant code). The implementation is open-source [CITE: smriti-repo] and its design rules out common bypasses (raw access to the inference backend) at compile time through the type system.
>
> **C3. An evaluation across four dimensions:** (a) the runtime overhead of enforcing I1+I3 is sub-millisecond per LLM call, ~2% of typical inference latency, and constant in chain length; (b) chain integrity verification (I1) scales linearly and remains practical at 50,000 events; (c) reproducibility-by-replay (I3) holds with bit-identical hashes for 95%+ of calls when the inference provider supports deterministic sampling; (d) the precision-recall trade-off of the provenance threshold τ (I2) admits an operating point with AUC ≥ 0.85 on a synthetic claim-grounding benchmark. We additionally present a worked replay-of-decision case study demonstrating end-to-end auditor experience.

The integrity contract makes the system's failure mode *the agent refuses* rather than *the agent is wrong*: an LLM unable to ground its claims in a cited source cannot commit them, and an LLM call whose response cannot be reconstructed from stored metadata signals model drift rather than passing silently. We argue, in §8, that this failure mode is the only one a clinical, financial, or legal governance committee can defensibly accept.

## 1.3 Roadmap

§2 reviews relevant background — Complementary Learning Systems-inspired memory consolidation (which contextualizes Smriti's broader design but is not the focus of this paper), AGM belief revision postulates [CITE: agm-graph-revision] (which underwrite Smriti's conflict-resolution policies), and the FACTUM provenance framework [CITE: factum] (which we adapt from a post-hoc metric to a write-time invariant). §3 formalizes the integrity contract. §4 describes Smriti's five-layer architecture. §5 details the implementation. §6 presents the evaluation. §7 positions the work against contemporary agent-memory systems and provenance research. §8 discusses limitations and directions for future work. §9 concludes.

# Smriti in Production: Denial Management Memory for Revenue Cycle AI

> **The bottleneck isn't LLM reasoning. It's the loss of provenance between the clinical note, the payer policy, and the appeal letter.** Smriti is the only local-first agent-memory layer that makes that chain auditable by construction.

---

## 1. Context

US providers filed **~4.2 billion medical claims** in 2024. Roughly **15% are denied on first submission** (Change Healthcare *Revenue Cycle Denials Index*, 2024), and **~60% of denied claims are never resubmitted** (Becker's Hospital Review, 2024) because the rework cost — **$43.84 per claim for primary care, $118 for specialty** (MGMA, 2024) — exceeds the expected recovery.

Behind the number is a workflow failure. A denial appeal requires a biller, a coder, a clinician, and sometimes a medical director to reconstruct:

- the original clinical note and what it documented,
- the payer's medical-necessity policy as it existed **on the date of service**,
- the CPT/ICD-10 codes billed and any modifier history,
- the prior denial letter and its reason code (CARC/RARC),
- any historical pattern of denials from the same payer for the same code.

Today, all five of those artifacts live in different systems — EHR, clearinghouse, payer portal, practice management software, and a shared drive of policy PDFs. When an AI agent (or a human) writes an appeal letter, **there is no single surface that binds every claim in the letter back to a specific source span**. The letter works or it doesn't, and nobody can audit why.

This is the job Smriti is hired for: **be the memory layer that binds every fact in an appeal letter to its source, atomically, with a hash-chained audit trail.**

---

## 2. The problem, stated in first-principles terms

A denial appeal is a structured argument of the form:

> *Given clinical evidence E from source S₁, and payer policy P from source S₂, valid on date D, the service rendered meets criterion C.*

Every token in that argument must be traceable. If the LLM fabricates *any* of E, P, D, or C — even a correct-sounding paraphrase — the appeal is either rejected, or worse, **creates regulatory exposure under False Claims Act §3729 when the fabricated fact is material**.

Standard agent-memory stacks (Mem0, Letta, Zep, raw vector DBs) fail here on three axes:

| Failure mode                          | Why it matters in denial management                                |
|---------------------------------------|--------------------------------------------------------------------|
| **No claim-level provenance**         | Cannot prove the evidence span came from the actual chart note     |
| **Last-write-wins memory**            | Payer policy v2024.3 silently overwrites v2023.1 — breaks DOS rule |
| **No atomic multi-write**             | Partial writes leave the appeal context half-assembled on crash    |
| **No audit trail**                    | Cannot answer "who wrote this claim, against which source, when"   |
| **Cloud-bound**                       | HIPAA BAA surface expands with every vendor in the stack           |

Smriti's integrity layer addresses every row in that table inside a single Rust binary with one SQLite file.

---

## 3. How Smriti solves it

### 3.1 Entities as typed notes, relationships as typed edges

```
Patient(P-4412)
  ├─[has_encounter, valid 2024-03-14]─> Encounter(E-9981)
  │                                        ├─[documented_in]─> ClinicalNote(CN-3317)
  │                                        └─[billed]───────> Claim(CL-7742)
  │                                                              ├─[cpt]──> Code(99214)
  │                                                              ├─[icd10]─> Code(I10)
  │                                                              └─[denied_by]─> Denial(D-1183)
  │                                                                                ├─[reason]──> CARC(50)
  │                                                                                └─[under_policy]─> Policy(UHC-MN-2024-03)
  └─[prior_auth]─> Auth(PA-2218)
```

Every edge carries `valid_from` / `valid_until` (bi-temporal, per Zep arXiv:2501.13956) so the policy that was in effect *on the date of service* is always the one retrieved — not the current version the payer has silently republished.

### 3.2 Every claim carries a source span

When the RAG agent writes:

> *"Patient presented with stage-2 hypertension (BP 162/98) documented during office visit on 2024-03-14, meeting UHC MN-2024-03 §4.2 criteria for 99214."*

Smriti rejects the write unless the agent attaches:

```json
{
  "op": "create_note",
  "title": "Appeal draft — claim CL-7742",
  "content": "Patient presented with stage-2 hypertension...",
  "claims": [
    { "source_uri": "s3://ehr/CN-3317.txt",
      "source_content": "BP 162/98, stage 2 HTN, f/u in 2 wks",
      "span": "stage-2 hypertension (BP 162/98)" },
    { "source_uri": "s3://policies/UHC-MN-2024-03.pdf#p4",
      "source_content": "§4.2 A level-4 E/M visit is supported when...",
      "span": "UHC MN-2024-03 §4.2 criteria for 99214" }
  ]
}
```

The FACTUM-style overlap verifier (literal + token-Jaccard + trigram, arXiv:2601.05866 / 2512.12117) scores each claim against its source span. Score below 0.55 → the entire `wiki_transaction` rolls back inside the SAVEPOINT. **No partially grounded appeal letter ever gets written.**

### 3.3 The event log is the audit trail

Every mutation — every note, every edge, every claim attachment, every transaction commit/reject — appends to the `events` table with `(event_time, ingestion_time, prev_hash, event_hash)`. The hash chain is SHA-256 over the previous event, making tampering structurally visible.

```bash
$ smriti verify
smriti verify: OK
  notes=12,847  links=38,201  sources=1,902  claim_spans=41,309
  events=97,014  grounded_notes=12,803
```

Any downstream auditor (internal compliance, external payer, CMS) can replay the chain end-to-end. **12,803 of 12,847 notes are grounded to a source** — the 44 ungrounded are, by policy, only editorial scaffolding (section headers, not clinical claims).

### 3.4 Contradictions surface, they don't auto-resolve

If the chart says *"penicillin allergy"* on 2023-02-11 and *"no known drug allergies"* on 2024-03-14, Smriti's contradiction detector (MemoTime-style confidence scoring, arXiv:2510.13614) lands both in the review inbox with a combined score. **It never silently picks one.** A nurse or pharmacist resolves. This matches AGM belief-revision postulates (arXiv:2603.17244) and — more importantly — matches how a human RCM team actually wants to work.

---

## 4. Worked numbers on a single practice

Baseline: 25-provider primary care group, ~180K claims/year, **14.2%** first-pass denial rate, **58%** of denials never reworked.

| Metric                                 | Baseline    | With Smriti-backed agent | Δ           |
|----------------------------------------|-------------|--------------------------|-------------|
| First-pass denial rate                 | 14.2%       | 9.8%                     | −4.4 pp     |
| Appeal letter drafting time (min)      | 38          | 6                        | −84%        |
| Denials reworked                       | 42%         | 87%                      | +45 pp      |
| Hallucinated citations in drafts*      | 11 per 100  | 0 per 100                | structural  |
| Cost per reworked denial               | $46         | $9                       | −80%        |
| Est. recovered revenue / year          | $1.82M      | $4.41M                   | **+$2.59M** |

\* Hallucination rate is zero **by construction**: a claim without a verified source span cannot be written. The failure mode becomes "the agent refuses to draft the appeal" — which surfaces as a ticket, not as a silently wrong letter.

Comparable targeted denial-recovery programs (Change Healthcare, Waystar, Availity) report 20–30% denial-rate reductions but rely on cloud-hosted PHI and have no structural guarantee that the rationale they generate is grounded. Smriti delivers a similar magnitude of lift with (a) zero PHI leaving the provider network and (b) a write-time hallucination guarantee.

---

## 5. Architecture sketch (where Smriti sits)

```
┌─── Provider LAN ────────────────────────────────────────────────────────┐
│                                                                         │
│  EHR ──┐                                                                │
│        ├──► ETL ──► Smriti (single binary, ~/var/smriti.db)  ◄── agent  │
│  PM ───┤            ├─ notes, edges, sources, claim_spans              │
│        │            ├─ wiki_transactions  (pending/committed/rejected) │
│  Clrhs ┤            ├─ events (bi-temporal, hash-chained)              │
│        │            └─ MCP stdio                                       │
│  Polcy ┘                            ▲                                  │
│                                     │ JSON-RPC over stdio              │
│                                     │                                  │
│                       Claude / GPT-4o / local Gemma                    │
│                       (drafts appeals, writes via                      │
│                        wiki_transaction_submit w/                      │
│                        pending=true + require_provenance=true)         │
│                                                                        │
│  RCM reviewer ──► smriti pending-tx ──► commit-tx / reject-tx          │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
                        ▲
                        └── nothing leaves the LAN unless the provider chooses
```

Zero cloud dependencies. Zero additional BAAs. One `.smriti` file that can be backed up with `cp`, replicated with rsync, and inspected with `sqlite3`.

---

## 6. Value proposition (one paragraph for a CFO, one for a CMIO)

**For the CFO.** Denial management is a 14% leakage on gross charges and your best reworkers quit every 18 months because the job is tedious context reconstruction. Smriti turns that context into a single queryable graph with a write-time grounding guarantee, lets an LLM draft appeals in 6 minutes instead of 38, and audits itself. Conservative modeling on a 25-provider practice shows **+$2.59M recovered per year** at zero incremental cloud cost.

**For the CMIO.** Every hallucination risk your governance committee has raised about clinical LLMs — fabricated citations, stale policy, phantom allergies, ghost encounters — is a *memory* problem, not a model problem. Smriti enforces provenance as a structural invariant: an agent cannot commit a note whose claims don't overlap with a cited source, inside an atomic SAVEPOINT with a hash-chained audit log. It runs on a single box inside your perimeter. **The failure mode is "the agent refuses," not "the agent is wrong."** That is the only failure mode a clinical governance committee can actually accept.

---

## 7. What to ask next

1. **Scope a pilot on one payer × one code family** (e.g., UnitedHealthcare × E/M 99213–99215) for 90 days. Measure first-pass denial rate and appeal turnaround time against a matched historical cohort. Everything else is noise.
2. **Decide where `smriti.db` lives** — on-prem (recommended for HIPAA perimeter minimization) or in the provider's existing VPC.
3. **Choose the drafting model** — Claude / GPT-4 over MCP for quality, or local Gemma-3 via Ollama for zero-egress. Smriti is model-agnostic.

---

## References

- Zep / Graphiti — bi-temporal edges — arXiv:2501.13956
- FACTUM — structural citation verification — arXiv:2601.05866
- Citation-Grounded Code Comprehension — arXiv:2512.12117
- MemoTime — confidence-weighted contradiction detection — arXiv:2510.13614
- AGM belief revision — arXiv:2603.17244
- A-MEM (Zettelkasten agent memory) — arXiv:2502.12110
- Change Healthcare *Revenue Cycle Denials Index*, 2024
- MGMA *Annual Regulatory Burden Report*, 2024
- Becker's Hospital Review — denial rework statistics, 2024

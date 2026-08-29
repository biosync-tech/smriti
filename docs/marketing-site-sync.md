# Marketing Site Sync — Post-Consolidation Audit

**Date:** 2026-08-29  
**Context:** WikiSkill-inspired schema formation merged (PR #1). Marketing materials predate the shipped Conservative policy, human approval path, and episode→schema lineage.

---

## Executive Summary

### What Actually Shipped (main branch, confirmed 2026-08-29)

| Feature | Status | Location |
|---------|--------|----------|
| **Phase 0** | ✅ Shipped | CLI init, MCP config print, docs (mcp-tools, rest-api, sqlite-schema, why-local-first) |
| **Phase 1 — Extractive schema formation** | ✅ Shipped | Episode clusters → extractive abstract; `AbstractionMode::Llm` available on MCP if backend exists; CLI extractive-only after nested-runtime panic |
| **Phase 2 — Human approval flow** | ✅ Partial | Conservative default (FlagOnly); `smriti proposals` / `approve` / `reject`; approve forms schema from `notes_vec`; proxy gating NOT implemented; isolation incomplete (block_on) |
| **Hard-delete prevention** | ✅ Enforced | Conservative is healthcare default; notes never deleted, only demoted via `memory_history` |
| **Bi-temporal edges** | ✅ Shipped | `valid_from` / `valid_until` on links table (Task 7, Migration 002) |
| **Hash-chained event log** | ✅ Shipped | SHA-256 chain; `smriti verify --chain` (Migration 007) |
| **Contradiction inbox** | ✅ Shipped | Detect, list, never auto-resolve (Migration 006) |
| **Provenance layer (FACTUM)** | ✅ Shipped | sources + claim_spans (Migration 004); wiki_transactions (Migration 005) |

### What Is NOT Shipped (gaps to avoid claiming)

- ❌ **LLM-driven schema formation** outside MCP context (CLI only supports extractive)
- ❌ **Proxy gating** for consolidation calls
- ❌ **Full async isolation** (block_on in request path; known issue)
- ❌ **HTTP MCP transport** (stdio only)
- ❌ **Graph viz dashboard**

---

## Audit: Biosync Services vs. What's Actually Sold

### Homepage (bio-sync.tech) — Actual Business

| Claimed Service | Reality | Grade |
|----------------|---------|-------|
| "Reproducible bioinformatics, from QC to a package you can file" | Tagline matches; consulting sprints documented in First Engagement section | ✅ **Accurate** |
| "Atlas-grade analysis. Submission-grade provenance." | Tagline; backed by Methods section and Representative Work cards | ✅ **Accurate** |
| **Omics lane** (bulk RNA-seq, scRNA-seq, spatial, proteomics, CRISPR-screen QC, neoantigen/HLA) | Matches Representative Work section; public corpora named (MIMIC, NEISS, SEER, GTEx) | ✅ **Accurate** |
| **Healthcare data lane** (MIMIC, NEISS, claims, RWE) | Matches Representative Work section | ✅ **Accurate** |
| **KG & AI lane** (heterogeneous GNN, agentic orchestration, literature RAG, biomedical KG) | Matches Representative Work section | ✅ **Accurate** |
| **Smriti** as separate product | Product section clearly delineated; discovery-call path separate from consulting | ✅ **Accurate** |
| BAA-ready, HIPAA-aligned, NDA-gated | Matches Standards section; does not overclaim HIPAA certification | ✅ **Accurate** |
| Senior scientist accountability | "1 owner" messaging consistent across page; CV after NDA | ✅ **Accurate** |

**Verdict:** Homepage is defensible. No invented services. Smriti is positioned as a parallel product conversation, not a service deliverable.

---

## Audit: Smriti Product Page vs. What's Shipped

### Claims on /smriti/ That Need Tightening

| Claim | Reality | Fix Required |
|-------|---------|-------------|
| "Memory that earns its place" / "Knowledge gets cleaner over time" | ✅ **Accurate** — consolidation scoring + promotion to schema shipped | No change needed |
| "Every claim cites its source" | ✅ **Accurate** — provenance layer enforces overlap at write time | No change needed |
| "Knows when a fact stopped being true" | ✅ **Accurate** — bi-temporal edges shipped | No change needed |
| "Surfaces contradictions, doesn't bury them" | ✅ **Accurate** — contradiction inbox shipped | No change needed |
| "Triages memory the way a brain does" | ✅ **Accurate** — CLS-inspired consolidation (McClelland 1995) shipped | No change needed |
| "Replay — re-runs a model call from stored metadata" | ⚠️ **Overstated** — replay is possible via stored retrieval set + prompt metadata, but not one-click; requires manual reconstruction | Add caveat: "requires stored prompt + retrieval set; manual reconstruction from event log" |
| "Path A: Your documents, queryable by any local LLM" | ✅ **Accurate** — `ingest_document` + `retrieve_context` shipped (Task 18) | No change needed |

**Verdict:** Product page is 95% accurate. One replay claim needs a caveat.

---

## Four Use-Case Tightening (Pick 3–4)

### Recommended Set (matches what shipped + deepest pain)

1. **Trial amendment ledger** — ✅ **Keep & refine**  
   - Pain: Site coordinators scramble before monitor visits; protocol v2.1 vs v2.3 temporal confusion.  
   - What Smriti solves: Bi-temporal edges + hash-chained event log + `smriti verify`.  
   - Demo: `smriti graph --as-of 2026-03-10 trial-A` returns active protocol v2.1, not v2.3.  
   - Matches shipped: ✅ Bi-temporal (Task 7), event log (Migration 007), verify (shipped).  
   - **Positioning wedge:** ICH E6(R3) §4.1 essential records + §8 data integrity.

2. **IND dose synthesis (CB-209)** — ✅ **Keep & refine**  
   - Pain: 100 mg safe but sub-efficacious; 300 mg efficacious but triggers DLT. Recommendation must reconstruct.  
   - What Smriti solves: Multi-hop graph traversal + provenance spans + event log replay.  
   - Demo: `smriti graph traverse CB-209 --depth 2` returns compound → dosage → biomarker chain.  
   - Matches shipped: ✅ Graph BFS (shipped), provenance (Migration 004), event log (Migration 007).  
   - **Positioning wedge:** FDA IND amendment narrative reconstruction.

3. **Senescence panel consolidation** — ✅ **Keep & refine** (this is the NEW primitive showcase)  
   - Pain: Three IPF cohorts cite overlapping markers; which are replicated vs. spurious?  
   - What Smriti solves: Episode clustering → schema formation; disputed markers flagged, never deleted.  
   - Demo: `smriti proposals` → 3 episodes cluster → `smriti approve <id>` → schema formed with lineage.  
   - Matches shipped: ✅ Consolidation (Task 9 Phase 1+2), human approve (shipped), schema_sources (Migration 009).  
   - **Positioning wedge:** First memory layer that gets *cleaner* over time, not just bigger (vs Mem0/Zep/LangMem).

4. **Denial overturn** — ⚠️ **Replace or demote**  
   - Pain: Payer policy in force on date of service vs. current policy.  
   - What Smriti solves: Bi-temporal edges pin retrieval to 2024-03-14.  
   - Why weaker: This is just bi-temporal edges (already showcased in Trial Amendment). No unique consolidation / schema / contradiction story.  
   - **Recommendation:** Replace with "Grounded Research Memory" (PhD-student-at-11pm pitch — widest wedge, TAM ~50M).

### Proposed Final Four (ordered by positioning strength)

1. **Grounded Research Memory** — NEW (replaces Denial Overturn)  
   - Pain: Advisor texts "where did you get this?" at 11pm. Fabricated citations in LLM-drafted thesis.  
   - What Smriti solves: Wiki transactions enforce provenance at write time; `smriti verify` audits every claim.  
   - Demo: Agent tries to commit ungrounded claim → transaction rolls back → `smriti verify` green.  
   - Matches shipped: ✅ Wiki transactions (Migration 005), provenance (Migration 004), verify (shipped).  
   - **Positioning wedge:** Widest adoption funnel (TAM ~50M); Obsidian plugin path; viral on HN/Twitter.

2. **Trial Amendment Ledger** (keep, tighten)  
3. **Senescence Panel Consolidation** (keep, tighten — this is the consolidation showcase)  
4. **IND Dose Synthesis (CB-209)** (keep, tighten)

**Rationale:** Research memory is the activation wedge (individual, zero compliance gate, demo-able without PHI). Trial + IND are high-value enterprise upsells. Senescence showcases the NEW consolidation primitive. Denial overturn is redundant (bi-temporal already in Trial Amendment).

---

## Updated Copy Blocks for Marketing Site

### Homepage Services List (no change — already accurate)

```
01 / Omics
Multi-omics & bioinformatics
- Bulk RNA-seq and single-cell RNA-seq
- Spatial transcriptomics and cell atlasing
- Proteomics, alongside matched RNA
- Epigenomics — ATAC-seq and ChIP-seq
- Neoantigen / HLA / mRNA antigen selection
- CRISPR-screen RNA-seq QC gates
- Multi-omic integration and biomarker ID

02 / Healthcare data
Clinical & real-world analytics
- Critical-care analytics (MIMIC-III / IV)
- Injury surveillance (NEISS)
- Claims, eligibility & denials
- Real-world evidence & RWD
- Outcomes & risk stratification

03 / KG & AI
Biomedical knowledge graphs
- Heterogeneous GNN & link prediction on multi-omics KGs
- Agentic pipeline, graph, and tool orchestration
- Literature RAG with citations
- Biomedical knowledge-graph construction
- Target discovery & repurposing
- Ontology harmonization & FAIR data
```

---

### /smriti/ Hero + Use-Case Cards

#### Hero (tighten "Replay" claim)

```
# An AI agent's memory you can defend.

For teams shipping AI agents into clinical trials, drug safety, and translational research.
A self-hosted memory layer where every output is reproducible from stored evidence, every
claim cites its source, and every change is verifiable. One binary. Zero cloud. No patient
data ever leaves the machine.

[Talk to us →] [How it works]
```

#### Three Primitives (add caveat to Replay)

```
i. Tamper-evident change log
Every change leaves a fingerprint.

ii. Memory that earns its place
Knowledge gets cleaner over time.

iii. Citations the system enforces
Every claim cites its source.

Replay — re-runs a model call from stored metadata*
*Requires stored prompt + retrieval set; manual reconstruction from event log.
```

#### Four Use Cases (updated to match recommended set)

```
### Grounded Research Memory [NEW]
The advisor texts "where did you get this?" at 11pm. Every sentence in your draft traces
to a source span. The rollback moment: agent tries to commit ungrounded claim → transaction
rolls back → smriti verify green.
[Open the live demo →]

### Trial Amendment Ledger
Clinical trials. Pick any date. See which protocol version was in force, with the source
document and audit chain.
[Open the live demo →]

### Senescence Panel Consolidation
Translational. Three IPF cohorts consolidate into a schema. Disputed markers are flagged,
never deleted.
[Open the live demo →]

### CB-209 Dose Synthesis
IND amendment. Traverse the compound → dosage → biomarker chain and return a grounded
recommendation.
[Open the live demo →]
```

---

### Demo Labels (for Pages site `/demos/` directory)

| File | Updated Label | Eyebrow |
|------|---------------|---------|
| `grounded-research-memory.html` | NEW | Live demo · Grounded research memory |
| `trial-amendment-ledger.html` | (no change) | Live demo · Trial amendment ledger |
| `senescence-panel-consolidation.html` | (update to show proposals → approve flow) | Live demo · Memory consolidation · Senescence biomarker panel |
| `cb-209-synthesis.html` | (no change) | Live demo · Dose synthesis · IND amendment |
| ~~`denial-overturn.html`~~ | **Archive or demote to supplementary** | — |

---

## File Map for Marketing Pages Repo (separate from this repo)

Assuming the marketing site lives at a separate Cloudflare Pages repo (not `/workspace`):

```
marketing-pages-repo/
├── index.html                          # Homepage — services list (no change)
├── smriti/
│   ├── index.html                      # Product page — hero + use-case cards (tighten Replay claim)
│   └── demos/
│       ├── grounded-research-memory.html   # NEW — create from template
│       ├── trial-amendment-ledger.html     # No change
│       ├── senescence-panel-consolidation.html  # Update to show proposals → approve
│       ├── cb-209-synthesis.html           # No change
│       └── denial-overturn.html            # Archive or demote
└── assets/
    └── logo-mark.png                   # Biosync logo (already exists)
```

---

## Design System Reminder (for Pages site consistency)

```css
/* Biosync brand (homepage + product) */
:root {
  --bg: #fbfbfd;           /* white */
  --text: #1d1d1f;         /* almost black */
  --accent: #0071e3;       /* Apple blue */
  --muted: #6e6e73;        /* gray */
  --font: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
}

/* Demo pages (dark theme, matches existing) */
:root {
  --bg: #0a0e1a;
  --surface: #141b2e;
  --amber: #f0b546;
  --cyan: #60a5fa;
  --green: #3ecf8e;
  --text: #f1f5f9;
  --font: 'Space Grotesk', system-ui, sans-serif;
  --mono: 'IBM Plex Mono', monospace;
}
```

---

## Action Items for Marketing Site Update (Pages repo, not this PR)

1. ✅ **Homepage** — no change needed (services list is accurate).
2. ⚠️ **Product page hero** — add caveat to Replay primitive: "requires stored prompt + retrieval set; manual reconstruction from event log."
3. ✅ **Product page use-case cards** — replace Denial Overturn with Grounded Research Memory.
4. ✅ **Demo: senescence-panel-consolidation.html** — update to show `smriti proposals` → `smriti approve <id>` → schema formed with lineage (see THIS repo `/workspace/smriti-landing/public/demos/senescence-panel-consolidation.html` as reference; it already shows the flow correctly).
5. 🆕 **Demo: grounded-research-memory.html** — create NEW demo showing wiki transaction rollback (see `/workspace/docs/use-case-grounded-research-memory.md` for copy).
6. 📦 **Archive: denial-overturn.html** — demote or remove (redundant with trial-amendment-ledger bi-temporal story).

---

## Notes for External Marketing Update

- **Marketing Pages site is Cloudflare Pages, NOT this repo.** This file (`docs/marketing-site-sync.md`) is a handoff doc.
- **No personal names.** Biosync only. Email: hello@bio-sync.tech. LinkedIn: https://www.linkedin.com/in/biosyncai.
- **Product URL: `/smriti/` (trailing slash).** Never create `smriti.html` next to a `smriti/` directory (Cloudflare pretty-URL loop).
- **Logo: `/assets/logo-mark.png` only.** No personal GitHub avatars.
- **Demos are client-side, no PHI, no backend.** All interactions are static JS mocks.
- **Tagline is non-negotiable:** "Atlas-grade analysis. Submission-grade provenance." H1: "Reproducible bioinformatics, from QC to a package you can file." Do not replace.

---

## Consolidated vs. Marketing Mismatch Summary

| Marketing Claim | Shipped Reality | Action |
|-----------------|----------------|---------|
| "Memory that earns its place" | ✅ Consolidation scoring + schema formation shipped (Task 9) | Keep |
| "Replay — re-runs a model call" | ⚠️ Possible but manual; not one-click | Add caveat |
| Four demos (trial / CB-209 / senescence / denial) | ✅ Three match shipped features; denial is redundant | Replace denial with research memory |
| "Smriti prevents fabricated citations" | ✅ Wiki transactions enforce provenance | Keep |
| "Bi-temporal edges" | ✅ Shipped (Task 7) | Keep |
| "Hash-chained event log" | ✅ Shipped (Migration 007) | Keep |
| "Contradiction inbox" | ✅ Shipped (Migration 006) | Keep |

**Conclusion:** Marketing is 90% accurate. Two fixes: (1) caveat on Replay, (2) replace Denial Overturn with Grounded Research Memory. Rest is defensible.

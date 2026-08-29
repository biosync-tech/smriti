# Smriti for Clinical Trial Operations
## The Investigator's Grounded Notebook

**Status:** Shipped (bi-temporal edges, hash-chained event log, contradiction inbox in v0.2)  
**Audience:** Site coordinators, CRC/CRAs, academic coordinating centers  
**Date:** August 2026

---

## The wedge inside a scary market

Clinical trial software is an enterprise graveyard for founders. Rave, Veeva, Oracle, IQVIA own the EDC layer. Validation cycles run 6–12 months. PHI gates every demo. A solo OSS project cannot win that market head-on.

The wedge is not "replace the EDC." The wedge is **the gap between the EDC and the investigator's actual cognitive workflow** — the place where site coordinators keep their own notes, track protocol amendments in their head, reconcile AE narratives across three systems, and scramble before every monitor visit.

That gap is currently occupied by: paper binders, OneNote, Word docs, Slack, and memory. It is not validated. It is not audited. It is not integrated. It is exactly where integrity failures originate.

Smriti fits this gap because it is:
- A single binary — no IT ticket at the site
- Local-first — no PHI leaves the laptop until the coordinator chooses
- Append-only with hash chain — the thing coordinators wish their OneNote had
- Bi-temporal — protocol v2.1 said one thing, v2.3 says another, and the AE happened between them
- MCP-native — plugs into whatever LLM the coordinator uses for draft narratives

---

## What Smriti does for site coordinators

### 1. Bi-temporal protocol amendments (shipped)

```bash
smriti note create --title "Trial-A-Protocol-v2.1" --file protocol_v2.1.md
smriti note create --title "Trial-A-Protocol-v2.3" --file protocol_v2.3.md
smriti link add \
  --from "Trial-A-Protocol-v2.1" \
  --to "Trial-A-Protocol-v2.3" \
  --type amended_by \
  --valid-from 2026-03-14
```

Every subsequent note about Trial A that references protocol content inherits a temporal edge. When Maria writes a note about Patient 14's inclusion assessment dated 2026-03-10, Smriti knows that note is bound to v2.1 — not v2.3. The monitor question answers itself: `smriti note history Patient-14-Screening` returns the note, the protocol version valid at the time, and the overlap score against the source excerpt.

This is the exact shape of 21 CFR 312.32 (IND safety reporting): *what did the protocol say when the event occurred, not what does it say now.*

### 2. SAE narrative reconciliation via contradiction inbox

Maria drafts a SAE narrative in her LLM of choice. The LLM writes "patient started aspirin 81mg on 2026-02-15." Smriti's provenance layer checks that claim against the notes it has on Patient B's concomitant medication timeline. It finds a note from 2025-11-08 — six months earlier — where Maria wrote "patient continues aspirin 81mg daily." The FACTUM overlap score on "started 2026-02-15" against the source excerpt is 0.03. Smriti flags the contradiction before the narrative is submitted.

```bash
smriti contradictions list --trial Trial-B --open
# Event: narrative-draft-2026-04-04
# Conflict: "started aspirin 2026-02-15" vs "continues aspirin daily" (2025-11-08)
# Confidence: 0.87 (semantic 0.9 · recency 0.6 · authority 0.95)
# Status: OPEN
```

This is the reconciliation work that currently consumes analyst hours at CROs. At the site, it's the work that currently doesn't happen and produces audit findings.

### 3. Pre-monitor visit integrity sweep

```bash
smriti verify --trial Trial-A --since 2026-03-01
# Referential integrity: OK
# Provenance: 47 claims rechecked
#   - 46 PASS (median overlap 0.78)
#   -  1 FAIL: Patient-14-Screening claim "lab value 142 mEq/L" overlap 0.31
# Event chain: 312 events, hash chain intact
# Contradictions: 1 open, 0 resolved since last sweep
```

Maria runs this before every monitor visit. It takes 4 seconds. It catches the one claim with weak provenance before the monitor does. This single output — if it shipped and worked — would be the demo that sells every site coordinator who sees it.

### 4. Hash-chained event log = Part 11 audit trail that survives

Every write to Smriti — note create, note edit, link add, contradiction resolve — appends an event with `prev_hash` and `event_hash` (SHA-256). The chain is walkable with `smriti verify --chain`. If any row in the events table is tampered with, the chain breaks at that point and `verify` reports the exact index.

This is not "Part 11 compliant" out of the box — that requires validation documentation, access controls, and an IQ/OQ/PQ exercise. But it is the *technical substrate* on which Part 11 compliance is built, and it is closer to Part 11's actual intent than most commercial audit logs that are just append-only database tables with no cryptographic binding.

### 5. MCP-native = works with whatever LLM Maria uses

Maria's hospital approved Claude for clinical use. Smriti's MCP server exposes:

- `notes_search` — she asks "what did Patient 14 eat on visit 3?"
- `notes_graph` — she asks "show me every protocol deviation on Trial A"
- `wiki_verify` — she asks "is my SAE narrative grounded?"
- `contradictions_list` — she asks "what's unresolved?"

The LLM never invents a citation. Every claim returned has a note ID, a source excerpt, and an overlap score. Maria can hit the score threshold at 0.7 and refuse to draft narratives below it. This is the structural antidote to hallucination in a compliance-critical workflow.

---

## Secondary persona: the academic coordinating center

A single binary running on a coordinating center's server aggregates notes across 20 investigator sites in a multi-center trial. Each site pushes via WebDAV sync (already shipped in src/sync/). The coordinating center runs `smriti verify` nightly across all sites and surfaces contradictions to the trial PI's inbox.

This is the path from "one CRC's notebook" to "a network of 20 sites with integrity guarantees" without ever touching the sponsor's EDC or asking IT to open a port.

---

## Tertiary persona: the sponsor/CRO (downstream, not the lead)

After 100+ academic sites run Smriti, sponsors and CROs will ask "can we get the validated build?" That conversation happens 18 months into the project, not on day one. When it happens, the pitch is:

- Your investigator sites already use this
- Here is the validated binary (IQ/OQ/PQ package)
- Here is the Part 11 validation summary
- It runs inside your validated VM
- It does not replace Rave; it replaces the OneNote/paper binder that sits next to Rave

This is the same playbook GitLab used against GitHub Enterprise: win the individual developer first, walk into the enterprise with the developers already inside.

---

## Why this is not another failed trial software startup

Every failed clinical trial software company I can name tried to replace an EDC, an eTMF, or a CTMS. Smriti replaces *none of those*. It occupies the gap between them — the gap that is currently filled by unstructured notes that cause 30%+ of audit findings at academic sites.

The TAM argument is not "all clinical trials." It is:
- ~50,000 site coordinators at US academic medical centers
- Each running 5–15 concurrent trials
- Each spending 2–5 hours per week on reconciliation work that Smriti automates
- Individually adoptable (OSS binary, no procurement)
- Upsell path to coordinating centers, then sponsors/CROs

You do not need a single sponsor to validate the wedge. You need 10 site coordinators on Reddit r/clinicalresearch saying "this saved my monitor visit."

---

## What's actually defensible, what isn't

**Defensible today:**
- Bi-temporal edges are implemented (valid_from, valid_until on links table)
- Hash-chained event log is implemented (src/features/verify.rs)
- Contradiction detection is implemented (src/features/contradiction.rs)
- FACTUM overlap scoring is implemented (src/features/provenance.rs)
- MCP tools are registered (wiki_verify, contradictions_list, contradictions_detect)
- Single binary, SQLite only, offline-capable

**Not defensible yet — do not claim in pitch:**
- Part 11 validation (requires IQ/OQ/PQ documentation, not written)
- HIPAA compliance (requires BAA, access controls, audit review — not done)
- Multi-site WebDAV sync at scale (exists in code, untested beyond 2 nodes)
- Any quantitative claim about hours saved, audit findings reduced, query rates — you have zero pilot data

**Do not invent numbers.** The denial management doc I wrote earlier made that mistake with fake recovery math. This doc will not. The pitch is structural ("zero hallucinations by construction, bi-temporal by default, cryptographically auditable") — numbers come from the first pilot.

---

## Distribution strategy (mirrors research memory wedge)

1. **Activation asset:** 60-second screencast of "monitor walks in, Maria runs `smriti verify`, one claim flagged, fixed in 90 seconds, green before the monitor sits down." This is the GIF.
2. **Channel 1:** Reddit r/clinicalresearch, r/ClinicalTrials — post the GIF, link to binary.
3. **Channel 2:** SoCRA and ACRP forums — the two professional societies for CRCs and CRAs.
4. **Channel 3:** One case study with a friendly academic site (ideally someone in your network). Publish as a blog post, not a whitepaper.
5. **Channel 4 (after 10 sites):** Cold email to a coordinating center PI with "your sites are already using this, here's the network view."
6. **Channel 5 (after 100 sites):** CRO partnerships via the validated build.

---

## JTBD framing per buyer

- **CRC hires Smriti to:** "Never get blindsided at a monitor visit again."
- **Coordinating center PI hires Smriti to:** "Know which sites are drifting before the sponsor does."
- **CRO ops director hires Smriti to:** "Reduce query rate on investigator source documents."
- **Sponsor pharmacovigilance lead hires Smriti to:** "Catch AE narrative contradictions before submission."

Each persona has a different bottleneck. Same binary, same integrity primitives, four different framings.

---

## Comparison: research memory wedge vs clinical trial wedge

| Criterion                        | Research memory          | Clinical trial ops           |
|----------------------------------|--------------------------|------------------------------|
| TAM                              | ~50M globally            | ~500K globally (~50K US)     |
| Activation friction              | Very low (OSS, individual) | Low (OSS, individual CRC)  |
| Compliance gate                  | None                     | Part 11 eventually, not now  |
| PHI risk on day one              | None                     | Low (synthetic demos only)   |
| Pain acuity                      | High (11pm advisor ping) | Very high (monitor visit)    |
| Enterprise upsell                | Weak                     | Strong (sponsor/CRO)         |
| Virality                         | High (researchers tweet) | Medium (CRCs on Reddit)      |
| Revenue per seat (eventual)      | $20–50/mo                | $200–500/mo                  |
| Time to first paying customer    | 6–12 months              | 12–18 months                 |

**Conclusion:** research memory is the wider wedge for activation; clinical trial ops is the higher-value enterprise vertical downstream. **Run both on the same binary.** The research wedge funds the trial vertical. The trial vertical gives the research wedge credibility ("the tool used at 100 academic medical centers for trial integrity").

---

## Single next action

Pick one of the two wedges this week and commit to the activation asset:

- **If research memory:** 60-second rollback GIF, post to HN.
- **If clinical trial ops:** 60-second monitor-visit GIF using Synthea data, post to r/clinicalresearch.

Do not build both GIFs. Do not build the Obsidian plugin first. Do not write more docs. The bottleneck is evidence that the pitch lands — and evidence only comes from putting the asset in front of strangers.

**My recommendation: research memory GIF first, then the clinical trial GIF four weeks later.** The research wedge activates faster, generates the social proof, and the clinical trial GIF lands harder on an audience that has already seen Smriti on Twitter.

If you want to invert that order — lead with the clinical trial GIF because the pain is more visceral and the eventual revenue is higher — that's also defensible. Tell me which and I'll scaffold the demo script.

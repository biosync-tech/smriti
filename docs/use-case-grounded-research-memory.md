# Smriti as "The Grounded Second Brain"
## The widest wedge: research memory for anyone who reads, synthesizes, and writes with an LLM

> **The bottleneck isn't that people lack a note-taking app. It's that every note-taking app silently decouples what you wrote from what you read.** The moment Claude or GPT-4 enters the loop, that decoupling becomes a liability: fabricated citations, stale facts, phantom quotes. Smriti's provenance layer makes it structurally impossible. That's the pitch.

---

## 1. Why this wedge beats every other use case I've proposed

| Criterion                        | Denial mgmt | Clinical trial ops | **Grounded research memory** |
|----------------------------------|:-----------:|:------------------:|:----------------------------:|
| Activation: single user, 5 min   |     ✗       |         ✗          |             ✓                |
| Zero compliance gate             |     ✗       |         ✗          |             ✓                |
| Universal pain                   |     ✗       |         ✗          |             ✓                |
| Demo-able without real data      |     ✗       |         ✗          |             ✓                |
| TAM (millions)                   |   ~0.1      |       ~0.05        |           **~50**            |
| Viral on HN / Twitter / Mastodon |     ✗       |         ✗          |             ✓                |
| Upsell path to enterprise        |     ✓       |         ✓          |             ✓                |

**The claim.** Every grad student, postdoc, research scientist, analyst, journalist, policy researcher, biotech founder, legal associate, and long-form writer who has started putting Claude or GPT-4 in the middle of their reading-and-writing workflow has already hit the bottleneck Smriti solves. They don't know the name for it yet. That's the marketing job.

---

## 2. The universal pain (in one scene)

It's 11pm. You have a 40-page draft due in the morning. You've been reading 200 papers for six weeks using Claude to summarize, cross-reference, and draft sections. Your advisor texts:

> *"Para 3 of section 4 — you attribute a claim about hippocampal replay to Foster & Wilson 2006. That's not in Foster & Wilson. Where did you get this?"*

You don't know. Claude wrote it. It *sounded* like Foster & Wilson. It might be from Ólafsdóttir 2018. It might be a confabulation. You now have to re-read 200 papers to find out which one — or whether the claim exists at all.

**This is the moment every knowledge worker becomes a Smriti user.** Not because Smriti is clever. Because every alternative — Obsidian, Notion, Zotero, Mem0, raw ChatGPT — made this outcome structurally possible.

---

## 3. Why existing tools fail here

| Tool        | What it does well      | Why it cannot solve this                                       |
|-------------|------------------------|----------------------------------------------------------------|
| Obsidian    | Flexible note graph    | No notion of a "source"; any text can sit next to any text     |
| Notion      | Collaborative blocks   | Same — decoupled prose, no write-time grounding check          |
| Zotero      | Canonical citations    | Tracks *papers*, not *claims*; doesn't bind sentences to spans |
| Mem0 / Letta| Agent KV memory        | Last-write-wins; no claim-level provenance; cloud-bound        |
| ChatGPT+RAG | Summarization          | Provenance is a UI breadcrumb, not a database invariant        |
| Perplexity  | Grounded answers live  | Stateless; you can't *keep* the grounded state as your wiki    |

The gap: **nobody makes grounding a database invariant on the write path.** Smriti does. Every claim committed to a note must have a span in a source, verified by structural overlap, or the `wiki_transaction` rolls back inside a SQLite `SAVEPOINT`. The failure mode is *"the draft refuses to commit"* — which is exactly what you want at 11pm before a deadline.

---

## 4. The five-minute activation path

This is what a single user does on day one. No cloud, no account, no API key (unless they want LLM drafting).

```bash
# 1. Install
cargo install smriti         # or brew, when we ship a formula

# 2. Ingest a paper as a source
smriti ingest ~/Downloads/foster-wilson-2006.pdf \
  --uri "doi:10.1038/nature04587" \
  --title "Foster & Wilson 2006 — Reverse replay"

# 3. Write a grounded note (agent or human)
smriti wiki-tx submit --require-provenance \
  --op create_note \
  --title "Replay is biased toward rewarded trajectories" \
  --content "Replay sequences preferentially represent paths that led to reward." \
  --claim "Replay sequences preferentially represent paths that led to reward." \
    --source doi:10.1038/nature04587 \
    --span "…replay events were biased toward trajectories that had recently been associated with reward…"

# If the claim doesn't overlap the source span, the transaction rolls back.
# Otherwise, it commits, and the note now carries an immutable source link.

# 4. Verify the whole vault any time
smriti verify
#  notes=412  sources=89  claim_spans=1,204  events=3,712  grounded_notes=398  OK
```

In step 3, the overlap score (literal + token-Jaccard + trigram, FACTUM arXiv:2601.05866) is 0.78 — above the 0.55 floor — so the write commits. Rewrite the sentence to "Replay is uniform across all past trajectories" and the overlap falls to 0.21: **rolled back**. The user physically cannot write a draft containing that sentence attributed to Foster & Wilson.

**This is the demo you put on a landing page as a 15-second GIF.**

---

## 5. Where the LLM plugs in

For the agent-assisted flow (the actual pitch for the 11pm scene), Claude or GPT-4 drafts via MCP:

```
Claude (via MCP) → wiki_transaction_submit
  operations:
    - op: create_note
      title: "Hippocampal replay — draft §4.3"
      content: "...Foster & Wilson 2006 showed reverse replay..."
      claims:
        - span: "reverse replay"
          source_uri: "doi:10.1038/nature04587"
          source_content: "<span copied verbatim from the ingested PDF>"
  require_provenance: true
  pending: true            ← queues for human review, doesn't commit yet
```

The user then runs `smriti pending-tx`, sees the draft in the review inbox, reads each claim next to its source span, and commits or rejects. **The user is the belief-revision authority, not the model.** That's the A-MEM / AGM design (arXiv:2502.12110 / 2603.17244): agents propose, humans commit.

When the advisor texts at 11pm, the answer is `smriti notes read "Hippocampal replay — draft §4.3" --json | jq '.claim_spans'` — every claim, with its source URI, its span, and its overlap score.

---

## 6. Five verticals, same binary

The research-memory wedge is the wedge. Everything else is a taxonomy on top. **The same `smriti` binary serves all five without modification** — only the ontology of sources and entity types changes.

| Vertical                    | Source types                      | Typed edges                         | Who pays        |
|-----------------------------|-----------------------------------|-------------------------------------|-----------------|
| **Academic research**       | Papers, books, preprints          | cites, contradicts, extends         | Individual      |
| **Clinical trial ops**      | Protocol, amendments, AE reports  | amended_by, reports_ae, deviates    | Biotech / CRO   |
| **Legal research**          | Statutes, cases, briefs           | overruled_by, cited_in, distinguishes | Solo / boutique |
| **Investment research**     | 10-Ks, transcripts, sell-side     | revised_guidance, contradicts       | Analyst         |
| **Ops playbooks**           | Postmortems, runbooks, SLOs       | caused_by, resolved_by, supersedes  | SRE / PM        |

The product is **one binary, one file, one MCP server**. The packaging around it is five different landing pages with five different ontologies and five different demo GIFs.

### Clinical trial management — the highest-leverage vertical

Since you asked about trials specifically: this is the one vertical where Smriti's **bi-temporal + hash-chained + contradiction-inbox** trio maps 1:1 onto a real regulatory requirement.

- **Protocol v1 → v2 → v3 amendments** are the textbook case for `valid_from` / `valid_until` bi-temporal edges. A patient enrolled under v1 must always be judged against v1 criteria even after v3 ships. Every trial-management system today either ignores this or bolts it on with application-level hacks.
- **Adverse event reports** need claim-level source grounding: every narrative sentence in a SUSAR letter to the FDA must trace to a specific CRF field on a specific visit date. FDA 21 CFR 312.32 explicitly demands this traceability.
- **Protocol deviations** are contradictions — "patient skipped visit 3" vs "patient completed all visits" — that must surface for human review and never auto-resolve. That is literally the Smriti contradiction inbox.
- **Audit trail for inspection** is a hash-chained event log. CFR 11 electronic-records rules require exactly this.

So clinical trials is a **real** vertical — more defensible than denial management, more valuable per seat, and technically almost free because the primitives are already in v0.2. But it is still an enterprise sale. **Ship the research wedge first to accumulate reference customers, then sell into trial operations from a position of evidence.**

---

## 7. Numbers I'm willing to put in writing

I am not going to repeat the denial-management mistake of citing vendor math. Here is what is defensible on day one:

- **Hallucinated citations in LLM-drafted long-form text: structural zero.** Not a benchmark — a write-path invariant. If the user or agent tries to commit an ungrounded claim with `require_provenance: true`, the SAVEPOINT rolls back. There is nothing to measure because nothing can happen.
- **Time to find "where did this claim come from" collapses from minutes to one query.** `smriti notes read <id> --json | jq '.claim_spans'`. Defensible because it's a lookup, not a re-read.
- **All data stays on the user's machine.** Defensible because it's in a single `.db` file on disk.

Anything beyond that — productivity lift, draft quality, citation accuracy — needs a user study with real cohorts before I'll put a number on it. I'd rather ship the tool and let early users generate the evidence than fabricate it.

---

## 8. Distribution: how you get the first 1,000 users

The research wedge is built for organic distribution. The enterprise use cases are not. Sequence:

1. **Ship a 60-second GIF** of the rollback moment. Claim that doesn't overlap → transaction rolls back → green `smriti verify` at the end. Post to HN, r/LocalLLaMA, r/ObsidianMD, academic Twitter.
2. **Write one case study with one real PhD student.** Pick a field where citation stakes are high (neuroscience, law, history). Let them run it on an actual thesis chapter. Publish the workflow.
3. **Ship an Obsidian plugin** that proxies every note write through `smriti wiki-tx submit`. Zero-migration adoption — Obsidian users keep their vault, gain provenance enforcement. This is the single highest-leverage integration because it meets users where they already are.
4. **Build a Zotero importer.** Every academic already has a Zotero library. `smriti import zotero ~/Zotero/` ingests every PDF as a source with verified metadata. Eliminates the onboarding cliff.
5. **Ship a "Claude-code for researchers" MCP profile.** A pre-wired MCP config that points Claude at the local `smriti.db` and disables any write that isn't grounded. Distribute as a single JSON file.

At step 3 you have a viral loop. At step 4 you have zero onboarding friction. At step 5 you are the only agent-memory layer that works inside an existing writing workflow with zero code changes.

---

## 9. Value proposition (one line for each buyer)

- **For the PhD student / postdoc**: Your advisor will never again text you *"where did you get this?"* and leave you panicking at midnight. Because the answer is `smriti notes read` and the provenance is in the file.
- **For the solo analyst / journalist**: Every sentence in your draft is traceable to a source span, and your vault is a single file you can email to fact-checkers.
- **For the biotech operations lead**: The same binary that your research scientists use for lit review is the one that will pass your next FDA inspection on protocol amendment traceability. One tool, two revenue lines.
- **For the open-source maintainer**: Finally, an agent-memory layer that doesn't require a SaaS subscription or a vector DB server. One binary. One file. Rust. MIT.

---

## 10. Next action (one, not three)

**Ship the 60-second rollback GIF and post it to HN with a link to the binary.** That is the entire go-to-market for v0.2. Everything else — Obsidian plugin, Zotero importer, trial-ops whitepaper — is downstream of whether that post lands. Until you have the GIF, nothing else matters.

Failure mode to watch for: spending the next two weeks building the Obsidian plugin before you've tested whether the core pitch lands. Do not do that. **Ship the demo. Validate the pitch. Then build integrations.**

---

## References

- FACTUM — structural citation verification — arXiv:2601.05866
- Citation-Grounded Code Comprehension — arXiv:2512.12117
- Zep / Graphiti — bi-temporal edges — arXiv:2501.13956
- MemoTime — confidence-weighted contradiction detection — arXiv:2510.13614
- A-MEM — Zettelkasten-style agent memory — arXiv:2502.12110
- AGM belief revision postulates — arXiv:2603.17244
- FDA 21 CFR 312.32 — IND safety reporting (clinical trial traceability)
- FDA 21 CFR Part 11 — electronic records / audit trails

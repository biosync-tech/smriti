# Smriti Go-to-Market Strategy

**Status:** Draft v1 · 2026-05-04
**Owner:** Biosync
**Decision pending:** Side project vs. co-equal company (see §10)

---

## 0. Bottleneck Logic (read this first)

Smriti's actual moat = bi-temporal edges + enforced provenance + hash-chained audit + contradiction detection + CLS consolidation. That moat is worth ~$0 to people who don't need verifiable knowledge, and worth $1k–$5k/seat/month to people who get audited.

The lane question is not a marketing question. It is a willingness-to-pay question. Pick the audited people.

---

## 1. The Lane

**Verifiable knowledge infrastructure for AI-assisted regulated work.** Specifically: clinical research and biotech operations.

What this is *not*:
- Not a developer agent-memory framework competing with Mem0/Letta/Zep (race to the bottom, VC-subsidized free tier, no defensible WTP).
- Not a consumer second brain competing with Obsidian/Notion/Mem.ai (wrong moat, wrong distribution, ChatGPT memory is killing this market).
- Not an enterprise knowledge graph competing with Glean/Notion/Confluence (wrong founder shape, requires sales team).

---

## 2. ICP — Ideal Customer Profile

A 5–50 person organization that:

1. Generates AI-assisted documentation that **will be audited** (FDA, IRB, sponsor monitor, internal QA).
2. **Cannot justify Veeva** ($150k–500k/yr) but can justify $500–5k/month.
3. Has a **technical-enough champion** — CTO, head of ops, lead PI — who can install software without IT procurement.

### Three concrete shapes of this ICP

| Segment | Who | WTP | Sales motion | Year-1 priority |
|---|---|---|---|---|
| **Decentralized clinical trial sponsors (DCTs)** | Newer, smaller, tech-forward sponsors with distributed sites and patient self-reporting | $2–5k/mo per protocol | Founder-led demos, conference presence | **Primary buyer** |
| **Health-AI / digital-health startups** | Building AI for clinical decisions, need every output to cite source | $500–2k/mo | Dev-led adoption + outbound | **Primary buyer** |
| **Academic IIS investigators (PIs)** | Run investigator-initiated studies, no Veeva budget, currently on Word+email | $0 (free tier) | Open source + community | **Design partners + distribution, not revenue** |

### Explicitly excluded for Year 1

- Big pharma sponsors (have Veeva, 18-month sales cycle, wrong founder for enterprise sales)
- Large CROs (same as above)
- Mid-tier biotechs with existing systems (sticky vendor lock-in)
- Non-regulated knowledge workers (no WTP for audit features)

---

## 3. Job-to-Be-Done

> "When I have to prove to a regulator, sponsor, or auditor that the AI-assisted decisions in my trial were grounded in real source data and made under the rules in force at the time, I need a single auditable knowledge layer that survives the inspection — so I don't lose 6 months of timeline or my next funding round."

The job is the same across both primary buyers: **make AI-assisted clinical work auditable and reconstructable.** That is the pitch.

---

## 4. Pricing

All SaaS recurring. No one-time licenses — kills retention signal and investor narrative.

| Tier | Price | What's included | Buyer |
|---|---|---|---|
| **Open Source** | Free, self-hosted | Core graph, integrity layer, MCP server, web dashboard | Devs, academic PIs, distribution funnel |
| **Smriti Pro** | $500/month | Hosted (or assisted self-host), integrity sweep export, contradiction inbox UI, support SLA, IRB-ready exports | Health-AI startups, small DCT teams |
| **Smriti for Trials** | $2k–$5k/month per active protocol | Trial amendment ledger templates, FDA-formatted exports, bi-temporal evidence chain views, **validation pack (IQ/OQ/PQ)** | DCT sponsors |

**Critical:** the validation pack is the price-anchor for the higher tier. Without IQ/OQ/PQ documentation, Smriti is a shadow ledger and price ceiling is ~$1k/month. With it, you can charge $2k–$5k/month per protocol.

---

## 5. Distribution Channels (ranked by ROI for a solo founder)

### 1. Dev-led adoption via open source (highest leverage)
- Excellent docs, GitHub stars, MCP integrations.
- Devs at biotechs adopt for personal use → drag into work → org-level buy.
- Same playbook as Linear, Hashicorp, Notion, Obsidian.
- **Why this matters most:** scales once, not per-customer. Compounds.

### 2. Targeted community presence (where biotech ops people actually are)
- DTRA (Decentralized Trials & Research Alliance)
- SCRS (Society for Clinical Research Sites)
- ACRP (Association of Clinical Research Professionals)
- RESI Conference, HLTH, HIMSS adjacent events
- Newsletters: Endpoints News, STAT, Health Tech Nerds, NEJM AI
- Communities: BioBeta, RA Capital network, biotech founder Slacks

### 3. Direct outbound to 50 named accounts
- Public sources: ClinicalTrials.gov DCT filter, Decibel, Crunchbase health-AI seed/A list
- 4-email sequence ending in 20-min demo
- Target 5–10% cold-to-call conversion
- Your background (PhD + healthcare AI startup) is the warm-up

### 4. One conference talk per quarter
- Topic: "How we built an audit-trail-first knowledge graph for clinical AI"
- Highest-credibility lead source per dollar spent

### Channels to skip explicitly

- Paid ads (don't have budget for 90-day sales cycle)
- Local in-person networking (BNI, Chamber of Commerce — wrong audience)
- Generic content marketing (no audience yet to compound on)
- LinkedIn influencer plays (low signal in regulated industries)
- Podcast tour (high effort, low conversion until you have a logo)

---

## 6. 12-Month Sequenced Motion

| Phase | Months | Goal | Success metric |
|---|---|---|---|
| **0 — Position** | M1 | Reposition landing on regulated knowledge. Ship trial amendment ledger demo. Publish "How we'd model a Phase 2 NSCLC amendment ledger" as a long-form artifact. | 5 cold conversations with named buyers |
| **1 — Design partners** | M2–3 | Recruit 3 academic IIS PIs as **free** design partners (testimonials in exchange). Recruit 1 DCT sponsor as **paid pilot** ($500–1k/mo discount). Build templates from their needs only — no speculative features. | 1 paid pilot live |
| **2 — First paying tier** | M4–6 | Launch Smriti Pro publicly. Begin validation pack work for higher tier. One conference talk. | 5 paying customers, $2.5k MRR |
| **3 — Validation + Trials tier** | M7–12 | Ship validation pack. Launch Smriti for Trials. Hire first part-time SE or contract sales help. | 15 paying customers, $20–40k MRR |

### Year-1 north star metric

Not raw MRR. Not customer count. **Sean Ellis test ≥40%** — % of users who'd be "very disappointed" if Smriti went away. This is the only metric that proves PMF in a small-N regulated B2B market.

---

## 7. Critical Build Dependencies

The GTM stalls without these. Build them in priority order:

1. **Killer demo on landing page.** Trial amendment ledger walkthrough. Highest-conversion artifact you can build.
2. **Hosted offering.** Required for non-engineer biotech ops buyers. Self-hosted-only caps you at devs.
3. **Validation pack** (IQ/OQ/PQ documentation). Required to price above $1k/mo. Estimated cost: 2–4 months of work + $20–50k if outsourced.
4. **One named reference customer with logo or testimonial.** Until this exists, every sales call starts cold.
5. **IRB/FDA-formatted export templates.** Speeds time-to-value for new pilots.

### What to NOT build (defer or skip)

- SOC 2 (premature — wait until first enterprise asks)
- HITRUST (premature — wait until first health system asks)
- Enterprise SSO (premature — wait until 5 paying teams ask)
- Mobile app (irrelevant for ICP)
- Mem0-style consumer episodic memory (would dilute positioning)
- Plugin ecosystem (premature — ship integrations 1-by-1 yourself)

---

## 8. Risks (honest)

### Risk 1: Validation pack is real money + months of work
- **Impact:** May delay $2k+ tier by 6–12 months
- **Mitigation:** Sell Pro tier without it first. Fund validation from Pro revenue. Consider partnering with a validation consultancy on revenue share.

### Risk 2: You have another company
- **Impact:** Healthcare AI startup competes for attention. *This is the single biggest risk to this GTM.*
- **Mitigation:** Decide explicitly whether Smriti is a side project or a co-equal company. See §10. Cannot be both.

### Risk 3: Sales cycles in regulated industries are slow
- **Impact:** 60–180 days even at the small end. Cash burn before revenue materializes.
- **Mitigation:** Plan for 12 months of runway minimum. Consider angel/seed round if pursuing co-equal path.

### Risk 4: Solo founder velocity ceiling
- **Impact:** This GTM hits a wall at ~$500k–$1M ARR without sales/SE hire.
- **Mitigation:** Plan for first hire by month 12. Build with that assumption.

### Risk 5: Open-source devalues paid tier perception
- **Impact:** Buyers may resist paying for what's "free on GitHub."
- **Mitigation:** Paid tier value is hosting + validation pack + support, not the code. Make this explicit on pricing page.

---

## 9. What "Done" Looks Like — Year 1 Exit Criteria

By month 12, you should have one of these three outcomes:

**Outcome A (success):** $300k+ ARR, 15+ paying customers, ≥40% Sean Ellis score, 1 conference keynote, 2 named reference customers. → Raise seed or grow bootstrapped.

**Outcome B (mixed):** $50k–300k ARR, 5–15 customers, ≥30% Sean Ellis score, signal of demand but not breakaway. → Decide whether to continue investing or downgrade to side project.

**Outcome C (failure):** <$50k ARR, <5 customers, <30% Sean Ellis. → Open source the project, harvest credibility, redirect to other ventures.

Pre-commit to which outcome triggers which action. Don't decide reactively at month 12.

---

## 10. Meta-Bottleneck: Is Smriti a Side Project or a Co-Equal Company?

**This decision must be made in writing before executing the GTM above.** Without it, every tactical question stalls.

### Decision framework

Score each on 1–5 (1 = constrained, 5 = abundant):

| Factor | Side project requires | Co-equal requires | Your score |
|---|---|---|---|
| **Weekly time available for Smriti** | 5–10 hrs | 25–40 hrs | __ |
| **Capital runway (12 months)** | $0 (open source) | $50–150k | __ |
| **Co-founder or first hire by M12** | Not needed | Likely required | __ |
| **Cannibalization with other startup** | Acceptable if pure OSS | Must be addressed | __ |
| **Personal energy for B2B sales cycles** | Not applicable | Required (60–180 day cycles) | __ |
| **Acceptance of 12–18 month payback period** | Not applicable | Required | __ |

### Decision rule

- All scores ≥3 → **co-equal company.** Pursue full GTM above.
- Any score ≤2 → **side project.** Do not pursue paid tiers. Limit scope to: open source + GitHub stars + occasional conference talks. Accept zero revenue. Optimize for credibility and option value (acquisition, future co-founder, future raise).
- Mixed (2s and 4s) → **time-boxed pilot.** 90-day commitment to a thin slice (1 design partner + landing reposition + 1 demo). Reassess at day 90 with hard go/no-go gate.

### Why this matters

The GTM above assumes co-equal. If Smriti is a side project, the right answer is open source it loudly, ship the integrity layer credibly, and let it be a portfolio asset — not a revenue line. Trying to half-execute the GTM as a side project burns time, generates no traction, and erodes the "verifiable knowledge" positioning by association with abandoned promises.

---

## 11. This Week's Actions

1. **Make the §10 decision in writing.** This is the gate.
2. If co-equal or time-boxed pilot: ship the trial amendment ledger demo + reposition the landing page (queued).
3. List 50 named target accounts (DCT sponsors + health-AI startups). Use ClinicalTrials.gov DCT filter and Crunchbase.
4. Reach out to 3 academic PIs in your aging/lung network — offer free design-partner status.

---

## Appendix A: Why Not the Other Lanes (one-liner reasons)

| Lane | Why not |
|---|---|
| Developer agent memory infra | VC-subsidized free competitors, no defensible WTP, race to the bottom |
| Consumer SMB second brain | Wrong moat (no audit need), wrong distribution (mass-market), ChatGPT memory killing the category |
| Enterprise knowledge graph | Wrong founder shape (need sales team), wrong sales motion (RFPs, 12-month cycles) |
| Local in-person SMB | Wrong unit economics ($30/hr effective wage), wrong audience (no audit pain), wrong product (services masquerading as software) |

## Appendix B: Founder-Market Fit Statement

You are one of perhaps 200 people on earth who can credibly say:

> "I built this with a PhD background in lung biology and aging, operating experience in healthcare AI workflow orchestration (prior auth, eligibility, denials), and an integrity layer architected against ICH E6(R3) and 21 CFR Part 11."

That sentence does not land at the South Valley Chamber of Commerce. It lands hard at a biotech CTO dinner, a DTRA panel, or an RA Capital LP gathering.

Use the credentialed channels. Skip the rest.

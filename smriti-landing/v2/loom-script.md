# Smriti — 90-second Loom video script

> A precision-instrument demo, not a sales reel. Goal: kill the install-friction objection. After watching, the viewer should think *"OK I get it; I'd try it on a real workload"* — not *"sounds cool, where's the deck."*

---

## Production checklist (≈ 30 min total)

- [ ] Loom account + screen-recording permission granted
- [ ] Terminal: large font (≥ 18 pt), warm color scheme matching the landing (cream paper / oxidized rust accent if possible — `Solarized Light` or `Smyck` work)
- [ ] Browser: Smriti landing page open in one tab, GitHub repo open in another
- [ ] Pre-seeded SQLite db with one realistic clinical-trial scenario (script below)
- [ ] Webcam ON, lower-right, small. Your face is the trust signal — even if you hate it.
- [ ] Quiet room. Single take preferred. Don't edit if you can help it; rough = honest.

---

## Pre-seed the demo database

Run once before recording. This sets up the scenario the viewer will see replayed.

```bash
# A single trial-amendment scenario with a hash-chained trail
smriti notes create --title "Protocol v3.2 amendments" \
  --content "Amendment v3.2 effective 2026-02-20 raised upper age limit from 65 to 75."

smriti notes create --title "Patient-14 enrolment" \
  --content "Patient-14 enrolled 2026-01-15. ECOG 1, age 58. Inclusion under v3.0."

smriti notes create --title "Patient-22 enrolment" \
  --content "Patient-22 enrolled 2026-03-05. ECOG 1, age 71. Eligibility under amendment v3.2."

# (Imagine an LLM call happens here that retrieves the above and produces an
# audited answer to "Why was Patient-22 eligible at age 71?" — the chain entry
# below stands in for that call's audit row.)
```

You'll execute one or two `smriti` commands LIVE in the recording. Everything else is just narration over what's already on disk.

---

## The 90-second script

Format: `[VISUAL]` = what's on screen · `[VOICE]` = what you say. Time markers are cumulative.

---

### 0:00 — 0:08 · The hook (8 s)

`[VISUAL]` Webcam fullscreen for 2 seconds, then jump-cut to a terminal showing a typical `tail -f` of mixed app/model/retrieval logs scrolling past. Optionally overlay text: **"FDA asks. You have four hours."**

`[VOICE]`
> "Your AI agent suggested something. A regulator asks where it got that. This is what most teams have to dig through —"

`[INTENT]` Plant the pain in 8 seconds. Don't soften it.

---

### 0:08 — 0:24 · The pivot (16 s)

`[VISUAL]` Cut to your browser, on the Smriti landing page (`smriti-landing/v2/index.html`). Scroll slowly past the headline ("An AI agent's memory you can defend") and the three-up problem section, pausing for a half-beat on each problem card.

`[VOICE]`
> "I built Smriti because watching teams reconstruct an AI decision after the fact, manually, from log files, was the fourth time I'd seen the same wasted afternoon. Self-hosted. Single Rust binary. SQLite. Zero cloud. Three primitives wired in so they can't be opted out of —"

`[INTENT]` Establish stakes + credibility (you've seen this happen multiple times). Land the three primitives by gesture, not by detail.

---

### 0:24 — 0:55 · The reveal (31 s — the meat)

`[VISUAL]` Switch to terminal. Type and run:

```bash
$ smriti verify --chain
```

Output appears: chain integrity ✓ across N events. Pause one beat.

Then run:

```bash
$ smriti consolidate --explain n_clinical_42
```

Output shows the score breakdown: cascade salience, degree, diversity, sigmoid score. Pause again.

Then (optional, if you have it wired) run:

```bash
$ smriti llm-audit replay <call_id>
```

Show the metadata that proves a past LLM call is reproducible: model, seed, prompt template, retrieval set, output hash.

`[VOICE]`
> "Here's the chain. Every change to memory — every note, every link, every model call — gets a SHA-256 hash that includes the previous one. One pass tells you it hasn't been touched.
>
> Here's why a piece of knowledge is in the system. Not just *what's stored*, but *why this matters now* — based on how it's been accessed, what it's connected to, how recently. Built on a 2016 neuroscience paper on how the brain consolidates memory — same principle, in code.
>
> And here's a model call from last week. Same model, same seed, same retrieved context — re-running it produces a bit-identical answer. Reproducible by design, not by hope."

`[INTENT]` Show, don't pitch. Three live commands. Each takes ≤ 8 seconds. The viewer sees the audit-trail story actually working.

---

### 0:55 — 1:18 · The shape of the answer (23 s)

`[VISUAL]` Cut back to the landing page. Scroll to the §02 approach cards (the three primitives). Pause on each card for ~3 seconds. Then scroll to the §03 proof section showing the spec sheet.

`[VOICE]`
> "Three primitives, in plain English. A hash-chained event log so tampering is detectable in one pass. Multi-timescale consolidation so memory gets cleaner over time, not bigger. Structural provenance so every claim has to overlap with a cited source — at write time, not as an afterthought.
>
> Open source. Measured benchmarks. Sub-millisecond audit overhead. Thirty-megabyte binary. No services. No cloud."

`[INTENT]` Anchor the three things in the viewer's head. The landing page does the work; you're a guide.

---

### 1:18 — 1:30 · The ask (12 s)

`[VISUAL]` Cut back to webcam, fullscreen.

`[VOICE]`
> "If you build AI agents in regulated work — clinical, biotech, healthcare — and your compliance team has ever asked you to defend a model output: I want twenty minutes. I'm in customer discovery. Not selling. The link is below. Or hi at smriti dot dev."

`[INTENT]` Direct ask. No "subscribe and like." The CTA is a 20-minute call.

---

## Editing notes

- **Don't edit if you can help it.** A single uncut take signals confidence. Re-record three times if needed; pick the cleanest.
- **Add captions.** Loom does this automatically; turn it on. ~40% of viewers watch muted.
- **Background music: NO.** Music turns this into marketing. Silence + your voice + terminal sounds = signal.
- **Loom thumbnail:** the terminal frame at 0:30 (the `verify --chain` output) — not your face, not the landing page. Anchor the viewer in *what they're going to see*.

---

## Distribution

When you DM the targets from the Week-1 list, the link to this Loom goes in the **second** message — not the first. First message is the question; you only send the Loom if they say "what is it again?" Sending it cold lowers the response rate.

In the call follow-up email: "Here's the 90 seconds of context I didn't want to do live." Always after the call, never before — preserves the candor.

Embed on the landing page in a future iteration as a `<video>` tag in the proof section. Today it's a one-link follow-up artifact.

---

## What this script deliberately does NOT include

- **Per-feature deep-dives.** You're showing the *shape* of three primitives, not their implementation. Anyone who wants depth can read the repo.
- **Comparison to Mem0 / Zep / Letta.** Comparisons turn a demo into a sales pitch and put the viewer on guard. Save them for the call.
- **Pricing.** You don't have it yet, and saying it would commit you publicly before discovery is done.
- **Founder backstory.** Twelve seconds of "I built this because…" is enough; thirty seconds is indulgent.
- **A live install.** You're showing what Smriti *does*, not how to set it up. The README handles that.

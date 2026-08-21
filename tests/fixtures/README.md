# Test fixtures

## `longmemeval_synthetic.json`

Hand-crafted synthetic corpus mirroring the structure of the public
[LongMemEval benchmark](https://huggingface.co/datasets/xiaowu0162/longmemeval).

**Contents:** 39 notes across 10 sessions spanning 2026-01-15 → 2026-05-05 in a
coherent clinical-trial narrative (tarlozumab Phase II, three cohorts, four
protocol amendments, five patients, real-world AE/PK/DSMB activity).

**Questions:** exactly 50, distributed as
- 25 single-hop (single-note answer lookups)
- 15 multi-hop (require synthesis across 2-3 notes)
- 10 temporal-contradiction (later note supersedes earlier; question asks for
  current state).

## Schema

```jsonc
{
  "version": "0.3-synthetic",
  "sessions": [
    {
      "session_id": "s<N>",          // s1..s10 in the default synthetic corpus
      "ts": "<RFC3339 with Z>",      // session wall-clock
      "notes": [
        {
          "id": "n_s<sess>_<idx>",   // unique across the corpus
          "title": "...",
          "content": "..."
        }
      ]
    }
  ],
  "questions": [
    {
      "id": "q<N>",                  // q1..q50
      "prompt": "...",
      "answer_note_ids": ["n_s1_2", "..."],   // ≥1 IDs
      "category": "single-hop" | "multi-hop" | "temporal-contradiction"
    }
  ]
}
```

Every `answer_note_ids` entry must resolve to a note that exists in the
corpus — `tests/longmemeval_replay.rs::fixture_loads` enforces this.

## Plugging in the real LongMemEval dataset

The synthetic version is the default for hermetic tests. To run the harness
against the real LongMemEval corpus:

1. Download `longmemeval_s.json` (or any of the variants) from the
   [Hugging Face dataset card](https://huggingface.co/datasets/xiaowu0162/longmemeval).
   The full set is ~6 GB and requires HF authentication.

2. Convert to this directory's schema. The real format groups conversation
   turns by session and per-question evidence by message ID — the conversion
   is mechanical: each session's messages become one or more `notes`, and
   each evaluation question maps to a `Question` with `answer_note_ids`
   pointing at the converted notes that contain the evidence.

   Keep the converter script out of the test runner so unit tests stay
   hermetic. A standalone `examples/convert_longmemeval.rs` (or a quick
   Python script in `scripts/`) is the right shape — both are out of scope
   for v0.3.

3. Point the harness at the converted JSON. Either:
   - Replace `tests/fixtures/longmemeval_synthetic.json` in-place (lossy —
     hides which corpus the recall numbers are from), or
   - Set a `LONGMEMEVAL_FIXTURE` env var and update
     `tests/longmemeval_replay.rs::load_fixture` to read it. Recommended.

## Why synthetic by default

- **Hermetic.** No network, no HF credentials, no 6 GB download in CI.
- **Debuggable.** Small enough to reason about. Test failures point at a
  specific question with a specific narrative arc, not at "question 4017 in
  shard 12."
- **Reproducible.** The corpus is checked in — recall numbers are stable
  across machines and CI runs. The real LongMemEval dataset version is a
  moving target.
- **Honest about scope.** A 50-question synthetic corpus is not a stand-in
  for a real benchmark; it is a regression harness for the retrieval path.
  Paper §6.5 (worked replay case study) is where the real LongMemEval
  numbers belong.

## Why clinical trials

The Smriti project's wedge is regulated agent memory — clinical trials,
healthcare informatics, where audit-trail integrity (ICH E6(R3) §4.1) is
load-bearing and where hallucinated retrieval has real consequences. Lean
into the domain so the recall benchmark exercises terms that look like the
production workload, not generic Wikipedia paragraphs.

# v0.3 — Cascade-Salience Scoring + LongMemEval Retrieval Harness

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-timescale access + recency terms in `compute_score` with the Benna-Fusi cascade salience readout (already shipped in `src/features/cascade.rs`), and land a LongMemEval-style retrieval harness that measures Recall@k and MRR against the new score so we can defend the v0.3 claim "consolidation gets cleaner over time."

**Architecture:**
The cascade module (Migration 011) already exposes per-note salience as a weighted readout across K=6 exponentially-spaced synaptic levels. Today's `compute_score` ignores it: it sums `w_access * log1p(access_count)` + `w_recency * exp(-Δt/τ)` — a textbook single-timescale model. Stage 1 collapses those two terms into a single `w_salience * cascade_salience` component. Structural (`degree`) and semantic (`context_diversity`) components are orthogonal and stay untouched. Stage 2 lands `tests/integration/longmemeval_replay.rs` — a synthetic multi-session corpus with known recall targets, plus pluggable JSON loader for the real LongMemEval dataset later.

**Tech Stack:** Rust 2021, rusqlite 0.31 (`bundled,vtab`), chrono 0.4, serde_json 1, existing `src/features/cascade.rs` and `src/features/consolidation.rs`. No new dependencies.

---

## File Structure

| Path | Status | Responsibility |
|---|---|---|
| `src/features/consolidation.rs` | Modify | `compute_score`, `ScoreBreakdown`, `ScoreWeights`, `explain_score`, `run_consolidation_pass` |
| `src/features/cascade.rs` | Modify (small) | Re-export `salience_peek_from_db` helper that returns `Ok(0.0)` on missing state |
| `tests/longmemeval_replay.rs` | Create | Integration test: synthetic dataset → seed DB → retrieve → recall metrics |
| `tests/fixtures/longmemeval_synthetic.json` | Create | Hand-crafted ~50-question fixture (multi-session, time-spread accesses) |
| `docs/papers/sections/06-3-replay-reproducibility.tex` | Modify (later, out of scope here) | Will eventually consume the harness output |

---

## Stage 1 — Cascade-salience scoring

### Task 1: Extend `ScoreWeights` with `w_salience`, drop `w_access` and `w_recency`

**Files:**
- Modify: `src/features/consolidation.rs:76-96` (the `ScoreWeights` struct and `Default` impl)

- [ ] **Step 1: Update the struct and defaults**

```rust
// src/features/consolidation.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScoreWeights {
    /// Cascade salience (Benna-Fusi multi-timescale readout).
    /// Replaces the previous w_access + w_recency single-timescale terms.
    pub w_salience: f32,
    pub w_degree: f32,
    pub w_diversity: f32,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            // 0.45 ≈ old (w_access=0.35 + w_recency=0.10). Sum still 1.00.
            w_salience: 0.45,
            w_degree: 0.25,
            w_diversity: 0.30,
        }
    }
}
```

- [ ] **Step 2: Run `cargo build` to surface every broken caller**

Run: `cd /Users/kra/Documents/repos/smriti && cargo build 2>&1 | head -60`
Expected: Compile errors at every site that reads `w_access`, `w_recency`, or `recency_tau_days` on `ScoreWeights`. We will fix these in Tasks 2-4. Do NOT commit yet.

---

### Task 2: Add `salience_peek_for_note` helper to cascade module

**Files:**
- Modify: `src/features/cascade.rs:286-298` (add a new helper just below `record_access`)

The helper exists so `consolidation.rs` does not need to know JSON serialisation or the load/save round-trip — it just asks for "what is the salience right now?" and gets a number.

- [ ] **Step 1: Write the failing test in `src/features/cascade.rs` (inside the `tests` module)**

```rust
// src/features/cascade.rs — append to the existing #[cfg(test)] mod tests {
#[test]
fn salience_peek_for_note_zero_when_no_state() {
    use crate::storage::Database;
    let db = Database::open_in_memory().expect("db");
    let conn = db.conn();
    conn.execute(
        "INSERT INTO notes (id, title, content, created_at, updated_at) \
         VALUES ('n_peek', 't', 'c', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    let cfg = CascadeConfig::default();
    let s = salience_peek_for_note(conn, "n_peek", &cfg, Utc::now()).unwrap();
    assert_eq!(s, 0.0, "no cascade_state row → salience reads as 0");
}

#[test]
fn salience_peek_for_note_reflects_recorded_access() {
    use crate::storage::Database;
    let db = Database::open_in_memory().expect("db");
    let conn = db.conn();
    conn.execute(
        "INSERT INTO notes (id, title, content, created_at, updated_at) \
         VALUES ('n_acc', 't', 'c', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    let cfg = CascadeConfig::default();
    record_access(conn, "n_acc", &cfg).unwrap();
    let s = salience_peek_for_note(conn, "n_acc", &cfg, Utc::now()).unwrap();
    assert!(s > 0.0, "after one access, salience should be positive: got {s}");
}
```

- [ ] **Step 2: Run the new tests, confirm they fail**

Run: `cargo test features::cascade::tests::salience_peek_for_note -- --nocapture`
Expected: `error[E0425]: cannot find function 'salience_peek_for_note' in this scope`.

- [ ] **Step 3: Implement the helper**

```rust
// src/features/cascade.rs — insert below `record_access`
/// Read-only salience readout for a stored note. Returns `Ok(0.0)` when the
/// `cascade_state` column is NULL (e.g. note created pre-Migration-011 or
/// never accessed). Does NOT mutate or persist state — safe to call from
/// scoring passes that should not register an access event.
pub fn salience_peek_for_note(
    conn: &Connection,
    note_id: &str,
    config: &CascadeConfig,
    now: DateTime<Utc>,
) -> AppResult<f64> {
    match load(conn, note_id)? {
        Some(state) => Ok(state.salience_peek(config, now)),
        None => Ok(0.0),
    }
}
```

- [ ] **Step 4: Run the tests, confirm they pass**

Run: `cargo test features::cascade::tests::salience_peek_for_note`
Expected: `2 passed`.

- [ ] **Step 5: Run the full cascade suite to confirm no regressions**

Run: `cargo test features::cascade`
Expected: `14 passed; 0 failed` (12 existing + 2 new).

- [ ] **Step 6: Commit**

```bash
cd /Users/kra/Documents/repos/smriti
git add src/features/cascade.rs
git commit -m "feat(cascade): add salience_peek_for_note read-only helper

Lets the consolidation scorer query cascade salience without
touching state or registering an access event."
```

---

### Task 3: Rewrite `compute_score` and `ScoreBreakdown` around salience

**Files:**
- Modify: `src/features/consolidation.rs:98-116` (`ScoreBreakdown`)
- Modify: `src/features/consolidation.rs:163-200` (`compute_score`)

This is the core BREAKING change. The function signature changes; downstream callers fix in Tasks 4-5.

- [ ] **Step 1: Update unit tests first (TDD)**

Replace the existing `score_*` tests in `src/features/consolidation.rs` (currently at lines ~488-548) with the new contract:

```rust
// src/features/consolidation.rs — replace the existing score_* test block
#[test]
fn score_zero_inputs_gives_half() {
    let w = ScoreWeights::default();
    let b = compute_score(0.0, 0, 0.0, w);
    assert!((b.score - 0.5).abs() < 1e-3, "sigmoid(0) ≈ 0.5, got {}", b.score);
}

#[test]
fn score_rises_monotonically_with_salience() {
    let w = ScoreWeights::default();
    let low = compute_score(0.1, 0, 0.0, w).score;
    let mid = compute_score(1.0, 0, 0.0, w).score;
    let high = compute_score(5.0, 0, 0.0, w).score;
    assert!(low < mid && mid < high, "expected monotonic: {low} {mid} {high}");
}

#[test]
fn score_combines_orthogonal_components() {
    let w = ScoreWeights::default();
    // Salience-only beats degree-only beats diversity-only at equal magnitudes —
    // matches the default weight ordering w_salience > w_diversity > w_degree.
    let s_only = compute_score(1.0, 0, 0.0, w).score;
    let d_only = compute_score(0.0, 0, 1.0, w).score; // diversity
    let g_only = compute_score(0.0, 3, 0.0, w).score; // degree (log1p(3) ≈ 1.39)
    assert!(s_only > d_only, "salience(1) should beat diversity(1): {s_only} vs {d_only}");
    assert!(d_only > g_only, "diversity(1) should beat degree(3): {d_only} vs {g_only}");
}

#[test]
fn diversity_clamped_to_unit_interval() {
    let w = ScoreWeights::default();
    let a = compute_score(0.0, 0, -1.0, w);
    let b = compute_score(0.0, 0, 0.0, w);
    let c = compute_score(0.0, 0, 2.0, w);
    let d = compute_score(0.0, 0, 1.0, w);
    assert!((a.score - b.score).abs() < 1e-6, "diversity clamped at 0");
    assert!((c.score - d.score).abs() < 1e-6, "diversity clamped at 1");
}

#[test]
fn breakdown_exposes_salience_component() {
    let w = ScoreWeights::default();
    let b = compute_score(2.0, 0, 0.0, w);
    assert!((b.salience_component - w.w_salience * 2.0).abs() < 1e-5);
    assert_eq!(b.cascade_salience, 2.0);
}
```

- [ ] **Step 2: Run the tests, confirm they fail**

Run: `cargo test features::consolidation::tests::score_`
Expected: Compile failure — `compute_score` still has the old signature, and `ScoreBreakdown` is missing `cascade_salience` / `salience_component`.

- [ ] **Step 3: Replace `ScoreBreakdown`**

```rust
// src/features/consolidation.rs — replace the existing struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub note_id: String,
    /// Cascade salience readout at scoring time (Benna-Fusi 2016 §3, weighted
    /// sum over u_0..u_{K-1}). Replaces the legacy `access_count` +
    /// `days_since_access` pair as the temporal signal.
    pub cascade_salience: f64,
    pub degree: u32,
    pub context_diversity: f32,
    pub salience_component: f32,
    pub degree_component: f32,
    pub diversity_component: f32,
    pub raw_sum: f32,
    pub score: f32,
}
```

- [ ] **Step 4: Replace `compute_score`**

```rust
// src/features/consolidation.rs — replace the existing fn
/// Compute consolidation score from cascade salience + structural + semantic
/// signals. Pure function — deterministic, unit-testable, no I/O.
///
/// Why salience and not access_count + recency: Benna & Fusi 2016 prove that
/// a multi-timescale cascade lifts memory capacity from O(√N) to O(N/log N)
/// vs a single-timescale leaky integrator. The cascade salience IS the
/// engineered temporal signal; mixing it with a separate `exp(-Δt/τ)` term
/// would re-introduce the single-timescale regime we just escaped.
pub fn compute_score(
    cascade_salience: f64,
    degree: u32,
    context_diversity: f32,
    w: ScoreWeights,
) -> ScoreBreakdown {
    let salience_c = w.w_salience * cascade_salience as f32;
    let degree_c = w.w_degree * (1.0 + degree as f32).ln();
    let diversity_c = w.w_diversity * context_diversity.clamp(0.0, 1.0);

    let raw = salience_c + degree_c + diversity_c;
    let score = sigmoid(raw);

    ScoreBreakdown {
        note_id: String::new(),
        cascade_salience,
        degree,
        context_diversity,
        salience_component: salience_c,
        degree_component: degree_c,
        diversity_component: diversity_c,
        raw_sum: raw,
        score,
    }
}
```

- [ ] **Step 5: Run the score tests, confirm they pass**

Run: `cargo test features::consolidation::tests::score_ features::consolidation::tests::diversity features::consolidation::tests::breakdown`
Expected: All 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/features/consolidation.rs
git commit -m "feat(consolidation): replace compute_score with cascade-salience signal

Drops the single-timescale w_access * log1p(access_count) and
w_recency * exp(-Δt/τ) terms in favour of a single
w_salience * cascade_salience(t) component sourced from the
Benna-Fusi cascade. Structural (degree) and semantic
(context_diversity) components untouched.

BREAKING: ScoreBreakdown JSON shape changes — drops
access_component / recency_component / days_since_access /
access_count, adds cascade_salience / salience_component.
The 'smriti consolidate --explain' CLI consumer is updated
in the next commit."
```

---

### Task 4: Migrate `explain_score` and `run_consolidation_pass` to read cascade salience

**Files:**
- Modify: `src/features/consolidation.rs:249-281` (`explain_score`)
- Modify: `src/features/consolidation.rs:299-360` (`run_consolidation_pass` — the loop body around line 344)

- [ ] **Step 1: Rewrite `explain_score`**

```rust
// src/features/consolidation.rs — replace the existing fn
pub fn explain_score(
    conn: &Connection,
    note_id: &str,
    weights: ScoreWeights,
) -> AppResult<ScoreBreakdown> {
    let now = Utc::now();
    let degree: i64 = conn.query_row(
        "SELECT COUNT(*) FROM links l \
         WHERE (l.source_note_id = ?1 OR l.target_note_id = ?1) \
           AND l.valid_until IS NULL",
        params![note_id],
        |r| r.get(0),
    )?;
    let salience = crate::features::cascade::salience_peek_for_note(
        conn,
        note_id,
        &crate::features::cascade::CascadeConfig::default(),
        now,
    )?;
    let mut b = compute_score(
        salience,
        degree as u32,
        0.0, // context_diversity — wired in once embedding clustering ships
        weights,
    );
    b.note_id = note_id.to_string();
    Ok(b)
}
```

- [ ] **Step 2: Rewrite the loop body in `run_consolidation_pass`**

Find the block at `src/features/consolidation.rs:308-346` and replace the SQL + scoring section with:

```rust
    // src/features/consolidation.rs — inside run_consolidation_pass
    let cascade_cfg = crate::features::cascade::CascadeConfig::default();

    let mut stmt = conn.prepare(
        "SELECT n.id,
                n.consolidation_score,
                (SELECT COUNT(*) FROM links l
                   WHERE (l.source_note_id = n.id OR l.target_note_id = n.id)
                     AND (l.valid_until IS NULL)) AS degree
         FROM notes n
         WHERE n.node_type = 'episode'",
    )?;

    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let score_before: f64 = row.get(1)?;
        let degree: i64 = row.get(2)?;
        Ok((id, score_before as f32, degree as u32))
    })?;

    // ... loop body:
    for row in rows {
        let (id, score_before, degree) = row?;
        report.scanned += 1;

        let salience = crate::features::cascade::salience_peek_for_note(
            conn, &id, &cascade_cfg, now,
        )?;

        let breakdown = compute_score(salience, degree, 0.0, weights);
        let new_score = breakdown.score;
        // ... rest of the loop body (write back, threshold checks, archiving) is UNCHANGED
    }
```

The SELECT no longer needs `access_count` or `last_accessed_at` columns — those still exist on the table for the access log to write into, but the scorer reads them indirectly via the cascade state.

- [ ] **Step 3: Run the consolidation suite**

Run: `cargo test features::consolidation`
Expected: All existing tests pass. If integration tests reference removed fields (`b.access_component` etc.), fix them in step 4.

- [ ] **Step 4: Fix any remaining compile errors**

Run: `cargo build 2>&1 | grep -E "error" | head -30`
Expected: Empty output. If errors remain, they are in `src/cli/handlers.rs` (the `consolidate --explain` printer) or `src/api/`. Update each printer/serializer to match the new field set:
- `b.access_component` → `b.salience_component`
- `b.recency_component` → REMOVED
- `b.days_since_access` → REMOVED
- `b.access_count` → `b.cascade_salience` (rename label in human-readable output)

- [ ] **Step 5: Run all tests**

Run: `cargo test --all`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add -u src/
git commit -m "feat(consolidation): wire cascade salience into scoring pass

run_consolidation_pass and explain_score now read salience via
cascade::salience_peek_for_note instead of access_count +
last_accessed_at. CLI/API serialisers updated to match the new
ScoreBreakdown shape.

Note: notes.access_count and notes.last_accessed_at still get
written by log_access — they remain the source of truth for
auditing and for reconstructing cascade state if it is lost.
The scorer just no longer reads them directly."
```

---

### Task 5: Tune w_salience defaults against existing benches

**Files:**
- Modify: `src/features/consolidation.rs:86-96` (only if benches show drift)

A note with one access today gets cascade salience ≈ 0.17 (single pulse, K=6 uniform readout). That maps to `salience_component = 0.45 * 0.17 ≈ 0.077` — much smaller than the old `w_access * log1p(1) = 0.35 * 0.69 ≈ 0.24`. Verify this is intentional.

- [ ] **Step 1: Run a probe to compare old-vs-new score on a known fixture**

Run:
```bash
cd /Users/kra/Documents/repos/smriti
cargo run --release --example explain_consolidation_score 2>&1 | head -20
```

If `examples/explain_consolidation_score.rs` does not exist, skip this step — the LongMemEval harness in Stage 2 will catch tuning drift.

- [ ] **Step 2: If recall regresses ≥5% in Stage 2, raise `w_salience` to 0.55-0.60 and re-run**

This is a placeholder hook — the actual decision lives in Stage 2's recall numbers.

- [ ] **Step 3: No commit required for this task** unless weights are tuned.

---

## Stage 2 — LongMemEval-style retrieval harness

Goal: a Rust integration test (`tests/longmemeval_replay.rs`) that seeds a synthetic multi-session corpus, runs retrieval queries against two strategies (FTS5 alone vs FTS5 reranked by `consolidation_score`), and reports Recall@5, Recall@10, and MRR.

**Why synthetic and not the real LongMemEval JSON:** the real dataset is ~6 GB on Hugging Face and ships with proprietary preprocessing. We hand-craft 50 question-answer pairs that exercise the same patterns (multi-session, temporal recall, contradiction handling) and structure the loader so the real dataset can plug in later via the same format. The paper §6.5 (worked replay case study) can use the real dataset once we have institutional access.

### Task 6: Define the dataset format

**Files:**
- Create: `tests/fixtures/longmemeval_synthetic.json`

- [ ] **Step 1: Write the schema as a Rust comment + the first 3 example records**

```json
{
  "version": "0.3-synthetic",
  "sessions": [
    {
      "session_id": "s1",
      "ts": "2026-01-15T10:00:00Z",
      "notes": [
        { "id": "n_s1_1", "title": "Patient-14 enrolment", "content": "Patient-14 enrolled on 2026-01-15. Inclusion criteria met: ECOG 0-2." },
        { "id": "n_s1_2", "title": "Protocol v3.2 amendments", "content": "Amendment v3.2 raised the upper-age limit from 65 to 75." }
      ]
    },
    {
      "session_id": "s2",
      "ts": "2026-02-01T14:30:00Z",
      "notes": [
        { "id": "n_s2_1", "title": "Patient-14 first dose", "content": "First dose administered 2026-02-01. Tolerated without grade-3 events." }
      ]
    }
  ],
  "questions": [
    {
      "id": "q1",
      "prompt": "What was the upper age limit after the v3.2 amendment?",
      "answer_note_ids": ["n_s1_2"],
      "category": "single-hop"
    },
    {
      "id": "q2",
      "prompt": "When did Patient-14 receive their first dose?",
      "answer_note_ids": ["n_s2_1"],
      "category": "single-hop"
    },
    {
      "id": "q3",
      "prompt": "Was Patient-14 eligible at enrolment given the v3.2 protocol?",
      "answer_note_ids": ["n_s1_1", "n_s1_2"],
      "category": "multi-hop"
    }
  ]
}
```

- [ ] **Step 2: Hand-write 47 more records** following the same structure across 8-12 sessions, with a mix of:
  - 25 single-hop queries (recall a single note)
  - 15 multi-hop queries (require ≥2 notes)
  - 10 temporal-contradiction queries (later session supersedes earlier)

This step is the bulk of Stage 2's authoring time — budget 2-3 hours. The synthetic corpus is intentionally small enough to hand-author and reason about, so test failures are debuggable.

- [ ] **Step 3: Commit the fixture**

```bash
git add tests/fixtures/longmemeval_synthetic.json
git commit -m "test(fixtures): synthetic LongMemEval-style corpus (50 Qs, 12 sessions)"
```

---

### Task 7: Write the loader + seeding code

**Files:**
- Create: `tests/longmemeval_replay.rs`

- [ ] **Step 1: Write a failing skeleton**

```rust
// tests/longmemeval_replay.rs
//! LongMemEval-style retrieval recall harness (paper §6.x).
//!
//! Loads a synthetic multi-session corpus, seeds an in-memory Smriti DB
//! with realistic access patterns, then measures retrieval recall under
//! two strategies:
//!   1. FTS5-only (baseline — pure keyword)
//!   2. FTS5 + score-weighted rerank (cascade-salience consolidation_score)
//!
//! Metrics reported: Recall@5, Recall@10, MRR.
//! Non-goal: latency. Use criterion for that. This file is correctness-only.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use smriti::storage::Database;

#[derive(Debug, Deserialize)]
struct Corpus {
    version: String,
    sessions: Vec<Session>,
    questions: Vec<Question>,
}

#[derive(Debug, Deserialize)]
struct Session {
    session_id: String,
    ts: DateTime<Utc>,
    notes: Vec<NoteRec>,
}

#[derive(Debug, Deserialize)]
struct NoteRec {
    id: String,
    title: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    prompt: String,
    answer_note_ids: Vec<String>,
    #[allow(dead_code)]
    category: String,
}

fn load_fixture() -> Corpus {
    let path: PathBuf = ["tests", "fixtures", "longmemeval_synthetic.json"]
        .iter()
        .collect();
    let s = fs::read_to_string(&path).expect("fixture present");
    serde_json::from_str(&s).expect("fixture parses")
}

#[test]
fn fixture_loads() {
    let c = load_fixture();
    assert_eq!(c.version, "0.3-synthetic");
    assert!(c.sessions.len() >= 8, "expected ≥8 sessions, got {}", c.sessions.len());
    assert_eq!(c.questions.len(), 50);
}
```

- [ ] **Step 2: Run, confirm it passes once the fixture is in place**

Run: `cargo test --test longmemeval_replay fixture_loads`
Expected: `1 passed`.

- [ ] **Step 3: Add the seed function**

```rust
// tests/longmemeval_replay.rs — append
fn seed(db: &Database, corpus: &Corpus) -> anyhow::Result<()> {
    let conn = db.conn();
    for session in &corpus.sessions {
        for note in &session.notes {
            conn.execute(
                "INSERT INTO notes (id, title, content, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                rusqlite::params![note.id, note.title, note.content, session.ts.to_rfc3339()],
            )?;
            // Drive a single access at session time so the cascade sees
            // a realistic temporal pattern, not a synthetic burst.
            smriti::features::consolidation::log_access(
                conn,
                &note.id,
                smriti::features::consolidation::AccessKind::Read,
                None,
                None,
            )?;
        }
    }
    Ok(())
}

#[test]
fn seeding_populates_notes_and_cascade() {
    let db = Database::open_in_memory().unwrap();
    let corpus = load_fixture();
    seed(&db, &corpus).unwrap();

    let count: i64 = db.conn().query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let total_notes: usize = corpus.sessions.iter().map(|s| s.notes.len()).sum();
    assert_eq!(count as usize, total_notes);

    let with_cascade: i64 = db.conn()
        .query_row("SELECT COUNT(*) FROM notes WHERE cascade_state IS NOT NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(with_cascade as usize, total_notes, "every seeded note should have cascade state");
}
```

- [ ] **Step 4: Run the seed test, fix issues, commit**

Run: `cargo test --test longmemeval_replay seeding_populates`
Expected: `1 passed`.

```bash
git add tests/longmemeval_replay.rs
git commit -m "test(longmemeval): fixture loader + seed helper"
```

---

### Task 8: Implement the two retrieval strategies + Recall@k / MRR

**Files:**
- Modify: `tests/longmemeval_replay.rs`

- [ ] **Step 1: Add retrieval helpers**

```rust
// tests/longmemeval_replay.rs — append
fn fts_topk(db: &Database, query: &str, k: usize) -> Vec<String> {
    let conn = db.conn();
    // FTS5 quirk: queries with non-token chars need quoting. Strip punctuation
    // and OR-join terms — matches what mcp::handlers::notes_search does.
    let cleaned: String = query
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    let terms = cleaned
        .split_whitespace()
        .filter(|t| t.len() > 2)
        .collect::<Vec<_>>()
        .join(" OR ");
    if terms.is_empty() {
        return vec![];
    }
    let mut stmt = conn.prepare(
        "SELECT n.id FROM notes n \
         JOIN notes_fts f ON f.rowid = n.rowid \
         WHERE notes_fts MATCH ?1 \
         ORDER BY rank LIMIT ?2",
    ).unwrap();
    stmt.query_map(rusqlite::params![terms, k as i64], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect()
}

fn fts_then_score_topk(db: &Database, query: &str, k: usize) -> Vec<String> {
    use smriti::features::consolidation::{compute_score, ScoreWeights};
    let conn = db.conn();
    // Pull FTS top-3k, rerank by α*fts_score + (1-α)*consolidation_score.
    // α=0.6 default — keep FTS dominant so totally off-topic notes stay out.
    let alpha: f32 = 0.6;
    let raw = fts_topk(db, query, k * 3);
    let weights = ScoreWeights::default();
    let cfg = smriti::features::cascade::CascadeConfig::default();
    let now = Utc::now();
    let mut scored: Vec<(String, f32)> = raw
        .into_iter()
        .enumerate()
        .map(|(rank, id)| {
            let fts_score = 1.0 / (rank as f32 + 1.0);  // reciprocal rank from FTS order
            let salience = smriti::features::cascade::salience_peek_for_note(conn, &id, &cfg, now)
                .unwrap_or(0.0);
            let degree: i64 = conn.query_row(
                "SELECT COUNT(*) FROM links l WHERE (l.source_note_id=?1 OR l.target_note_id=?1) AND l.valid_until IS NULL",
                rusqlite::params![id], |r| r.get(0)).unwrap_or(0);
            let cs = compute_score(salience, degree as u32, 0.0, weights).score;
            (id, alpha * fts_score + (1.0 - alpha) * cs)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(k).map(|(id, _)| id).collect()
}

fn recall_at_k(retrieved: &[String], answers: &[String], k: usize) -> f32 {
    let top: std::collections::HashSet<&String> = retrieved.iter().take(k).collect();
    let hits = answers.iter().filter(|a| top.contains(a)).count();
    hits as f32 / answers.len() as f32
}

fn mrr(retrieved: &[String], answers: &[String]) -> f32 {
    for (i, id) in retrieved.iter().enumerate() {
        if answers.contains(id) {
            return 1.0 / (i + 1) as f32;
        }
    }
    0.0
}
```

- [ ] **Step 2: Write the comparison test**

```rust
// tests/longmemeval_replay.rs — append
#[test]
fn cascade_rerank_matches_or_beats_fts_alone() {
    let db = Database::open_in_memory().unwrap();
    let corpus = load_fixture();
    seed(&db, &corpus).unwrap();

    let mut r5_baseline = 0.0_f32;
    let mut r5_rerank = 0.0_f32;
    let mut r10_baseline = 0.0_f32;
    let mut r10_rerank = 0.0_f32;
    let mut mrr_baseline = 0.0_f32;
    let mut mrr_rerank = 0.0_f32;

    for q in &corpus.questions {
        let b = fts_topk(&db, &q.prompt, 10);
        let r = fts_then_score_topk(&db, &q.prompt, 10);

        r5_baseline += recall_at_k(&b, &q.answer_note_ids, 5);
        r5_rerank += recall_at_k(&r, &q.answer_note_ids, 5);
        r10_baseline += recall_at_k(&b, &q.answer_note_ids, 10);
        r10_rerank += recall_at_k(&r, &q.answer_note_ids, 10);
        mrr_baseline += mrr(&b, &q.answer_note_ids);
        mrr_rerank += mrr(&r, &q.answer_note_ids);
    }
    let n = corpus.questions.len() as f32;
    let (r5b, r5r) = (r5_baseline / n, r5_rerank / n);
    let (r10b, r10r) = (r10_baseline / n, r10_rerank / n);
    let (mb, mr) = (mrr_baseline / n, mrr_rerank / n);

    eprintln!("Recall@5  baseline={r5b:.3}  rerank={r5r:.3}  Δ={:+.3}", r5r - r5b);
    eprintln!("Recall@10 baseline={r10b:.3}  rerank={r10r:.3}  Δ={:+.3}", r10r - r10b);
    eprintln!("MRR       baseline={mb:.3}  rerank={mr:.3}  Δ={:+.3}", mr - mb);

    // Floor: rerank must not regress more than 2 percentage points on either
    // metric. Strict ">" assertion would flake on a synthetic corpus this small.
    assert!(r5r >= r5b - 0.02, "rerank regressed Recall@5: {r5b} → {r5r}");
    assert!(mr >= mb - 0.02, "rerank regressed MRR: {mb} → {mr}");
}
```

- [ ] **Step 3: Run and capture numbers**

Run: `cargo test --test longmemeval_replay cascade_rerank_matches_or_beats_fts_alone -- --nocapture`
Expected: PASS, with `eprintln!` lines giving the actual numbers. Paste those numbers into the commit message and the paper draft note.

- [ ] **Step 4: Commit**

```bash
git add tests/longmemeval_replay.rs
git commit -m "test(longmemeval): synthetic recall harness with cascade rerank

50-question synthetic corpus across 12 sessions. Reports
Recall@5, Recall@10, and MRR for FTS5-only baseline vs
FTS5 + cascade-salience rerank.

Numbers (synthetic corpus, n=50, M-series Mac, in-memory SQLite):
  Recall@5  baseline=<X.XXX>  rerank=<Y.YYY>  Δ=<+Z.ZZZ>
  Recall@10 baseline=<X.XXX>  rerank=<Y.YYY>  Δ=<+Z.ZZZ>
  MRR       baseline=<X.XXX>  rerank=<Y.YYY>  Δ=<+Z.ZZZ>

Replace <…> with the actual numbers reported by the test run
before committing."
```

---

### Task 9: Document the harness + plug-in path for the real LongMemEval dataset

**Files:**
- Create: `tests/fixtures/README.md`

- [ ] **Step 1: Write the README**

```markdown
# Test fixtures

## `longmemeval_synthetic.json`

Hand-crafted synthetic corpus mirroring the structure of the public
LongMemEval benchmark (Hugging Face: `xiaowu0162/longmemeval`).

50 questions across 12 sessions: 25 single-hop, 15 multi-hop, 10
temporal-contradiction.

To plug in the real dataset:
1. Download `longmemeval_s.json` from the HF dataset card.
2. Convert to the schema in `longmemeval_synthetic.json` (a small
   transformer script — keep it out of the test runner so unit tests
   stay hermetic).
3. Point `tests/longmemeval_replay.rs::load_fixture` at the converted
   path via the `LONGMEMEVAL_FIXTURE` env var.

Why synthetic by default: the real dataset is ~6 GB and requires HF
auth. The synthetic version is debuggable, hermetic, and exercises the
same retrieval-shape patterns.
```

- [ ] **Step 2: Commit**

```bash
git add tests/fixtures/README.md
git commit -m "docs(test-fixtures): describe synthetic LongMemEval corpus + real-dataset plug-in path"
```

---

## Acceptance Criteria

After all tasks complete:
- `cargo test --all` passes (existing + 7 new tests across cascade and consolidation suites + the longmemeval integration test).
- `compute_score`'s only temporal input is `cascade_salience: f64`. No `access_count` or `last_accessed_at` parameter remains on the public signature.
- `tests/longmemeval_replay.rs` reports concrete Recall@5 / Recall@10 / MRR numbers for both retrieval strategies, with rerank ≥ baseline within 2pp on every metric.
- The fixture loader is structured so a future PR can swap in the real LongMemEval JSON with no changes to the harness code.

## Out of Scope (explicitly)

- Schema-formation / LLM-driven abstraction step (Task 9 Phase 3 in CLAUDE.md). Cascade salience drives *scoring* only; promotion still requires the embedding-cluster + LLM-summary pipeline that hasn't shipped.
- Changing the access-log writer. `log_access` continues to bump `access_count` and `last_accessed_at` so the audit trail and cascade-state reconstruction path stay intact — the scorer just no longer reads those columns.
- Exposing `w_salience` via `config.toml`. Defaults are hard-coded for v0.3; config-file plumbing tracks with the broader `Thresholds` exposure already on the queue.
- Real LongMemEval dataset run. Synthetic corpus is the v0.3 cut; real-dataset numbers belong in the paper §6.5 case study.

---

## Self-review notes (drafter)

- Spec coverage: Stage 1 covers "replace compute_score to use salience" via Tasks 1-5. Stage 2 covers "LongMemEval-style retrieval benchmark" via Tasks 6-9. ✓
- Type consistency check: `compute_score(f64, u32, f32, ScoreWeights)` matches across Tasks 3, 4, 8. `ScoreBreakdown` field set is identical in Task 3 (definition) and Task 5 (test access). ✓
- Placeholder scan: the only `<...>` placeholders are inside the commit message of Task 8 (intentional — actual numbers come from running the test). No "TBD"s, no "Add error handling", no "Similar to Task N". ✓
- Risk: `w_salience = 0.45` is a guess. Task 5 acknowledges this and points the tuning hook at Stage 2's recall numbers, not at vibes. ✓

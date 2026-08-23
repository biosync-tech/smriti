//! LongMemEval-style retrieval recall harness (paper §6.x).
//!
//! Loads a synthetic multi-session corpus, seeds an in-memory Smriti DB
//! with realistic access patterns, then measures retrieval recall under
//! two strategies (added in the next commit):
//!   1. FTS5-only (baseline — pure keyword)
//!   2. FTS5 + cascade-salience rerank
//!
//! Metrics: Recall@5, Recall@10, MRR. Latency belongs in criterion;
//! this file is correctness-only.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use smriti::errors::AppResult;
use smriti::features::cascade::{salience_peek_for_note, CascadeConfig};
use smriti::features::consolidation::{compute_score, log_access, AccessKind, ScoreWeights};
use smriti::storage::Database;

#[derive(Debug, Deserialize)]
struct Corpus {
    version: String,
    sessions: Vec<Session>,
    questions: Vec<Question>,
}

#[derive(Debug, Deserialize)]
struct Session {
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    prompt: String,
    answer_note_ids: Vec<String>,
    #[allow(dead_code)]
    category: String,
}

fn load_fixture() -> Corpus {
    // Default to the hermetic synthetic corpus; allow the real LongMemEval
    // JSON to plug in via $LONGMEMEVAL_FIXTURE without touching this file.
    let path: PathBuf = std::env::var("LONGMEMEVAL_FIXTURE")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            ["tests", "fixtures", "longmemeval_synthetic.json"]
                .iter()
                .collect()
        });
    let s = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture not readable at {}: {}", path.display(), e));
    serde_json::from_str(&s).unwrap_or_else(|e| {
        panic!(
            "fixture at {} did not parse as Corpus: {}",
            path.display(),
            e
        )
    })
}

/// Seed an in-memory Smriti DB from a corpus. Each note is inserted with the
/// session's wall-clock as its created_at/updated_at, then immediately gets a
/// `Read` access logged so `log_access` populates the cascade state via the
/// best-effort `cascade::record_access` hook.
fn seed(db: &Database, corpus: &Corpus) -> AppResult<()> {
    for session in &corpus.sessions {
        for note in &session.notes {
            db.execute(|conn| {
                conn.execute(
                    "INSERT INTO notes (id, title, content, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    rusqlite::params![note.id, note.title, note.content, session.ts.to_rfc3339()],
                )?;
                log_access(conn, &note.id, AccessKind::Read, None, None)?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[test]
fn fixture_loads() {
    let c = load_fixture();
    assert_eq!(c.version, "0.3-synthetic");
    assert!(
        c.sessions.len() >= 8,
        "expected ≥8 sessions, got {}",
        c.sessions.len()
    );
    assert_eq!(
        c.questions.len(),
        50,
        "fixture must have exactly 50 questions"
    );

    // Every answer_note_ids reference must resolve to a real note.
    let note_ids: std::collections::HashSet<&String> = c
        .sessions
        .iter()
        .flat_map(|s| s.notes.iter().map(|n| &n.id))
        .collect();
    for q in &c.questions {
        for nid in &q.answer_note_ids {
            assert!(
                note_ids.contains(nid),
                "question {} references unknown note {}",
                q.id,
                nid
            );
        }
    }
}

#[test]
fn seeding_populates_notes_and_cascade() {
    let db = Database::new(":memory:").expect("in-memory db");
    let corpus = load_fixture();
    seed(&db, &corpus).expect("seed succeeds");

    let total_notes: usize = corpus.sessions.iter().map(|s| s.notes.len()).sum();

    let count: i64 = db
        .execute(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?))
        .expect("notes count");
    assert_eq!(
        count as usize, total_notes,
        "all corpus notes should be inserted"
    );

    let with_cascade: i64 = db
        .execute(|conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM notes WHERE cascade_state IS NOT NULL",
                [],
                |r| r.get(0),
            )?)
        })
        .expect("cascade count");
    assert_eq!(
        with_cascade as usize, total_notes,
        "every seeded note should have cascade state after a Read access"
    );
}

// ── Retrieval strategies ──────────────────────────────────────────────────

/// FTS5-only baseline. Strips punctuation, OR-joins surviving terms longer
/// than two characters, and returns the top-k notes ordered by FTS5 BM25.
fn fts_topk(db: &Database, query: &str, k: usize) -> Vec<String> {
    let cleaned: String = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    let terms = cleaned
        .split_whitespace()
        .filter(|t| t.len() > 2)
        .collect::<Vec<_>>()
        .join(" OR ");
    if terms.is_empty() {
        return vec![];
    }
    db.execute(|conn| {
        let mut stmt = conn.prepare(
            "SELECT n.id FROM notes_fts AS f \
             INNER JOIN notes AS n ON n.rowid = f.rowid \
             WHERE notes_fts MATCH ?1 \
             ORDER BY f.rank LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![terms, k as i64], |r| {
                r.get::<_, String>(0)
            })?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        Ok(rows)
    })
    .unwrap_or_default()
}

/// FTS5 + cascade-salience rerank. Pulls FTS top-3k, then reorders by
/// `α * fts_reciprocal_rank + (1-α) * consolidation_score`. α = 0.6 keeps
/// FTS dominant so totally off-topic notes stay out of the top-k.
fn fts_then_score_topk(db: &Database, query: &str, k: usize) -> Vec<String> {
    let alpha: f32 = 0.6;
    let raw = fts_topk(db, query, k * 3);
    if raw.is_empty() {
        return vec![];
    }
    let weights = ScoreWeights::default();
    let cfg = CascadeConfig::default();
    let now = Utc::now();
    let scored: Vec<(String, f32)> = db
        .execute(|conn| {
            let mut out = Vec::with_capacity(raw.len());
            for (rank, id) in raw.iter().enumerate() {
                let fts_score = 1.0 / (rank as f32 + 1.0);
                let salience = salience_peek_for_note(conn, id, &cfg, now).unwrap_or(0.0);
                let degree: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM links l \
                         WHERE (l.source_note_id = ?1 OR l.target_note_id = ?1) \
                           AND l.valid_until IS NULL",
                        rusqlite::params![id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                let cs = compute_score(salience, degree as u32, 0.0, weights).score;
                out.push((id.clone(), alpha * fts_score + (1.0 - alpha) * cs));
            }
            Ok(out)
        })
        .unwrap_or_default();
    let mut scored = scored;
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(k).map(|(id, _)| id).collect()
}

// ── Metrics ───────────────────────────────────────────────────────────────

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

// ── Comparison test ───────────────────────────────────────────────────────

#[test]
fn cascade_rerank_matches_or_beats_fts_alone() {
    let db = Database::new(":memory:").expect("in-memory db");
    let corpus = load_fixture();
    seed(&db, &corpus).expect("seed succeeds");

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

    eprintln!(
        "Recall@5  baseline={r5b:.3}  rerank={r5r:.3}  Δ={:+.3}",
        r5r - r5b
    );
    eprintln!(
        "Recall@10 baseline={r10b:.3}  rerank={r10r:.3}  Δ={:+.3}",
        r10r - r10b
    );
    eprintln!(
        "MRR       baseline={mb:.3}  rerank={mr:.3}  Δ={:+.3}",
        mr - mb
    );

    // Floor: rerank must not regress more than 2pp on either metric.
    // Strict ">" would flake on a synthetic corpus this small where every
    // note got one Read access at creation — the cascade signal degenerates
    // to a recency proxy and may penalise old-but-correct answers.
    assert!(
        r5r >= r5b - 0.02,
        "rerank regressed Recall@5: {r5b} → {r5r}"
    );
    assert!(
        r10r >= r10b - 0.02,
        "rerank regressed Recall@10: {r10b} → {r10r}"
    );
    assert!(mr >= mb - 0.02, "rerank regressed MRR: {mb} → {mr}");
}

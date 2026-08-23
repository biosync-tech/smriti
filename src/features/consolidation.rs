//! Task 9 — Memory Consolidation + Schema Formation (CLS-inspired).
//!
//! The hippocampus replays recent episodes during rest; the neocortex
//! extracts statistical regularities into durable schemas. Smriti mirrors
//! this: frequently-replayed, structurally central notes get promoted to
//! `node_type = 'schema'`. Rarely-accessed notes are flagged for review —
//! never hard-deleted. ICH E6(R3) §4.1 requires the full audit trail to
//! remain reconstructable.
//!
//! Research refs:
//!   McClelland, McNaughton, O'Reilly 1995 — Psych Review vol 102 (CLS).
//!   Kumaran, Hassabis, McClelland 2016 — Trends in Cognitive Sciences.
//!
//! Do NOT frame this as "immune system memory" in public materials.
//! The mechanism is hippocampal-neocortical replay consolidation, not
//! immune clonal selection. Picking the wrong metaphor is a credibility
//! cost with healthcare buyers.
//!
//! Deliberate design choices (guard against regressions):
//!   * No division by age — old + replayed + central = schema. That's the point.
//!   * Scoring runs in a background pass, NOT on the request path.
//!   * Promotion requires an abstraction step (embedding cluster → LLM summary).
//!     This module handles the scoring + flagging only; `schema_formation.rs`
//!     (follow-up) owns the abstraction pipeline.
//!   * Archiving never deletes — it records a consolidation_event and flips
//!     state. Hard-delete is forbidden for compliance.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::AppResult;

// ── Types ─────────────────────────────────────────────────────────────────

/// Consolidation policy. Default is `Conservative` — safe for compliance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationPolicy {
    /// Never auto-promote, only flag low-score notes for review.
    #[default]
    Conservative,
    /// Promote clusters on threshold; flag low-score for review.
    Standard,
    /// Promote + auto-archive flagged notes past the grace period.
    Aggressive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessKind {
    Read,
    SearchHit,
    GraphTraverse,
    McpRetrieve,
}

impl AccessKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessKind::Read => "read",
            AccessKind::SearchHit => "search_hit",
            AccessKind::GraphTraverse => "graph_traverse",
            AccessKind::McpRetrieve => "mcp_retrieve",
        }
    }
}

/// Weights for the consolidation score components.
///
/// Defaults tuned for general use. Healthcare profiles should raise
/// `w_salience` — compliance artefacts that are low-frequency but
/// high-value will accumulate cascade mass at deep levels, so a higher
/// salience weight preserves them correctly without penalising idle time.
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

/// Full score breakdown. Surfaced by `smriti consolidate --explain <id>` for
/// transparency — agents, auditors, and humans can all see *why* a score
/// landed where it did.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub note_id: String,
    /// Cascade salience readout at scoring time (Benna-Fusi 2016 §3,
    /// weighted sum over u_0..u_{K-1}). Replaces legacy access_count
    /// + days_since_access as the temporal signal.
    pub cascade_salience: f64,
    pub degree: u32,
    pub context_diversity: f32,
    pub salience_component: f32,
    pub degree_component: f32,
    pub diversity_component: f32,
    pub raw_sum: f32,
    pub score: f32,
}

/// Thresholds for state transitions. Exposed via config.toml in the
/// follow-up PR; hard-coded defaults for now.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Thresholds {
    pub promote_above: f32,
    pub flag_below: f32,
    /// Grace period before an Aggressive policy archives a flagged note.
    pub archive_grace_days: i64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            promote_above: 0.80,
            flag_below: 0.15,
            archive_grace_days: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationRunReport {
    pub dry_run: bool,
    pub policy: ConsolidationPolicy,
    pub scanned: usize,
    pub promoted: Vec<String>,
    pub flagged: Vec<String>,
    pub archived: Vec<String>,
    /// note_id → human-readable reason (used by the audit endpoint).
    pub reasons: HashMap<String, String>,
}

// ── Scoring (pure, no I/O) ────────────────────────────────────────────────

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Compute consolidation score from cascade salience + structural + semantic
/// signals. Pure function — deterministic, unit-testable, no I/O.
///
/// Why salience and not access_count + recency: Benna & Fusi 2016 prove that
/// a multi-timescale cascade lifts memory capacity from O(√N) to O(N/log N)
/// vs a single-timescale leaky integrator. The cascade salience IS the
/// engineered temporal signal; mixing it with a separate exp(-Δt/τ) term
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

// ── Access logging ────────────────────────────────────────────────────────

/// Log an access event and bump the denormalised counters on `notes`.
///
/// Called from read handlers. For now this runs synchronously within the
/// caller's transaction — if that becomes a p50 problem, move the writes
/// behind a bounded tokio channel + worker task.
pub fn log_access(
    conn: &Connection,
    note_id: &str,
    kind: AccessKind,
    query_context: Option<&str>,
    agent_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO note_access_log
           (id, note_id, accessed_at, access_kind, query_context, agent_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, note_id, now, kind.as_str(), query_context, agent_id],
    )?;

    conn.execute(
        "UPDATE notes
         SET access_count = access_count + 1,
             last_accessed_at = ?1
         WHERE id = ?2",
        params![now, note_id],
    )?;

    // Best-effort cascade update (Migration 011, Benna-Fusi 2016).
    // We deliberately swallow errors here: the access log is the source of
    // truth and cascade state can be reconstructed from it. Failing the
    // access write because cascade serialisation hiccuped would be wrong.
    let _ = crate::features::cascade::record_access(
        conn,
        note_id,
        &crate::features::cascade::CascadeConfig::default(),
    );

    Ok(())
}

/// Recompute a score breakdown for a single note, reading live state from SQLite.
/// Used by `smriti consolidate --explain <note_id>`.
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
    let diversity = crate::features::schema_formation::context_diversity(conn, note_id)?;
    let mut b = compute_score(salience, degree as u32, diversity, weights);
    b.note_id = note_id.to_string();
    Ok(b)
}

// ── Consolidation pass ────────────────────────────────────────────────────

/// Run a consolidation pass across every episode-type note.
///
/// With `dry_run = true` (the default from the MCP tool): compute scores,
/// assemble the report, write nothing. Use this for CI-style audits or the
/// CLI's `smriti consolidate --dry-run`.
///
/// With `dry_run = false`: persist new `consolidation_score` values, write
/// `consolidation_events` rows for each flag/archive, and (under Aggressive)
/// archive flagged notes whose `last_accessed_at` is past the grace period.
///
/// This function does NOT promote episodes to schemas — that requires
/// embedding clustering + LLM abstraction, which lives in the follow-up
/// `schema_formation` module. Promotion-eligible notes are surfaced in
/// the report's `reasons` map so the abstraction pipeline can consume them.
pub fn run_consolidation_pass(
    conn: &Connection,
    policy: ConsolidationPolicy,
    dry_run: bool,
    weights: ScoreWeights,
    thresholds: Thresholds,
) -> AppResult<ConsolidationRunReport> {
    let now = Utc::now();
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

    let mut report = ConsolidationRunReport {
        dry_run,
        policy,
        scanned: 0,
        promoted: Vec::new(),
        flagged: Vec::new(),
        archived: Vec::new(),
        reasons: HashMap::new(),
    };
    let mut eligible_ids: Vec<String> = Vec::new();

    for row in rows {
        let (id, score_before, degree) = row?;
        report.scanned += 1;

        let salience =
            crate::features::cascade::salience_peek_for_note(conn, &id, &cascade_cfg, now)?;
        let diversity = crate::features::schema_formation::context_diversity(conn, &id)?;
        let breakdown = compute_score(salience, degree, diversity, weights);
        let new_score = breakdown.score;

        if !dry_run {
            conn.execute(
                "UPDATE notes SET consolidation_score = ?1 WHERE id = ?2",
                params![new_score as f64, id],
            )?;
        }

        // Flagging — all policies flag below-threshold scores.
        if new_score < thresholds.flag_below {
            report.flagged.push(id.clone());
            report.reasons.insert(
                id.clone(),
                format!(
                    "score {:.3} < flag_below {:.3} (salience={:.4} degree={})",
                    new_score, thresholds.flag_below, breakdown.cascade_salience, degree,
                ),
            );
            if !dry_run {
                write_event(
                    conn,
                    &id,
                    "flagged_for_review",
                    Some(score_before),
                    new_score,
                    &format!("below flag_below under {:?} policy", policy),
                )?;
            }

            // Aggressive archives past-grace items into the immutable audit trail.
            if policy == ConsolidationPolicy::Aggressive {
                let last_accessed_str: Option<String> = conn
                    .query_row(
                        "SELECT last_accessed_at FROM notes WHERE id = ?1",
                        params![id],
                        |r| r.get(0),
                    )
                    .optional()?
                    .flatten();
                let last_accessed = last_accessed_str.and_then(parse_rfc3339);
                let past_grace = last_accessed
                    .map(|t| now - t > Duration::days(thresholds.archive_grace_days))
                    .unwrap_or(true);
                if past_grace {
                    report.archived.push(id.clone());
                    if !dry_run {
                        archive_note(conn, &id)?;
                        write_event(
                            conn,
                            &id,
                            "archived",
                            Some(score_before),
                            new_score,
                            "aggressive policy: flagged + past grace period",
                        )?;
                    }
                }
            }
        }

        // Promotion eligibility. Actual schema formation (abstraction across
        // an embedding cluster) happens in a separate module; this just
        // records which episodes cleared the bar so the cluster pass can
        // pick them up.
        if new_score > thresholds.promote_above {
            eligible_ids.push(id.clone());
            if policy != ConsolidationPolicy::Conservative {
                report.reasons.insert(
                    id.clone(),
                    format!(
                        "score {:.3} > promote_above {:.3} — eligible for schema formation",
                        new_score, thresholds.promote_above,
                    ),
                );
            }
        }

        // Score-change audit even when nothing else happened — cheap, useful
        // for diffing runs and detecting drift.
        if !dry_run && (new_score - score_before).abs() > 0.01 {
            write_event(
                conn,
                &id,
                "score_recomputed",
                Some(score_before),
                new_score,
                "score changed > 0.01",
            )?;
        }
    }

    // CLS Phase 3 — cluster eligible episodes into schemas. Conservative
    // only flags. Standard/Aggressive write extractive schemas + lineage
    // (never deletes source episodes).
    let schema_cfg = crate::features::schema_formation::SchemaFormationConfig {
        mode: match policy {
            ConsolidationPolicy::Conservative => {
                crate::features::schema_formation::AbstractionMode::FlagOnly
            }
            ConsolidationPolicy::Standard | ConsolidationPolicy::Aggressive => {
                crate::features::schema_formation::AbstractionMode::Extractive
            }
        },
        ..crate::features::schema_formation::SchemaFormationConfig::default()
    };
    match crate::features::schema_formation::form_schemas(
        conn,
        &schema_cfg,
        dry_run,
        Some(&eligible_ids),
    ) {
        Ok(formed) => {
            for cluster in &formed.flagged {
                report.reasons.insert(
                    cluster
                        .member_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "cluster".into()),
                    format!(
                        "schema cluster of {} episodes (mean cosine {:.3})",
                        cluster.member_ids.len(),
                        cluster.mean_similarity
                    ),
                );
            }
            for schema in formed.created {
                report.promoted.push(schema.schema_id.clone());
                report.reasons.insert(
                    schema.schema_id,
                    format!(
                        "extractive schema over {} episodes: {}",
                        schema.source_ids.len(),
                        schema.title
                    ),
                );
            }
        }
        Err(e) => {
            tracing::warn!("schema formation skipped: {}", e);
        }
    }

    Ok(report)
}

// ── Internal helpers ──────────────────────────────────────────────────────

fn parse_rfc3339(s: String) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn write_event(
    conn: &Connection,
    note_id: &str,
    event_type: &str,
    score_before: Option<f32>,
    score_after: f32,
    reason: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO consolidation_events
           (id, note_id, event_type, score_before, score_after, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Uuid::new_v4().to_string(),
            note_id,
            event_type,
            score_before.map(|v| v as f64),
            score_after as f64,
            reason,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Archive a note. Compliance requires that the row itself stay — we only
/// flip state. A dedicated `note_history` snapshot table may be added later
/// if regulators require point-in-time reconstruction of content.
///
/// For now: archiving = writing the consolidation_events row (done by the
/// caller) + marking parent_schema_id as self as a sentinel for "archived".
/// This keeps every note fully queryable and auditable.
fn archive_note(conn: &Connection, note_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE notes SET parent_schema_id = id WHERE id = ?1 AND parent_schema_id IS NULL",
        params![note_id],
    )?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_zero_inputs_gives_half() {
        let w = ScoreWeights::default();
        let b = compute_score(0.0, 0, 0.0, w);
        assert!(
            (b.score - 0.5).abs() < 1e-3,
            "sigmoid(0) ≈ 0.5, got {}",
            b.score
        );
    }

    #[test]
    fn score_rises_monotonically_with_salience() {
        let w = ScoreWeights::default();
        let low = compute_score(0.1, 0, 0.0, w).score;
        let mid = compute_score(1.0, 0, 0.0, w).score;
        let high = compute_score(5.0, 0, 0.0, w).score;
        assert!(
            low < mid && mid < high,
            "expected monotonic: {low} {mid} {high}"
        );
    }

    #[test]
    fn score_combines_orthogonal_components() {
        let w = ScoreWeights::default();
        // salience=1.0 → salience_c = 0.45*1.0 = 0.45
        // diversity=1.0 → diversity_c = 0.30*1.0 = 0.30
        // degree=3      → degree_c = 0.25*ln(4) ≈ 0.346
        // All three components add independently to sigmoid input.
        let s_only = compute_score(1.0, 0, 0.0, w).score;
        let d_only = compute_score(0.0, 0, 1.0, w).score;
        let g_only = compute_score(0.0, 3, 0.0, w).score;
        let baseline = compute_score(0.0, 0, 0.0, w).score;
        // Each active component lifts score above baseline.
        assert!(
            s_only > baseline,
            "salience lifts score: {s_only} vs {baseline}"
        );
        assert!(
            d_only > baseline,
            "diversity lifts score: {d_only} vs {baseline}"
        );
        assert!(
            g_only > baseline,
            "degree lifts score: {g_only} vs {baseline}"
        );
        // Salience has the largest weight so it dominates.
        assert!(
            s_only > d_only,
            "salience(1) > diversity(1): {s_only} vs {d_only}"
        );
        assert!(
            s_only > g_only,
            "salience(1) > degree(3): {s_only} vs {g_only}"
        );
    }

    #[test]
    fn diversity_clamped_to_unit_interval() {
        let w = ScoreWeights::default();
        let a = compute_score(0.0, 0, -1.0, w);
        let b = compute_score(0.0, 0, 0.0, w);
        let c = compute_score(0.0, 0, 2.0, w);
        let d = compute_score(0.0, 0, 1.0, w);
        assert!((a.score - b.score).abs() < 1e-6);
        assert!((c.score - d.score).abs() < 1e-6);
    }

    #[test]
    fn breakdown_exposes_salience_component() {
        let w = ScoreWeights::default();
        let b = compute_score(2.0, 0, 0.0, w);
        assert!((b.salience_component - w.w_salience * 2.0).abs() < 1e-5);
        assert_eq!(b.cascade_salience, 2.0);
    }

    // ── Integration tests against a real (in-memory) database ────────────

    fn fresh_db() -> crate::storage::Database {
        crate::storage::Database::new(":memory:").expect("db")
    }

    fn insert_note(db: &crate::storage::Database, id: &str) {
        db.execute(|conn| {
            conn.execute(
                "INSERT INTO notes (id, title, content, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    "t",
                    "c",
                    Utc::now().to_rfc3339(),
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn log_access_bumps_counter_and_writes_row() {
        let db = fresh_db();
        insert_note(&db, "n1");

        db.execute(|conn| {
            log_access(conn, "n1", AccessKind::Read, Some("query"), Some("agent-a"))?;
            log_access(conn, "n1", AccessKind::McpRetrieve, None, None)?;
            Ok(())
        })
        .unwrap();

        db.execute(|conn| {
            let count: i64 =
                conn.query_row("SELECT access_count FROM notes WHERE id = 'n1'", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(count, 2);

            let logged: i64 = conn.query_row(
                "SELECT COUNT(*) FROM note_access_log WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(logged, 2);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn dry_run_does_not_mutate_state() {
        let db = fresh_db();
        insert_note(&db, "n1");

        let report = db
            .execute(|conn| {
                run_consolidation_pass(
                    conn,
                    ConsolidationPolicy::Standard,
                    true,
                    ScoreWeights::default(),
                    Thresholds::default(),
                )
            })
            .unwrap();

        assert_eq!(report.scanned, 1);

        db.execute(|conn| {
            let score: f64 = conn.query_row(
                "SELECT consolidation_score FROM notes WHERE id = 'n1'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(score, 0.0, "dry_run must not persist score");
            let events: i64 =
                conn.query_row("SELECT COUNT(*) FROM consolidation_events", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(events, 0, "dry_run must not write events");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn live_run_persists_score_and_emits_flag_event() {
        let db = fresh_db();
        insert_note(&db, "n1");

        // Force low score: a fresh note with no cascade state has salience=0.0,
        // giving sigmoid(0) = 0.5. Use a higher flag threshold to trigger a flag.
        let thresholds = Thresholds {
            flag_below: 0.6,
            ..Thresholds::default()
        };

        let report = db
            .execute(|conn| {
                run_consolidation_pass(
                    conn,
                    ConsolidationPolicy::Conservative,
                    false,
                    ScoreWeights::default(),
                    thresholds,
                )
            })
            .unwrap();

        assert_eq!(report.flagged.len(), 1);
        assert!(report.promoted.is_empty());

        db.execute(|conn| {
            let events: i64 = conn.query_row(
                "SELECT COUNT(*) FROM consolidation_events
                     WHERE event_type = 'flagged_for_review'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(events, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn explain_score_returns_cascade_salience_field() {
        let db = fresh_db();
        insert_note(&db, "n_explain");

        let breakdown = db
            .execute(|conn| explain_score(conn, "n_explain", ScoreWeights::default()))
            .unwrap();

        assert_eq!(breakdown.note_id, "n_explain");
        // No cascade state recorded yet — salience should be 0.0.
        assert_eq!(breakdown.cascade_salience, 0.0);
        assert!((breakdown.score - 0.5).abs() < 1e-3, "sigmoid(0) ≈ 0.5");
    }

    fn three_similar_embedded_notes(db: &crate::storage::Database) -> Vec<String> {
        use crate::models::note::CreateNoteRequest;
        let mut ids = Vec::new();
        for title in ["AE aspirin", "AE aspirin follow-up", "AE aspirin monitor"] {
            let n = db
                .create_note(CreateNoteRequest {
                    title: title.into(),
                    content: "patient continues aspirin 81mg daily".into(),
                    tags: vec![],
                })
                .unwrap();
            db.store_embedding(&n.id, &vec![1.0; 384], Some("test"))
                .unwrap();
            ids.push(n.id);
        }
        ids
    }

    #[test]
    fn standard_policy_does_not_schema_episodes_below_promote_above() {
        let db = fresh_db();
        let ids = three_similar_embedded_notes(&db);
        // Fresh notes score ≈ 0.5. Default promote_above is 0.80.
        let report = db
            .execute(|conn| {
                run_consolidation_pass(
                    conn,
                    ConsolidationPolicy::Standard,
                    false,
                    ScoreWeights::default(),
                    Thresholds::default(),
                )
            })
            .unwrap();

        assert!(
            report.promoted.is_empty(),
            "must not create schemas from notes below promote_above: {:?}",
            report.promoted
        );
        for id in &ids {
            let parent: Option<String> = db
                .execute(|conn| {
                    Ok(conn.query_row(
                        "SELECT parent_schema_id FROM notes WHERE id = ?1",
                        params![id],
                        |r| r.get(0),
                    )?)
                })
                .unwrap();
            assert!(
                parent.is_none(),
                "episode {id} was subsumed despite score below promote_above"
            );
        }
    }

    #[test]
    fn standard_policy_schemas_only_episodes_above_promote_above() {
        let db = fresh_db();
        let ids = three_similar_embedded_notes(&db);
        let thresholds = Thresholds {
            promote_above: 0.10,
            flag_below: 0.0,
            ..Thresholds::default()
        };
        let report = db
            .execute(|conn| {
                run_consolidation_pass(
                    conn,
                    ConsolidationPolicy::Standard,
                    false,
                    ScoreWeights::default(),
                    thresholds,
                )
            })
            .unwrap();

        assert_eq!(
            report.promoted.len(),
            1,
            "eligible cluster should form one schema"
        );
        for id in &ids {
            let parent: Option<String> = db
                .execute(|conn| {
                    Ok(conn.query_row(
                        "SELECT parent_schema_id FROM notes WHERE id = ?1",
                        params![id],
                        |r| r.get(0),
                    )?)
                })
                .unwrap();
            assert_eq!(parent.as_deref(), Some(report.promoted[0].as_str()));
        }
    }
}

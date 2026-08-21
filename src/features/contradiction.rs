//! Contradiction Detection — confidence-weighted inbox for human review.
//!
//! Smriti NEVER auto-resolves contradictions. It surfaces candidate pairs
//! with a composite confidence score and leaves resolution to a human or
//! a higher-level agent.
//!
//! Research anchors:
//!   - MemoTime              arXiv:2510.13614  (memory-augmented temporal KG reasoning)
//!   - EvoReasoner/EvoKG     arXiv:2509.15464  (confidence-based conflict resolution)
//!   - AGM (Alchourrón et al)               (belief revision postulates; J.Symb.Logic 1985)
//!   - Graph-Native AGM      arXiv:2603.17244  (AGM postulates over typed graphs)
//!   - FACTUM                arXiv:2601.05866  (provenance-grounded claim verification)
//!   - Citation-Grounded     arXiv:2512.12117  (claim-span structural grounding)
//!   - Zep / Graphiti        arXiv:2501.13956  (bi-temporal edges for time-aware retrieval)
//!
//! Combined score: `s = w1 · semantic + w2 · recency + w3 · authority`
//!
//! Where each component is in [0, 1]:
//!   * semantic  — cosine similarity of note contents (or text overlap proxy)
//!   * recency   — exponential decay on Δt between the two notes
//!   * authority — source verification score from claim_spans (partial credit
//!     when only one side is grounded; see `pair_authority_score`)
//!
//! ## Why polarity-token gating alone is not enough (Bug #4)
//!
//! The original `polarity_conflict()` filter required one note to contain a
//! literal negation token (`not `, `no `, `cannot `, ...) and the other not
//! to. That captures medical-decision toggles ("aspirin is safe" vs
//! "aspirin is not safe") but misses the most common silent-overwrite
//! shape in regulated work: **same conclusion, different cited authority**.
//!
//! Example: two notes claim Subject 14 is ELIGIBLE; one cites Protocol
//! v2.1, the other cites Protocol v2.3. No polarity reversal, but a
//! genuine attribution conflict that ICH E6(R3) §4.2.1 explicitly forbids.
//!
//! The Duhem-Quine thesis (Quine, "Two Dogmas of Empiricism" 1951)
//! formalises this: the truth value of a single proposition is conditional
//! on a web of cited beliefs. Two propositions sharing surface polarity
//! can still be inconsistent at the web level when they are grounded in
//! mutually-exclusive sources. Lexical-feature gates miss web-level
//! conflict by construction.
//!
//! Fix: augment polarity gating with a **cite-disagreement** signal that
//! fires when both notes are grounded (each has ≥1 claim_span), the notes
//! have substantive semantic overlap, and their cited source sets are not
//! identical. This is structural — no NLP, no entity extraction, just SQL
//! over the existing `claim_spans` table. It generalises EvoKG §4.1
//! authority weighting from a *score component* to a *gate component*.

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::AppResult;
use crate::storage::db::Database;

/// Default mixing weights. Sum does not have to equal 1.0; the score is
/// clamped at the end. These defaults match EvoKG §4.1 ablation.
pub const DEFAULT_W_SEMANTIC: f64 = 0.50;
pub const DEFAULT_W_RECENCY: f64 = 0.20;
pub const DEFAULT_W_AUTHORITY: f64 = 0.30;

/// Minimum combined score for a pair to land in the review inbox.
pub const DEFAULT_CONTRADICTION_THRESHOLD: f64 = 0.60;

/// Exponential recency decay: τ in days. After τ days the recency weight
/// drops to 1/e ≈ 0.37.
pub const RECENCY_TAU_DAYS: f64 = 14.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionEvent {
    pub id: String,
    pub note_id_a: String,
    pub note_id_b: String,
    pub semantic_score: f64,
    pub recency_score: f64,
    pub authority_score: f64,
    pub combined_score: f64,
    pub status: String,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ContradictionConfig {
    pub w_semantic: f64,
    pub w_recency: f64,
    pub w_authority: f64,
    pub threshold: f64,
    pub recency_tau_days: f64,
}

impl Default for ContradictionConfig {
    fn default() -> Self {
        Self {
            w_semantic: DEFAULT_W_SEMANTIC,
            w_recency: DEFAULT_W_RECENCY,
            w_authority: DEFAULT_W_AUTHORITY,
            threshold: DEFAULT_CONTRADICTION_THRESHOLD,
            recency_tau_days: RECENCY_TAU_DAYS,
        }
    }
}

/// Composite score inputs for a single pair.
#[derive(Debug, Clone, Copy)]
pub struct PairSignals {
    pub semantic: f64,
    pub recency: f64,
    pub authority: f64,
}

/// Compute the mixed score. Each input is clamped to [0, 1]; the result is
/// a normalized weighted average (weights renormalized so w1+w2+w3 = 1).
pub fn score(signals: PairSignals, cfg: ContradictionConfig) -> f64 {
    let s = signals.semantic.clamp(0.0, 1.0);
    let r = signals.recency.clamp(0.0, 1.0);
    let a = signals.authority.clamp(0.0, 1.0);
    let w = cfg.w_semantic + cfg.w_recency + cfg.w_authority;
    if w <= 0.0 {
        return 0.0;
    }
    (cfg.w_semantic * s + cfg.w_recency * r + cfg.w_authority * a) / w
}

/// Recency score based on the age gap between two notes.
pub fn recency_score(
    updated_a: DateTime<Utc>,
    updated_b: DateTime<Utc>,
    tau_days: f64,
) -> f64 {
    let delta = (updated_a - updated_b).num_seconds().abs() as f64;
    let delta_days = delta / 86_400.0;
    (-delta_days / tau_days.max(f64::MIN_POSITIVE)).exp()
}

/// Quick token-set Jaccard similarity used as the semantic proxy when no
/// embedding is available. Returns a value in [0, 1].
pub fn token_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let norm = |s: &str| -> HashSet<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect()
    };
    let ta = norm(a);
    let tb = norm(b);
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let inter = ta.intersection(&tb).count() as f64;
    let union = ta.union(&tb).count() as f64;
    inter / union
}

/// Heuristic contradiction hint: two notes look like they *might* contradict
/// if they share a substantive topic overlap (semantic > 0.4) but contain
/// opposing polarity markers. This is a deliberately conservative filter —
/// we prefer to miss candidates than to spam the inbox with false positives.
pub fn polarity_conflict(a: &str, b: &str) -> bool {
    let negations = ["not ", "no ", "never ", "cannot ", "isn't ", "doesn't ", "won't "];
    let la = a.to_lowercase();
    let lb = b.to_lowercase();
    let a_neg = negations.iter().any(|n| la.contains(n));
    let b_neg = negations.iter().any(|n| lb.contains(n));
    a_neg != b_neg
}

impl Database {
    /// Record a candidate contradiction. Idempotent on (note_id_a, note_id_b).
    pub fn record_contradiction(
        &self,
        note_id_a: &str,
        note_id_b: &str,
        signals: PairSignals,
        cfg: ContradictionConfig,
    ) -> AppResult<Option<ContradictionEvent>> {
        // Canonicalize pair order to make UNIQUE work regardless of argument order.
        let (a, b) = if note_id_a <= note_id_b {
            (note_id_a, note_id_b)
        } else {
            (note_id_b, note_id_a)
        };
        let combined = score(signals, cfg);
        if combined < cfg.threshold {
            return Ok(None);
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let conn = self.conn.lock()
            .map_err(|e| crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e)))?;
        conn.execute(
            "INSERT OR IGNORE INTO contradiction_events
             (id, note_id_a, note_id_b, semantic_score, recency_score,
              authority_score, combined_score, status, detected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8)",
            params![
                id,
                a,
                b,
                signals.semantic,
                signals.recency,
                signals.authority,
                combined,
                now.to_rfc3339(),
            ],
        )?;

        Ok(Some(ContradictionEvent {
            id,
            note_id_a: a.to_string(),
            note_id_b: b.to_string(),
            semantic_score: signals.semantic,
            recency_score: signals.recency,
            authority_score: signals.authority,
            combined_score: combined,
            status: "open".to_string(),
            detected_at: now,
            resolved_at: None,
            resolution: None,
        }))
    }

    /// List open contradictions, highest-confidence first.
    pub fn list_open_contradictions(&self, limit: i64) -> AppResult<Vec<ContradictionEvent>> {
        let conn = self.conn.lock()
            .map_err(|e| crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, note_id_a, note_id_b, semantic_score, recency_score,
                    authority_score, combined_score, status, detected_at,
                    resolved_at, resolution
             FROM contradiction_events
             WHERE status = 'open'
             ORDER BY combined_score DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], |row| {
                let detected: String = row.get(8)?;
                let resolved: Option<String> = row.get(9)?;
                Ok(ContradictionEvent {
                    id: row.get(0)?,
                    note_id_a: row.get(1)?,
                    note_id_b: row.get(2)?,
                    semantic_score: row.get(3)?,
                    recency_score: row.get(4)?,
                    authority_score: row.get(5)?,
                    combined_score: row.get(6)?,
                    status: row.get(7)?,
                    detected_at: DateTime::parse_from_rfc3339(&detected)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    resolved_at: resolved.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .map(|d| d.with_timezone(&Utc))
                            .ok()
                    }),
                    resolution: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Resolve a contradiction with a human-provided rationale.
    pub fn resolve_contradiction(&self, id: &str, resolution: &str) -> AppResult<()> {
        let conn = self.conn.lock()
            .map_err(|e| crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e)))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE contradiction_events
             SET status = 'resolved', resolved_at = ?1, resolution = ?2
             WHERE id = ?3",
            params![now, resolution, id],
        )?;
        Ok(())
    }

    /// Scan pairs of recent notes, compute signals, and persist candidates
    /// above threshold. Intended to be called from a background job.
    ///
    /// `scan_limit` caps the number of notes fetched; pairwise cost is O(n²).
    /// Keep it small (e.g. 50) for the default sweeper.
    pub fn detect_contradictions(
        &self,
        scan_limit: i64,
        cfg: ContradictionConfig,
    ) -> AppResult<Vec<ContradictionEvent>> {
        let rows: Vec<(String, String, DateTime<Utc>)> = {
            let conn = self.conn.lock()
                .map_err(|e| crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e)))?;
            let mut stmt = conn.prepare(
                "SELECT id, content, updated_at FROM notes
                 ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let collected: Vec<(String, String, DateTime<Utc>)> = stmt
                .query_map(params![scan_limit], |row| {
                    let updated: String = row.get(2)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        DateTime::parse_from_rfc3339(&updated)
                            .map(|d| d.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            collected
        };

        let mut out = Vec::new();
        for i in 0..rows.len() {
            for j in (i + 1)..rows.len() {
                let (ref ida, ref ca, ua) = rows[i];
                let (ref idb, ref cb, ub) = rows[j];
                let sem = token_similarity(ca, cb);
                if sem < 0.35 {
                    continue;
                }

                // ── Bug #4 fix: structural conflict signals ────────
                //
                // The pair survives the gate if EITHER:
                //   (a) Polarity-token conflict (legacy lexical heuristic),
                //       OR
                //   (b) Cite-disagreement: both notes are grounded but
                //       cite different sources on substantively similar
                //       content. This is the structural FACTUM/AGM-style
                //       conflict that survives surface-polarity matching
                //       (Quine 1951; Citation-Grounded arXiv:2512.12117).
                //
                // Either signal alone is sufficient. (b) does not require
                // any NLP — it is a pure SQL set-difference over the
                // claim_spans table.
                let polarity = polarity_conflict(ca, cb);
                let cite_disagreement = self
                    .cite_disagreement(ida, idb)
                    .unwrap_or(false);
                if !polarity && !cite_disagreement {
                    continue;
                }

                let rec = recency_score(ua, ub, cfg.recency_tau_days);
                // Authority: weighted average of claim_span verification
                // scores. Now gives partial credit when only one side is
                // grounded (see fix in `pair_authority_score`).
                let auth = self.pair_authority_score(ida, idb).unwrap_or(0.0);
                let signals = PairSignals {
                    semantic: sem,
                    recency: rec,
                    authority: auth,
                };
                if let Some(ev) = self.record_contradiction(ida, idb, signals, cfg)? {
                    out.push(ev);
                }
            }
        }
        Ok(out)
    }

    /// Average claim-span verification score across the pair.
    ///
    /// Bug #4 sub-fix: previously returned 0 unless **both** notes had
    /// claim spans. That zeroed the authority weight (30% of the combined
    /// score) for any grounded-vs-ungrounded pair, which is exactly the
    /// "draft vs canonical" shape we most want to surface.
    ///
    /// New behaviour: average over whichever sides have claims; partial
    /// credit when only one side is grounded. Aligns with the AGM
    /// "primacy of new information" postulate (Alchourrón, Gärdenfors,
    /// Makinson 1985 §4) — a grounded incoming claim should still
    /// dominate an ungrounded incumbent.
    fn pair_authority_score(&self, note_a: &str, note_b: &str) -> AppResult<f64> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!(
                "Failed to lock connection: {}",
                e
            ))
        })?;
        let avg_a: Option<f64> = conn
            .query_row(
                "SELECT AVG(verification_score) FROM claim_spans WHERE note_id = ?1",
                params![note_a],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        let avg_b: Option<f64> = conn
            .query_row(
                "SELECT AVG(verification_score) FROM claim_spans WHERE note_id = ?1",
                params![note_b],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        match (avg_a, avg_b) {
            (Some(a), Some(b)) => Ok((a + b) / 2.0),
            // Partial credit: a grounded note's authority partially
            // contributes even when paired with an ungrounded one.
            // The factor of 0.5 reflects "half the pair is verifiable."
            (Some(a), None) => Ok(a * 0.5),
            (None, Some(b)) => Ok(b * 0.5),
            _ => Ok(0.0),
        }
    }

    /// Returns true when:
    ///   1. Both notes have at least one claim_span (i.e., both grounded),
    ///   2. The two notes' cited source_id sets are not identical.
    ///
    /// Rationale: two grounded notes whose claim-source sets disagree
    /// represent a structural authority conflict — the same proposition
    /// ascribed to different authorities — independent of any lexical
    /// negation. This is the canonical shape of regulated-work conflicts
    /// (e.g., Subject 14 ELIGIBLE per v2.1 vs ELIGIBLE per v2.3).
    ///
    /// The check is intentionally cheap: SQL set difference, no joins
    /// across content. Recall trades off against precision through the
    /// caller's semantic threshold (`token_similarity ≥ 0.35`) — pairs
    /// must already share substantive surface content to be considered.
    fn cite_disagreement(&self, note_a: &str, note_b: &str) -> AppResult<bool> {
        use std::collections::HashSet;
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!(
                "Failed to lock connection: {}",
                e
            ))
        })?;
        let collect = |note_id: &str| -> AppResult<HashSet<String>> {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT source_id FROM claim_spans WHERE note_id = ?1",
            )?;
            let rows = stmt.query_map(params![note_id], |row| row.get::<_, String>(0))?;
            let mut set = HashSet::new();
            for row in rows {
                set.insert(row?);
            }
            Ok(set)
        };
        let sa = collect(note_a)?;
        let sb = collect(note_b)?;
        if sa.is_empty() || sb.is_empty() {
            // Need at least one grounded side on each note for the
            // structural signal to be meaningful. Otherwise we'd
            // false-positive every "plain note paired with grounded
            // note" — that's a different (less specific) signal.
            return Ok(false);
        }
        Ok(sa != sb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_weighted_average_is_normalized() {
        let cfg = ContradictionConfig::default();
        let s = score(
            PairSignals {
                semantic: 1.0,
                recency: 1.0,
                authority: 1.0,
            },
            cfg,
        );
        assert!((s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recency_decays_exponentially() {
        let a = Utc::now();
        let b = a - chrono::Duration::days(14);
        let r = recency_score(a, b, 14.0);
        assert!((r - (-1.0f64).exp()).abs() < 1e-6);
    }

    #[test]
    fn token_similarity_handles_basic_cases() {
        assert!(token_similarity("foo bar baz", "foo bar baz") > 0.99);
        assert_eq!(token_similarity("foo", "bar"), 0.0);
        assert!(token_similarity("", "anything") == 0.0);
    }

    #[test]
    fn polarity_conflict_detects_negation_mismatch() {
        assert!(polarity_conflict(
            "Aspirin is safe during pregnancy",
            "Aspirin is not safe during pregnancy"
        ));
        assert!(!polarity_conflict("X is Y", "X is Y"));
    }

    // ─── Bug #4 regression: cite-disagreement signal ────────────────
    //
    // Pre-fix: two notes claiming the same conclusion (e.g. "Subject
    // ELIGIBLE") with different cited authorities (Protocol v2.1 vs
    // v2.3) were skipped at the polarity gate before scoring. Post-fix:
    // structural cite-disagreement triggers the gate independently of
    // polarity tokens.

    use crate::features::wiki_transaction::{ClaimInlineRequest, SubmitTransactionRequest, WikiOp};

    #[test]
    fn cite_disagreement_surfaces_same_polarity_authority_conflict() {
        let db = Database::new(":memory:").unwrap();

        // Two distinct sources representing two protocol versions.
        // Use require_provenance: false because we want to validate
        // the gate itself, not the provenance pipeline.
        let req = SubmitTransactionRequest {
            agent_id: "test".into(),
            operations: vec![
                WikiOp::UpsertSource {
                    uri: "protocol://v2.1".into(),
                    title: Some("Protocol v2.1".into()),
                    content: "Subject is eligible per Protocol v2.1 inclusion criteria".into(),
                },
                WikiOp::UpsertSource {
                    uri: "protocol://v2.3".into(),
                    title: Some("Protocol v2.3".into()),
                    content: "Subject is eligible per Protocol v2.3 inclusion criteria".into(),
                },
                WikiOp::CreateNote {
                    title: "Note A v2.1".into(),
                    content: "Subject is eligible per Protocol v2.1 inclusion criteria".into(),
                    tags: vec![],
                    claims: vec![ClaimInlineRequest {
                        claim_start: 0,
                        claim_end: 56,
                        source_id: None,
                        source_uri: Some("protocol://v2.1".into()),
                        source_content: Some(
                            "Subject is eligible per Protocol v2.1 inclusion criteria".into(),
                        ),
                        source_span_start: None,
                        source_span_end: None,
                    }],
                },
                WikiOp::CreateNote {
                    title: "Note B v2.3".into(),
                    content: "Subject is eligible per Protocol v2.3 inclusion criteria".into(),
                    tags: vec![],
                    claims: vec![ClaimInlineRequest {
                        claim_start: 0,
                        claim_end: 56,
                        source_id: None,
                        source_uri: Some("protocol://v2.3".into()),
                        source_content: Some(
                            "Subject is eligible per Protocol v2.3 inclusion criteria".into(),
                        ),
                        source_span_start: None,
                        source_span_end: None,
                    }],
                },
            ],
            rationale: None,
            pending: false,
            require_provenance: false,
        };
        db.submit_wiki_transaction(req).unwrap();

        // Both notes have NO negation tokens, identical surface
        // polarity ("is eligible"). Pre-fix this would surface zero
        // candidates; post-fix the cite-disagreement gate fires.
        let events = db
            .detect_contradictions(50, ContradictionConfig::default())
            .unwrap();
        assert!(
            !events.is_empty(),
            "cite-disagreement on grounded notes must surface a candidate \
             without requiring polarity tokens (Bug #4 regression)"
        );
        // Sanity: authority weight contributes to the score.
        assert!(
            events.iter().any(|e| e.authority_score > 0.0),
            "authority signal must be non-zero when both notes are grounded"
        );
    }

    #[test]
    fn cite_disagreement_skips_when_only_one_side_grounded() {
        let db = Database::new(":memory:").unwrap();
        // Asymmetric grounding should NOT trigger cite-disagreement;
        // it would false-positive every plain-vs-grounded pair.
        // (Polarity-token gate still applies if applicable.)
        let req = SubmitTransactionRequest {
            agent_id: "test".into(),
            operations: vec![
                WikiOp::UpsertSource {
                    uri: "src://only".into(),
                    title: Some("Only Source".into()),
                    content: "Content goes here for grounding".into(),
                },
                WikiOp::CreateNote {
                    title: "Grounded".into(),
                    content: "Subject is eligible per the protocol".into(),
                    tags: vec![],
                    claims: vec![ClaimInlineRequest {
                        claim_start: 0,
                        claim_end: 36,
                        source_id: None,
                        source_uri: Some("src://only".into()),
                        source_content: Some("Subject is eligible per the protocol".into()),
                        source_span_start: None,
                        source_span_end: None,
                    }],
                },
                WikiOp::CreateNote {
                    title: "Ungrounded".into(),
                    content: "Subject is eligible per the protocol".into(),
                    tags: vec![],
                    claims: vec![],
                },
            ],
            rationale: None,
            pending: false,
            require_provenance: false,
        };
        db.submit_wiki_transaction(req).unwrap();

        let events = db
            .detect_contradictions(50, ContradictionConfig::default())
            .unwrap();
        // No polarity conflict + asymmetric grounding → no candidates.
        // (This guards against precision regressions from the new gate.)
        assert!(
            events.is_empty(),
            "asymmetric grounding alone must not surface a contradiction \
             without polarity disagreement"
        );
    }
}

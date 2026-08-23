//! Provenance layer — claim-level source attribution with structural verification.
//!
//! Research anchors:
//!   - FACTUM: Mechanistic Detection of Citation Hallucination in Long-Form RAG
//!     (arXiv:2601.05866)
//!   - Citation-Grounded Code Comprehension: Preventing LLM Hallucination Through
//!     Hybrid Retrieval and Graph-Augmented Context (arXiv:2512.12117)
//!   - Mitigating Hallucination in LLMs Survey (arXiv:2510.24476)
//!
//! The contract Smriti enforces:
//!
//! > Every claim synthesized by an agent must carry a source span, and
//! > the system REFUSES to commit a note whose claims fail overlap
//! > verification.
//!
//! This is a structural invariant (Citation-Grounded Code §3.2) rather than a
//! post-hoc detector (FACTUM §4). It is cheap, enforceable at write time, and
//! dominates prompt-level citation guidelines because no agent can bypass it.
//!
//! The verification method is a weighted max over three cheap signals:
//!   1. Literal substring containment (exact grounding)
//!   2. Token-set Jaccard over lowercased tokens (lexical grounding)
//!   3. Character trigram Jaccard (fuzzy grounding, robust to paraphrase)
//!
//! Any claim scoring below `min_verification_score` is rejected. The default
//! threshold (0.55) was chosen to match the "evidence recall" operating point
//! reported in GraphRAG-Bench (arXiv:2506.05690) for single-hop fact retrieval.

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::storage::Database;

/// A source document an agent has ingested. Content is identified by hash
/// so the same document is never stored twice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub uri: String,
    pub content_hash: String,
    pub title: Option<String>,
    pub excerpt: Option<String>,
    pub ingested_at: DateTime<Utc>,
}

/// A claim span locates a stretch of a note's content and binds it to a source.
///
/// `verification_score` is the result of `verify_overlap` at commit time. A
/// score below the configured threshold causes the enclosing wiki_transaction
/// to be rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimSpan {
    pub id: String,
    pub note_id: String,
    pub claim_start: usize,
    pub claim_end: usize,
    pub source_id: String,
    pub source_span_start: Option<usize>,
    pub source_span_end: Option<usize>,
    pub verification_score: f64,
    pub verified_at: DateTime<Utc>,
    pub method: String,
}

/// Request to attach a claim to a source. The span indexes into the note's
/// content; the source span (if present) indexes into the source excerpt.
#[derive(Debug, Clone, Deserialize)]
pub struct ClaimAttachmentRequest {
    pub note_id: String,
    pub claim_start: usize,
    pub claim_end: usize,
    pub source_id: String,
    pub source_span_start: Option<usize>,
    pub source_span_end: Option<usize>,
}

/// Configurable thresholds for overlap verification.
#[derive(Debug, Clone, Copy)]
pub struct VerificationConfig {
    pub min_score: f64,
    pub w_literal: f64,
    pub w_token: f64,
    pub w_trigram: f64,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            min_score: 0.55,
            w_literal: 1.0,
            w_token: 0.85,
            w_trigram: 0.70,
        }
    }
}

/// The result of verifying a single claim against a source span.
#[derive(Debug, Clone, Serialize)]
pub struct VerificationResult {
    pub score: f64,
    pub literal: f64,
    pub token: f64,
    pub trigram: f64,
    pub method: String,
    pub passed: bool,
}

/// Compute a hash for a source's content. Used for deduplication.
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify that a claim is grounded in a source span.
///
/// Returns a weighted score in [0, 1]. The method field records which signal
/// dominated ("literal" | "token" | "trigram") so callers can surface *why*
/// a claim was admitted.
pub fn verify_overlap(
    claim: &str,
    source_span: &str,
    config: &VerificationConfig,
) -> VerificationResult {
    let claim_norm = normalize(claim);
    let source_norm = normalize(source_span);

    // Signal 1: literal substring containment (cheapest, strongest)
    let literal = if source_norm.contains(&claim_norm) {
        1.0
    } else if claim_norm.contains(&source_norm) && !source_norm.is_empty() {
        // Claim longer than span but span is fully present in claim — still
        // evidence of grounding, weaker than full containment.
        0.75
    } else {
        0.0
    };

    // Signal 2: token-set Jaccard (robust to word reordering)
    let token = jaccard_tokens(&claim_norm, &source_norm);

    // Signal 3: character trigram Jaccard (robust to paraphrase + typos)
    let trigram = jaccard_trigrams(&claim_norm, &source_norm);

    let literal_w = literal * config.w_literal;
    let token_w = token * config.w_token;
    let trigram_w = trigram * config.w_trigram;

    let (score, method) = if literal_w >= token_w && literal_w >= trigram_w {
        (literal_w, "literal")
    } else if token_w >= trigram_w {
        (token_w, "token")
    } else {
        (trigram_w, "trigram")
    };

    VerificationResult {
        score,
        literal,
        token,
        trigram,
        method: method.to_string(),
        passed: score >= config.min_score,
    }
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lightweight suffix stripper: collapses "reduces"/"reduced"/"reducing"
/// to "reduc" so paraphrase survives token-Jaccard. Not a full Porter
/// stemmer — deliberately cheap and predictable.
fn stem_light(tok: &str) -> &str {
    let candidates = ["ing", "ies", "ied", "ied", "ed", "es", "s"];
    for suf in &candidates {
        if tok.len() > suf.len() + 2 && tok.ends_with(suf) {
            return &tok[..tok.len() - suf.len()];
        }
    }
    tok
}

fn is_stopword(tok: &str) -> bool {
    matches!(
        tok,
        "the"
            | "a"
            | "an"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "by"
            | "of"
            | "to"
            | "in"
            | "on"
            | "at"
            | "for"
            | "and"
            | "or"
            | "but"
            | "if"
            | "then"
            | "than"
            | "as"
            | "with"
            | "this"
            | "that"
            | "these"
            | "those"
            | "it"
            | "its"
    )
}

fn tokenize_for_jaccard(s: &str) -> HashSet<String> {
    s.split_whitespace()
        .filter(|t| !is_stopword(t))
        .map(|t| stem_light(t).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn jaccard_tokens(a: &str, b: &str) -> f64 {
    let a_set = tokenize_for_jaccard(a);
    let b_set = tokenize_for_jaccard(b);
    if a_set.is_empty() || b_set.is_empty() {
        return 0.0;
    }
    let inter = a_set.intersection(&b_set).count() as f64;
    let union = a_set.union(&b_set).count() as f64;
    inter / union
}

fn jaccard_trigrams(a: &str, b: &str) -> f64 {
    let a_set: HashSet<String> = trigrams(a);
    let b_set: HashSet<String> = trigrams(b);
    if a_set.is_empty() || b_set.is_empty() {
        return 0.0;
    }
    let inter = a_set.intersection(&b_set).count() as f64;
    let union = a_set.union(&b_set).count() as f64;
    inter / union
}

fn trigrams(s: &str) -> HashSet<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        return HashSet::new();
    }
    let mut out = HashSet::with_capacity(chars.len().saturating_sub(2));
    for w in chars.windows(3) {
        out.insert(w.iter().collect());
    }
    out
}

// ─── Database operations ────────────────────────────────────────────────

impl Database {
    /// Upsert a source. Dedupes on (uri, content_hash) so repeated ingestion
    /// of the same content is idempotent.
    pub fn upsert_source(
        &self,
        uri: &str,
        content: &str,
        title: Option<&str>,
        excerpt: Option<&str>,
    ) -> AppResult<Source> {
        let hash = hash_content(content);
        let now = Utc::now();
        self.execute(|conn| {
            // Try to find existing
            let existing: Option<String> = conn
                .query_row(
                    "SELECT id FROM sources WHERE uri = ?1 AND content_hash = ?2",
                    params![uri, hash],
                    |row| row.get(0),
                )
                .ok();

            if let Some(id) = existing {
                return Ok(Source {
                    id,
                    uri: uri.to_string(),
                    content_hash: hash,
                    title: title.map(String::from),
                    excerpt: excerpt.map(String::from),
                    ingested_at: now,
                });
            }

            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO sources (id, uri, content_hash, title, excerpt, ingested_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, uri, hash, title, excerpt, now.to_rfc3339(),],
            )?;

            Ok(Source {
                id,
                uri: uri.to_string(),
                content_hash: hash,
                title: title.map(String::from),
                excerpt: excerpt.map(String::from),
                ingested_at: now,
            })
        })
    }

    /// Attach a claim span to a note, verifying overlap against the source.
    /// Rejects the write if verification fails.
    pub fn attach_claim_span(
        &self,
        req: &ClaimAttachmentRequest,
        config: &VerificationConfig,
    ) -> AppResult<ClaimSpan> {
        // Fetch note content + source excerpt
        let (note_content, source_excerpt) = self.execute(|conn| {
            let note_content: String = conn
                .query_row(
                    "SELECT content FROM notes WHERE id = ?1",
                    params![req.note_id],
                    |row| row.get(0),
                )
                .map_err(|_| AppError::NoteNotFound(req.note_id.clone()))?;
            let source_excerpt: String = conn
                .query_row(
                    "SELECT COALESCE(excerpt, '') FROM sources WHERE id = ?1",
                    params![req.source_id],
                    |row| row.get(0),
                )
                .map_err(|_| {
                    AppError::BadRequest(format!("Source not found: {}", req.source_id))
                })?;
            Ok((note_content, source_excerpt))
        })?;

        if req.claim_end > note_content.len() || req.claim_start >= req.claim_end {
            return Err(AppError::BadRequest(format!(
                "Invalid claim span [{}, {}) for note of length {}",
                req.claim_start,
                req.claim_end,
                note_content.len()
            )));
        }

        let claim_text = slice_safe(&note_content, req.claim_start, req.claim_end);
        let source_slice = match (req.source_span_start, req.source_span_end) {
            (Some(s), Some(e)) if e > s && e <= source_excerpt.len() => {
                slice_safe(&source_excerpt, s, e).to_string()
            }
            _ => source_excerpt.clone(),
        };

        let verification = verify_overlap(claim_text, &source_slice, config);

        if !verification.passed {
            return Err(AppError::BadRequest(format!(
                "Claim rejected: verification score {:.3} < {:.3} (method: {}). \
                 Provide a source span that structurally supports the claim.",
                verification.score, config.min_score, verification.method
            )));
        }

        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        self.execute(|conn| {
            conn.execute(
                "INSERT INTO claim_spans (
                    id, note_id, claim_start, claim_end, source_id,
                    source_span_start, source_span_end,
                    verification_score, verified_at, method
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    req.note_id,
                    req.claim_start as i64,
                    req.claim_end as i64,
                    req.source_id,
                    req.source_span_start.map(|v| v as i64),
                    req.source_span_end.map(|v| v as i64),
                    verification.score,
                    now.to_rfc3339(),
                    verification.method,
                ],
            )?;
            Ok(())
        })?;

        Ok(ClaimSpan {
            id,
            note_id: req.note_id.clone(),
            claim_start: req.claim_start,
            claim_end: req.claim_end,
            source_id: req.source_id.clone(),
            source_span_start: req.source_span_start,
            source_span_end: req.source_span_end,
            verification_score: verification.score,
            verified_at: now,
            method: verification.method,
        })
    }

    /// List all claim spans for a note, joined with their source metadata.
    pub fn get_claim_spans(&self, note_id: &str) -> AppResult<Vec<(ClaimSpan, Source)>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT cs.id, cs.note_id, cs.claim_start, cs.claim_end,
                        cs.source_id, cs.source_span_start, cs.source_span_end,
                        cs.verification_score, cs.verified_at, cs.method,
                        s.uri, s.content_hash, s.title, s.excerpt, s.ingested_at
                 FROM claim_spans cs
                 JOIN sources s ON s.id = cs.source_id
                 WHERE cs.note_id = ?1
                 ORDER BY cs.claim_start ASC",
            )?;
            let rows = stmt
                .query_map(params![note_id], |row| {
                    let cs = ClaimSpan {
                        id: row.get(0)?,
                        note_id: row.get(1)?,
                        claim_start: row.get::<_, i64>(2)? as usize,
                        claim_end: row.get::<_, i64>(3)? as usize,
                        source_id: row.get(4)?,
                        source_span_start: row.get::<_, Option<i64>>(5)?.map(|v| v as usize),
                        source_span_end: row.get::<_, Option<i64>>(6)?.map(|v| v as usize),
                        verification_score: row.get(7)?,
                        verified_at: chrono::DateTime::parse_from_rfc3339(
                            &row.get::<_, String>(8)?,
                        )
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                        method: row.get(9)?,
                    };
                    let src = Source {
                        id: cs.source_id.clone(),
                        uri: row.get(10)?,
                        content_hash: row.get(11)?,
                        title: row.get(12)?,
                        excerpt: row.get(13)?,
                        ingested_at: chrono::DateTime::parse_from_rfc3339(
                            &row.get::<_, String>(14)?,
                        )
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    };
                    Ok((cs, src))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
    }

    /// Count notes that have at least one claim span. Used by `smriti verify`.
    pub fn count_grounded_notes(&self) -> AppResult<(usize, usize)> {
        self.execute(|conn| {
            let total: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
            let grounded: i64 = conn.query_row(
                "SELECT COUNT(DISTINCT note_id) FROM claim_spans",
                [],
                |row| row.get(0),
            )?;
            Ok((total as usize, grounded as usize))
        })
    }
}

/// Slice a string safely at byte boundaries (best-effort; callers have already
/// validated bounds).
fn slice_safe(s: &str, start: usize, end: usize) -> &str {
    let start = (start..=s.len())
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(0);
    let end = (0..=end.min(s.len()))
        .rev()
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(start);
    &s[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_literal_containment_scores_full() {
        let v = verify_overlap(
            "aspirin reduces platelet aggregation",
            "Aspirin reduces platelet aggregation by inhibiting COX-1.",
            &VerificationConfig::default(),
        );
        assert!(v.passed, "literal containment should pass");
        assert_eq!(v.method, "literal");
    }

    #[test]
    fn verify_paraphrase_scores_via_tokens() {
        let v = verify_overlap(
            "aspirin reduces platelet aggregation",
            "platelet aggregation is reduced by aspirin therapy",
            &VerificationConfig::default(),
        );
        assert!(v.token > 0.5, "token jaccard should be high on paraphrase");
    }

    #[test]
    fn verify_unrelated_claim_fails() {
        let v = verify_overlap(
            "the moon is made of cheese",
            "aspirin reduces platelet aggregation",
            &VerificationConfig::default(),
        );
        assert!(!v.passed, "unrelated claim must fail");
    }

    #[test]
    fn verify_empty_source_fails() {
        let v = verify_overlap("anything", "", &VerificationConfig::default());
        assert!(!v.passed);
    }

    #[test]
    fn hash_content_is_deterministic() {
        assert_eq!(hash_content("hello"), hash_content("hello"));
        assert_ne!(hash_content("hello"), hash_content("world"));
    }

    #[test]
    fn upsert_source_is_idempotent() {
        let db = Database::new(":memory:").unwrap();
        let s1 = db
            .upsert_source(
                "https://example.com/doc1",
                "some content",
                Some("Doc 1"),
                Some("some content"),
            )
            .unwrap();
        let s2 = db
            .upsert_source(
                "https://example.com/doc1",
                "some content",
                Some("Doc 1"),
                Some("some content"),
            )
            .unwrap();
        assert_eq!(s1.id, s2.id);
    }

    #[test]
    fn attach_claim_rejects_fabrication() {
        let db = Database::new(":memory:").unwrap();
        let note = db
            .create_note(crate::models::CreateNoteRequest {
                title: "Test".into(),
                content: "The moon is made of green cheese.".into(),
                tags: vec![],
            })
            .unwrap();
        let source = db
            .upsert_source(
                "https://example.com/cheese",
                "irrelevant content about cheese prices in France",
                None,
                Some("irrelevant content about cheese prices in France"),
            )
            .unwrap();

        let result = db.attach_claim_span(
            &ClaimAttachmentRequest {
                note_id: note.id,
                claim_start: 0,
                claim_end: 33,
                source_id: source.id,
                source_span_start: None,
                source_span_end: None,
            },
            &VerificationConfig::default(),
        );
        assert!(result.is_err(), "fabricated claim must be rejected");
    }
}

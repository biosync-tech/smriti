//! `smriti verify` — integrity sweep over a Smriti database.
//!
//! This is the single-shot audit command that proves a `.smriti` file is
//! internally consistent. It runs four checks:
//!
//!   1. Referential integrity — every link references existing notes;
//!      every claim_span references an existing source.
//!   2. Provenance overlap — re-runs the FACTUM/Citation-Grounded overlap
//!      verifier against stored claim_spans. Any span whose current score
//!      falls below the minimum is flagged (sources may have drifted).
//!   3. Event log hash chain — verifies the append-only `events` table
//!      is tamper-evident (each row's `prev_hash` matches the previous
//!      row's `event_hash`).
//!   4. Orphan detection — notes with no incoming or outgoing links,
//!      reported as warnings (not errors).
//!
//! Research refs:
//!   - FACTUM               arXiv:2601.05866
//!   - Citation-Grounded    arXiv:2512.12117
//!   - Zep/Graphiti         arXiv:2501.13956  (event log semantics)
//!   - Hallucination Survey arXiv:2510.24476

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::errors::AppResult;
use crate::features::provenance::{verify_overlap, VerificationConfig};
use crate::storage::db::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub ok: bool,
    pub referential_errors: Vec<String>,
    pub provenance_failures: Vec<ProvenanceFailure>,
    pub event_chain_errors: Vec<String>,
    pub orphan_notes: Vec<String>,
    pub stats: VerifyStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyStats {
    pub notes: i64,
    pub links: i64,
    pub sources: i64,
    pub claim_spans: i64,
    pub events: i64,
    pub grounded_notes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceFailure {
    pub claim_span_id: String,
    pub note_id: String,
    pub stored_score: f64,
    pub current_score: f64,
    pub reason: String,
}

impl Database {
    /// Run the full integrity sweep. Never mutates data.
    pub fn verify(&self) -> AppResult<VerifyReport> {
        let stats = self.collect_verify_stats()?;
        let referential_errors = self.check_referential_integrity()?;
        let provenance_failures = self.recheck_provenance()?;
        let event_chain_errors = self.check_event_chain()?;
        let orphan_notes = self.find_orphans()?;

        let ok = referential_errors.is_empty()
            && provenance_failures.is_empty()
            && event_chain_errors.is_empty();

        Ok(VerifyReport {
            ok,
            referential_errors,
            provenance_failures,
            event_chain_errors,
            orphan_notes,
            stats,
        })
    }

    fn collect_verify_stats(&self) -> AppResult<VerifyStats> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let count = |sql: &str| -> AppResult<i64> {
            match conn.query_row(sql, [], |row| row.get::<_, i64>(0)) {
                Ok(n) => Ok(n),
                Err(_) => Ok(0),
            }
        };
        Ok(VerifyStats {
            notes: count("SELECT COUNT(*) FROM notes")?,
            links: count("SELECT COUNT(*) FROM links")?,
            sources: count("SELECT COUNT(*) FROM sources")?,
            claim_spans: count("SELECT COUNT(*) FROM claim_spans")?,
            events: count("SELECT COUNT(*) FROM events")?,
            grounded_notes: count("SELECT COUNT(DISTINCT note_id) FROM claim_spans")?,
        })
    }

    fn check_referential_integrity(&self) -> AppResult<Vec<String>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let mut errors = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT l.id FROM links l
             LEFT JOIN notes n1 ON l.source_note_id = n1.id
             LEFT JOIN notes n2 ON l.target_note_id = n2.id
             WHERE n1.id IS NULL OR n2.id IS NULL",
        )?;
        let dangling_links = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in dangling_links {
            errors.push(format!("link {} references a missing note", id));
        }

        let mut stmt = conn.prepare(
            "SELECT cs.id FROM claim_spans cs
             LEFT JOIN sources s ON cs.source_id = s.id
             WHERE s.id IS NULL",
        )?;
        let dangling_claims = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in dangling_claims {
            errors.push(format!("claim_span {} references a missing source", id));
        }

        Ok(errors)
    }

    fn recheck_provenance(&self) -> AppResult<Vec<ProvenanceFailure>> {
        // Tuple shape: (claim_id, note_id, note_content, source_excerpt,
        // stored_verification_score, source_span_start, source_span_end).
        // Aliased for readability and to silence clippy::type_complexity.
        type ClaimRow = (
            String,
            String,
            String,
            String,
            f64,
            Option<i64>,
            Option<i64>,
        );

        let cfg = VerificationConfig::default();
        let rows: Vec<ClaimRow> = {
            let conn = self.conn.lock().map_err(|e| {
                crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
            })?;
            let mut stmt = conn.prepare(
                "SELECT cs.id, cs.note_id, n.content, s.excerpt,
                        cs.verification_score, cs.source_span_start, cs.source_span_end
                 FROM claim_spans cs
                 JOIN notes n    ON cs.note_id = n.id
                 JOIN sources s  ON cs.source_id = s.id",
            )?;
            let collected: Vec<ClaimRow> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                        row.get::<_, f64>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            collected
        };

        let mut failures = Vec::new();
        for (cs_id, note_id, content, excerpt, stored, _span_start, _span_end) in rows {
            // Use the full note content as the claim text. Production callers
            // can re-verify at finer granularity if they stored exact offsets.
            let result = verify_overlap(&content, &excerpt, &cfg);
            if result.score < cfg.min_score {
                failures.push(ProvenanceFailure {
                    claim_span_id: cs_id,
                    note_id,
                    stored_score: stored,
                    current_score: result.score,
                    reason: format!(
                        "overlap {:.3} below threshold {:.3}",
                        result.score, cfg.min_score
                    ),
                });
            }
        }
        Ok(failures)
    }

    fn check_event_chain(&self) -> AppResult<Vec<String>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let mut stmt = conn
            .prepare("SELECT id, prev_hash, event_hash FROM events ORDER BY ingestion_time ASC")?;
        let rows: Vec<(String, Option<String>, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut errors = Vec::new();
        let mut last_hash: Option<String> = None;
        for (id, prev, hash) in rows {
            if prev != last_hash {
                errors.push(format!(
                    "event {} prev_hash {:?} does not chain from previous {:?}",
                    id, prev, last_hash
                ));
            }
            last_hash = Some(hash);
        }
        Ok(errors)
    }

    fn find_orphans(&self) -> AppResult<Vec<String>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT n.id FROM notes n
             WHERE NOT EXISTS (SELECT 1 FROM links WHERE source_note_id = n.id)
               AND NOT EXISTS (SELECT 1 FROM links WHERE target_note_id = n.id)",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Append an event to the append-only log. Computes the hash chain.
    ///
    /// Research ref: Zep/Graphiti arXiv:2501.13956 §3.3 — T/T' bi-temporal
    /// semantics via (event_time, ingestion_time).
    pub fn append_event(
        &self,
        event_type: &str,
        entity_type: &str,
        entity_id: &str,
        payload: &serde_json::Value,
        agent_id: Option<&str>,
        transaction_id: Option<&str>,
    ) -> AppResult<String> {
        use chrono::Utc;
        use sha2::{Digest, Sha256};

        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let payload_str = serde_json::to_string(payload)?;

        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let prev_hash: Option<String> = conn
            .query_row(
                "SELECT event_hash FROM events ORDER BY ingestion_time DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        let mut hasher = Sha256::new();
        hasher.update(id.as_bytes());
        hasher.update(event_type.as_bytes());
        hasher.update(entity_type.as_bytes());
        hasher.update(entity_id.as_bytes());
        hasher.update(payload_str.as_bytes());
        hasher.update(now.as_bytes());
        if let Some(ref p) = prev_hash {
            hasher.update(p.as_bytes());
        }
        let event_hash = format!("{:x}", hasher.finalize());

        conn.execute(
            "INSERT INTO events
             (id, event_type, entity_type, entity_id, payload, agent_id,
              transaction_id, event_time, ingestion_time, prev_hash, event_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, ?10)",
            params![
                id,
                event_type,
                entity_type,
                entity_id,
                payload_str,
                agent_id,
                transaction_id,
                now,
                prev_hash,
                event_hash,
            ],
        )?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verify_empty_db_passes() {
        let db = Database::new(":memory:").unwrap();
        let report = db.verify().unwrap();
        assert!(report.ok);
        assert_eq!(report.stats.notes, 0);
    }

    #[test]
    fn event_chain_hashes_link_correctly() {
        let db = Database::new(":memory:").unwrap();
        db.append_event("note.created", "note", "n1", &json!({"t": "A"}), None, None)
            .unwrap();
        db.append_event("note.created", "note", "n2", &json!({"t": "B"}), None, None)
            .unwrap();
        let report = db.verify().unwrap();
        assert!(report.event_chain_errors.is_empty());
        assert_eq!(report.stats.events, 2);
    }
}

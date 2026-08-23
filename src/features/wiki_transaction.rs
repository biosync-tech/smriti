//! Wiki transaction — atomic multi-write primitive for agent-authored knowledge.
//!
//! ## Why this exists
//!
//! Every competitor in the agent-memory space (Zep, Mem0, Letta, A-MEM,
//! HippoRAG, LightRAG) writes to memory one operation at a time. A real
//! synthesis workflow is never one write:
//!
//! > create entity page + update index page + append log entry +
//! > create backlinks + archive superseded claims + attach sources
//!
//! If any step fails, the wiki is silently corrupted. Smriti's single-SQLite
//! file lets us wrap the whole batch in a SAVEPOINT that either commits
//! atomically or rolls back entirely.
//!
//! ## Pending vs committed
//!
//! A transaction can be submitted in `pending` state — the payload is stored
//! but nothing is applied. Humans or supervising agents review pending
//! transactions (the "diff review inbox" pattern) and call commit or reject.
//! This turns Smriti into *git for agent memory*: every change is a
//! reviewable patch, not a blind write.
//!
//! ## Provenance enforcement
//!
//! When a transaction creates or updates a note and carries source
//! attachments, the overlap verification invariant from
//! `features::provenance` is enforced as part of the same SAVEPOINT. Fail
//! any claim → rollback the whole transaction. No partial grounding.
//!
//! Research context:
//!   - No SOTA paper has this primitive — it is enabled by Smriti's choice
//!     of SQLite + single-binary architecture.
//!   - Citation-Grounded Code Comprehension (arXiv:2512.12117) argues for
//!     enforcing grounding as a structural constraint rather than a
//!     post-hoc detector. wiki_transaction is the enforcement point.

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::features::provenance::VerificationConfig;
use crate::models::LinkType;
use crate::storage::Database;

/// Operations a wiki transaction can contain. Kept intentionally narrow —
/// expand only when a real workflow demands it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WikiOp {
    /// Create a new note with optional tags.
    CreateNote {
        title: String,
        content: String,
        #[serde(default)]
        tags: Vec<String>,
        /// Required source attachments. If empty on a CreateNote in a
        /// provenance-required transaction, the transaction is rejected.
        #[serde(default)]
        claims: Vec<ClaimInlineRequest>,
    },
    /// Update an existing note's content/title.
    UpdateNote {
        id: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        content: Option<String>,
        /// Replace the explicit tag set on the note. If `None`, the existing
        /// tags are preserved; if `Some(vec)`, the supplied tag set replaces
        /// the existing one. Inline `#hashtag` tokens in `content` are
        /// always merged in by the handler (see Bug #2 / Bug #3 fix).
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        claims: Vec<ClaimInlineRequest>,
    },
    /// Create a typed link between two notes (by ID or title).
    CreateLink {
        source: String,
        target: String,
        #[serde(default = "default_link_type")]
        link_type: String,
    },
    /// Upsert a source document.
    UpsertSource {
        uri: String,
        content: String,
        #[serde(default)]
        title: Option<String>,
    },
}

fn default_link_type() -> String {
    "wikilink".to_string()
}

/// Inline claim attachment — the note_id is filled in by the transaction
/// engine once the target note exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimInlineRequest {
    pub claim_start: usize,
    pub claim_end: usize,
    /// Either an existing source_id OR a new source to upsert via `source_uri`.
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub source_uri: Option<String>,
    #[serde(default)]
    pub source_content: Option<String>,
    #[serde(default)]
    pub source_span_start: Option<usize>,
    #[serde(default)]
    pub source_span_end: Option<usize>,
}

/// A wiki transaction — a batch of operations that succeed or fail together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiTransaction {
    pub id: String,
    pub agent_id: String,
    pub status: TransactionStatus,
    pub operations: Vec<WikiOp>,
    pub rationale: Option<String>,
    pub require_provenance: bool,
    pub created_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    Committed,
    Rejected,
    Failed,
}

impl TransactionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Committed => "committed",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "committed" => Self::Committed,
            "rejected" => Self::Rejected,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

/// Request to submit a new wiki transaction.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitTransactionRequest {
    pub agent_id: String,
    pub operations: Vec<WikiOp>,
    #[serde(default)]
    pub rationale: Option<String>,
    /// If true, the transaction is stored as pending and NOT applied until
    /// a human or supervising agent calls commit_transaction.
    #[serde(default)]
    pub pending: bool,
    /// If true, any CreateNote/UpdateNote op with no claims is rejected.
    /// Default: true for safety.
    #[serde(default = "default_require_provenance")]
    pub require_provenance: bool,
}

fn default_require_provenance() -> bool {
    true
}

/// The result of a transaction commit — includes created entity IDs so
/// callers can follow up.
#[derive(Debug, Clone, Serialize)]
pub struct TransactionResult {
    pub transaction_id: String,
    pub status: String,
    pub created_note_ids: Vec<String>,
    pub created_source_ids: Vec<String>,
    pub created_link_count: usize,
    pub verified_claim_count: usize,
    pub message: Option<String>,
}

impl Database {
    /// Submit a wiki transaction. If `pending` is true, stores the operations
    /// and returns a pending status; otherwise applies them atomically inside
    /// a SAVEPOINT.
    pub fn submit_wiki_transaction(
        &self,
        req: SubmitTransactionRequest,
    ) -> AppResult<TransactionResult> {
        let tx_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Validate: if provenance is required, every CreateNote/UpdateNote
        // must have at least one claim. Fail fast before touching the DB.
        if req.require_provenance {
            for op in &req.operations {
                match op {
                    WikiOp::CreateNote { claims, title, .. } if claims.is_empty() => {
                        return Err(AppError::BadRequest(format!(
                            "CreateNote '{}' has no claims; provenance is required. \
                             Attach at least one source span or set require_provenance=false.",
                            title
                        )));
                    }
                    WikiOp::UpdateNote {
                        claims,
                        id,
                        content: Some(_),
                        ..
                    } if claims.is_empty() => {
                        return Err(AppError::BadRequest(format!(
                            "UpdateNote '{}' changes content but has no claims; \
                             provenance is required for content updates.",
                            id
                        )));
                    }
                    _ => {}
                }
            }
        }

        let ops_json = serde_json::to_string(&req.operations)?;

        // Pending path: record and return without applying
        if req.pending {
            self.execute(|conn| {
                conn.execute(
                    "INSERT INTO wiki_transactions
                     (id, agent_id, status, operations, rationale, created_at)
                     VALUES (?1, ?2, 'pending', ?3, ?4, ?5)",
                    params![
                        tx_id,
                        req.agent_id,
                        ops_json,
                        req.rationale,
                        now.to_rfc3339()
                    ],
                )?;
                Ok(())
            })?;
            return Ok(TransactionResult {
                transaction_id: tx_id,
                status: "pending".into(),
                created_note_ids: vec![],
                created_source_ids: vec![],
                created_link_count: 0,
                verified_claim_count: 0,
                message: Some("Transaction queued for review".into()),
            });
        }

        // Apply path: wrap in SAVEPOINT
        self.apply_transaction(&tx_id, &req, &ops_json, now)
    }

    /// Commit a pending transaction by id.
    pub fn commit_wiki_transaction(&self, tx_id: &str) -> AppResult<TransactionResult> {
        let (agent_id, ops_json, rationale, require_provenance, created_at) =
            self.execute(|conn| {
                conn.query_row(
                    "SELECT agent_id, operations, rationale, created_at
                     FROM wiki_transactions WHERE id = ?1 AND status = 'pending'",
                    params![tx_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            true,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .map_err(|_| {
                    AppError::BadRequest(format!("Pending transaction not found: {}", tx_id))
                })
            })?;

        let operations: Vec<WikiOp> = serde_json::from_str(&ops_json)?;
        let _ = created_at; // kept for future audit use

        let req = SubmitTransactionRequest {
            agent_id,
            operations,
            rationale,
            pending: false,
            require_provenance,
        };

        self.apply_transaction(tx_id, &req, &ops_json, Utc::now())
    }

    /// Reject a pending transaction.
    pub fn reject_wiki_transaction(
        &self,
        tx_id: &str,
        rejected_by: &str,
        reason: &str,
    ) -> AppResult<()> {
        let now = Utc::now();
        self.execute(|conn| {
            let changed = conn.execute(
                "UPDATE wiki_transactions
                 SET status = 'rejected', rejected_at = ?1,
                     rejected_by = ?2, rejection_reason = ?3
                 WHERE id = ?4 AND status = 'pending'",
                params![now.to_rfc3339(), rejected_by, reason, tx_id],
            )?;
            if changed == 0 {
                return Err(AppError::BadRequest(format!(
                    "Pending transaction not found: {}",
                    tx_id
                )));
            }
            Ok(())
        })
    }

    /// List pending transactions (the "review inbox").
    pub fn list_pending_transactions(&self, limit: usize) -> AppResult<Vec<WikiTransaction>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, status, operations, rationale, created_at, committed_at
                 FROM wiki_transactions
                 WHERE status = 'pending'
                 ORDER BY created_at ASC
                 LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit as i64], |row| {
                    let ops_json: String = row.get(3)?;
                    let operations = serde_json::from_str(&ops_json).unwrap_or_default();
                    Ok(WikiTransaction {
                        id: row.get(0)?,
                        agent_id: row.get(1)?,
                        status: TransactionStatus::parse(&row.get::<_, String>(2)?),
                        operations,
                        rationale: row.get(4)?,
                        require_provenance: true,
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                            .unwrap_or_default()
                            .with_timezone(&Utc),
                        committed_at: row
                            .get::<_, Option<String>>(6)?
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|d| d.with_timezone(&Utc)),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
    }

    /// Internal: apply a transaction inside a SAVEPOINT. The caller is
    /// responsible for having validated provenance requirements already.
    fn apply_transaction(
        &self,
        tx_id: &str,
        req: &SubmitTransactionRequest,
        ops_json: &str,
        created_at: DateTime<Utc>,
    ) -> AppResult<TransactionResult> {
        let config = VerificationConfig::default();
        let mut result = TransactionResult {
            transaction_id: tx_id.to_string(),
            status: "committed".into(),
            created_note_ids: vec![],
            created_source_ids: vec![],
            created_link_count: 0,
            verified_claim_count: 0,
            message: None,
        };

        // Upsert the transaction row in "pending" first so we can update it
        // to committed/failed inside the SAVEPOINT.
        self.execute(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO wiki_transactions
                 (id, agent_id, status, operations, rationale, created_at)
                 VALUES (?1, ?2, 'pending', ?3, ?4, ?5)",
                params![
                    tx_id,
                    req.agent_id,
                    ops_json,
                    req.rationale,
                    created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })?;

        // Phase 1: run all ops inside a SAVEPOINT and record outcomes.
        // We use a dedicated execute closure so the SAVEPOINT is held for
        // the whole batch.
        let apply_result: AppResult<()> = self.execute(|conn| {
            conn.execute_batch("SAVEPOINT wiki_tx;")?;

            let inner = (|| -> AppResult<()> {
                for op in &req.operations {
                    match op {
                        WikiOp::CreateNote {
                            title,
                            content,
                            tags,
                            claims,
                        } => {
                            // Insert note directly via raw SQL to stay inside
                            // the SAVEPOINT (create_note uses its own lock).
                            let note_id = Uuid::new_v4().to_string();
                            let now = Utc::now().to_rfc3339();
                            conn.execute(
                                "INSERT INTO notes (id, title, content, created_at, updated_at)
                                 VALUES (?1, ?2, ?3, ?4, ?4)",
                                params![note_id, title, content, now],
                            )?;

                            // ── Bug #3 fix: inline-extraction parity ────
                            //
                            // The standalone `notes_create` MCP handler runs
                            // `parser::extract_tags()` and
                            // `parser::extract_wikilinks()` on the note body
                            // before persisting (see mcp/handlers.rs ~L35–55).
                            // Prior to this fix, the wiki_transaction
                            // CreateNote op silently dropped both — content
                            // committed cleanly but no graph edges or tag
                            // index entries were created. That violates the
                            // principle of API symmetry: identical inputs
                            // through two write paths must produce identical
                            // observable state (Liskov substitution applied
                            // to interface contracts; Saltzer & Schroeder
                            // 1975 §3.A.5 Principle of Least Astonishment).
                            //
                            // Merge: explicit `tags` ∪ inline `#hashtag`
                            // tokens. Inline-extraction is additive; never
                            // overwrites caller intent.
                            let mut all_tags: Vec<String> = tags.clone();
                            for inline in crate::parser::extract_tags(content) {
                                if !all_tags.contains(&inline) {
                                    all_tags.push(inline);
                                }
                            }

                            for tag_name in &all_tags {
                                let tag_id = Uuid::new_v4().to_string();
                                conn.execute(
                                    "INSERT OR IGNORE INTO tags (id, name, created_at)
                                     VALUES (?1, ?2, ?3)",
                                    params![tag_id, tag_name, now],
                                )?;
                                let actual_tag_id: String = conn.query_row(
                                    "SELECT id FROM tags WHERE name = ?1",
                                    params![tag_name],
                                    |row| row.get(0),
                                )?;
                                conn.execute(
                                    "INSERT OR IGNORE INTO note_tags (note_id, tag_id)
                                     VALUES (?1, ?2)",
                                    params![note_id, actual_tag_id],
                                )?;
                            }

                            // Inline `[[wiki-link]]` extraction. For each
                            // link whose target title resolves to an
                            // existing note, materialise a graph edge with
                            // the typed relation (default: wikilink). This
                            // mirrors the standalone create-path exactly.
                            for wl in crate::parser::extract_wikilinks(content) {
                                let target_id: Option<String> = conn
                                    .query_row(
                                        "SELECT id FROM notes WHERE title = ?1",
                                        params![wl.target],
                                        |row| row.get(0),
                                    )
                                    .ok();
                                if let Some(tid) = target_id {
                                    let link_id = Uuid::new_v4().to_string();
                                    let lt = LinkType::parse(&wl.relation);
                                    conn.execute(
                                        "INSERT OR IGNORE INTO links
                                         (id, source_note_id, target_note_id,
                                          link_type, created_at, valid_from)
                                         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                                        params![link_id, note_id, tid, lt.as_str(), now],
                                    )?;
                                    result.created_link_count += 1;
                                }
                            }

                            // Attach claims + verify
                            for claim in claims {
                                let source_id = resolve_source_inline(conn, claim)?;
                                verify_and_attach_inline(
                                    conn, &note_id, content, &source_id, claim, &config,
                                )?;
                                result.verified_claim_count += 1;
                            }

                            result.created_note_ids.push(note_id);
                        }
                        WikiOp::UpdateNote {
                            id,
                            title,
                            content,
                            tags,
                            claims,
                        } => {
                            let now = Utc::now().to_rfc3339();
                            if let Some(c) = content {
                                conn.execute(
                                    "UPDATE notes SET content = ?1, updated_at = ?2 WHERE id = ?3",
                                    params![c, now, id],
                                )?;
                            }
                            if let Some(t) = title {
                                conn.execute(
                                    "UPDATE notes SET title = ?1, updated_at = ?2 WHERE id = ?3",
                                    params![t, now, id],
                                )?;
                            }

                            // ── Bug #2 + Bug #3 fix: re-run extraction on
                            // the post-update content. Without this, an
                            // update_note that rewrites content silently
                            // strands the prior wiki-link / tag indices.
                            //
                            // Strategy:
                            //   * If `tags` is Some(vec), the explicit set
                            //     replaces the existing tag set (caller
                            //     intent is authoritative). Inline tokens
                            //     from the new content are merged in.
                            //   * If `tags` is None but `content` changed,
                            //     re-derive tags from inline #hashtag
                            //     tokens in the new content (don't strand
                            //     the index).
                            //   * Wiki-links: any time `content` changes,
                            //     re-extract and ensure-edges. Existing
                            //     edges to the same target are preserved
                            //     by INSERT OR IGNORE on the (source,
                            //     target, link_type) UNIQUE constraint.
                            let updated_content: Option<String> = if content.is_some() {
                                Some(conn.query_row(
                                    "SELECT content FROM notes WHERE id = ?1",
                                    params![id],
                                    |row| row.get(0),
                                )?)
                            } else {
                                None
                            };

                            if let Some(explicit_tags) = tags {
                                // Replace the existing tag set with the
                                // explicit set ∪ inline tokens.
                                let mut all_tags: Vec<String> = explicit_tags.clone();
                                if let Some(c) = updated_content.as_deref() {
                                    for inline in crate::parser::extract_tags(c) {
                                        if !all_tags.contains(&inline) {
                                            all_tags.push(inline);
                                        }
                                    }
                                }
                                conn.execute(
                                    "DELETE FROM note_tags WHERE note_id = ?1",
                                    params![id],
                                )?;
                                for tag_name in &all_tags {
                                    let tag_id = Uuid::new_v4().to_string();
                                    conn.execute(
                                        "INSERT OR IGNORE INTO tags (id, name, created_at)
                                         VALUES (?1, ?2, ?3)",
                                        params![tag_id, tag_name, now],
                                    )?;
                                    let actual_tag_id: String = conn.query_row(
                                        "SELECT id FROM tags WHERE name = ?1",
                                        params![tag_name],
                                        |row| row.get(0),
                                    )?;
                                    conn.execute(
                                        "INSERT OR IGNORE INTO note_tags (note_id, tag_id)
                                         VALUES (?1, ?2)",
                                        params![id, actual_tag_id],
                                    )?;
                                }
                            } else if let Some(c) = updated_content.as_deref() {
                                // No explicit tag set, but content changed:
                                // ensure inline tokens are at least merged
                                // additively (don't drop them silently).
                                for inline in crate::parser::extract_tags(c) {
                                    let tag_id = Uuid::new_v4().to_string();
                                    conn.execute(
                                        "INSERT OR IGNORE INTO tags (id, name, created_at)
                                         VALUES (?1, ?2, ?3)",
                                        params![tag_id, inline, now],
                                    )?;
                                    let actual_tag_id: String = conn.query_row(
                                        "SELECT id FROM tags WHERE name = ?1",
                                        params![inline],
                                        |row| row.get(0),
                                    )?;
                                    conn.execute(
                                        "INSERT OR IGNORE INTO note_tags (note_id, tag_id)
                                         VALUES (?1, ?2)",
                                        params![id, actual_tag_id],
                                    )?;
                                }
                            }

                            // Wiki-link re-extraction on content change.
                            if let Some(c) = updated_content.as_deref() {
                                for wl in crate::parser::extract_wikilinks(c) {
                                    let target_id: Option<String> = conn
                                        .query_row(
                                            "SELECT id FROM notes WHERE title = ?1",
                                            params![wl.target],
                                            |row| row.get(0),
                                        )
                                        .ok();
                                    if let Some(tid) = target_id {
                                        let link_id = Uuid::new_v4().to_string();
                                        let lt = LinkType::parse(&wl.relation);
                                        let inserted = conn.execute(
                                            "INSERT OR IGNORE INTO links
                                             (id, source_note_id, target_note_id,
                                              link_type, created_at, valid_from)
                                             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                                            params![link_id, id, tid, lt.as_str(), now],
                                        )?;
                                        if inserted > 0 {
                                            result.created_link_count += 1;
                                        }
                                    }
                                }
                            }

                            if !claims.is_empty() {
                                let current: String = conn.query_row(
                                    "SELECT content FROM notes WHERE id = ?1",
                                    params![id],
                                    |row| row.get(0),
                                )?;
                                for claim in claims {
                                    let source_id = resolve_source_inline(conn, claim)?;
                                    verify_and_attach_inline(
                                        conn, id, &current, &source_id, claim, &config,
                                    )?;
                                    result.verified_claim_count += 1;
                                }
                            }
                        }
                        WikiOp::CreateLink {
                            source,
                            target,
                            link_type,
                        } => {
                            // Resolve by ID or title
                            let source_id = resolve_note_ref(conn, source)?;
                            let target_id = resolve_note_ref(conn, target)?;
                            let link_id = Uuid::new_v4().to_string();
                            let lt = LinkType::parse(link_type);
                            let now = Utc::now().to_rfc3339();
                            conn.execute(
                                "INSERT OR IGNORE INTO links
                                 (id, source_note_id, target_note_id, link_type,
                                  created_at, valid_from)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                                params![link_id, source_id, target_id, lt.as_str(), now],
                            )?;
                            result.created_link_count += 1;
                        }
                        WikiOp::UpsertSource {
                            uri,
                            content,
                            title,
                        } => {
                            let hash = crate::features::provenance::hash_content(content);
                            let existing: Option<String> = conn
                                .query_row(
                                    "SELECT id FROM sources WHERE uri = ?1 AND content_hash = ?2",
                                    params![uri, hash],
                                    |row| row.get(0),
                                )
                                .ok();
                            let source_id = if let Some(id) = existing {
                                id
                            } else {
                                let id = Uuid::new_v4().to_string();
                                let now = Utc::now().to_rfc3339();
                                conn.execute(
                                    "INSERT INTO sources
                                     (id, uri, content_hash, title, excerpt, ingested_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                    params![id, uri, hash, title, content, now],
                                )?;
                                id
                            };
                            result.created_source_ids.push(source_id);
                        }
                    }
                }
                Ok(())
            })();

            match inner {
                Ok(()) => {
                    conn.execute_batch("RELEASE SAVEPOINT wiki_tx;")?;
                    let committed_at = Utc::now().to_rfc3339();
                    conn.execute(
                        "UPDATE wiki_transactions
                         SET status = 'committed', committed_at = ?1
                         WHERE id = ?2",
                        params![committed_at, tx_id],
                    )?;
                    Ok(())
                }
                Err(e) => {
                    conn.execute_batch(
                        "ROLLBACK TO SAVEPOINT wiki_tx; RELEASE SAVEPOINT wiki_tx;",
                    )?;
                    conn.execute(
                        "UPDATE wiki_transactions
                         SET status = 'failed', rejection_reason = ?1
                         WHERE id = ?2",
                        params![e.to_string(), tx_id],
                    )?;
                    Err(e)
                }
            }
        });

        match apply_result {
            Ok(()) => Ok(result),
            Err(e) => {
                result.status = "failed".into();
                result.message = Some(e.to_string());
                Err(e)
            }
        }
    }
}

fn resolve_note_ref(conn: &rusqlite::Connection, reference: &str) -> AppResult<String> {
    // Try ID first
    let by_id: Option<String> = conn
        .query_row(
            "SELECT id FROM notes WHERE id = ?1",
            params![reference],
            |row| row.get(0),
        )
        .ok();
    if let Some(id) = by_id {
        return Ok(id);
    }
    // Fall back to title
    conn.query_row(
        "SELECT id FROM notes WHERE title = ?1",
        params![reference],
        |row| row.get(0),
    )
    .map_err(|_| AppError::NoteNotFound(reference.to_string()))
}

fn resolve_source_inline(
    conn: &rusqlite::Connection,
    claim: &ClaimInlineRequest,
) -> AppResult<String> {
    if let Some(sid) = &claim.source_id {
        return Ok(sid.clone());
    }
    let uri = claim
        .source_uri
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Claim missing source_id or source_uri".into()))?;
    let content = claim.source_content.as_ref().ok_or_else(|| {
        AppError::BadRequest("Claim with source_uri requires source_content".into())
    })?;
    let hash = crate::features::provenance::hash_content(content);
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM sources WHERE uri = ?1 AND content_hash = ?2",
            params![uri, hash],
            |row| row.get(0),
        )
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sources (id, uri, content_hash, title, excerpt, ingested_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
        params![id, uri, hash, content, now],
    )?;
    Ok(id)
}

fn verify_and_attach_inline(
    conn: &rusqlite::Connection,
    note_id: &str,
    note_content: &str,
    source_id: &str,
    claim: &ClaimInlineRequest,
    config: &VerificationConfig,
) -> AppResult<()> {
    use crate::features::provenance::verify_overlap;

    if claim.claim_end > note_content.len() || claim.claim_start >= claim.claim_end {
        return Err(AppError::BadRequest(format!(
            "Invalid claim span [{}, {}) for note of length {}",
            claim.claim_start,
            claim.claim_end,
            note_content.len()
        )));
    }

    let excerpt: String = conn
        .query_row(
            "SELECT COALESCE(excerpt, '') FROM sources WHERE id = ?1",
            params![source_id],
            |row| row.get(0),
        )
        .map_err(|_| AppError::BadRequest(format!("Source not found: {}", source_id)))?;

    let claim_text = {
        let start = (claim.claim_start..=note_content.len())
            .find(|i| note_content.is_char_boundary(*i))
            .unwrap_or(0);
        let end = (0..=claim.claim_end.min(note_content.len()))
            .rev()
            .find(|i| note_content.is_char_boundary(*i))
            .unwrap_or(start);
        &note_content[start..end]
    };

    let source_slice = match (claim.source_span_start, claim.source_span_end) {
        (Some(s), Some(e)) if e > s && e <= excerpt.len() => {
            let start = (s..=excerpt.len())
                .find(|i| excerpt.is_char_boundary(*i))
                .unwrap_or(0);
            let end = (0..=e.min(excerpt.len()))
                .rev()
                .find(|i| excerpt.is_char_boundary(*i))
                .unwrap_or(start);
            excerpt[start..end].to_string()
        }
        _ => excerpt.clone(),
    };

    let v = verify_overlap(claim_text, &source_slice, config);
    if !v.passed {
        return Err(AppError::BadRequest(format!(
            "Claim rejected: verification {:.3} < {:.3} (method {})",
            v.score, config.min_score, v.method
        )));
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO claim_spans
         (id, note_id, claim_start, claim_end, source_id,
          source_span_start, source_span_end,
          verification_score, verified_at, method)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            note_id,
            claim.claim_start as i64,
            claim.claim_end as i64,
            source_id,
            claim.source_span_start.map(|v| v as i64),
            claim.source_span_end.map(|v| v as i64),
            v.score,
            now,
            v.method,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_transaction_is_not_applied() {
        let db = Database::new(":memory:").unwrap();
        let req = SubmitTransactionRequest {
            agent_id: "claude".into(),
            operations: vec![WikiOp::CreateNote {
                title: "Test".into(),
                content: "The moon is a natural satellite".into(),
                tags: vec![],
                claims: vec![ClaimInlineRequest {
                    claim_start: 0,
                    claim_end: 31,
                    source_id: None,
                    source_uri: Some("https://example.com/moon".into()),
                    source_content: Some("The moon is a natural satellite of Earth".into()),
                    source_span_start: None,
                    source_span_end: None,
                }],
            }],
            rationale: None,
            pending: true,
            require_provenance: true,
        };
        let r = db.submit_wiki_transaction(req).unwrap();
        assert_eq!(r.status, "pending");

        // Note should not exist yet
        let pending = db.list_pending_transactions(10).unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn require_provenance_rejects_empty_claims() {
        let db = Database::new(":memory:").unwrap();
        let req = SubmitTransactionRequest {
            agent_id: "claude".into(),
            operations: vec![WikiOp::CreateNote {
                title: "Bare".into(),
                content: "unsourced claim".into(),
                tags: vec![],
                claims: vec![],
            }],
            rationale: None,
            pending: false,
            require_provenance: true,
        };
        let result = db.submit_wiki_transaction(req);
        assert!(result.is_err());
    }

    #[test]
    fn atomic_commit_applies_all_or_nothing() {
        let db = Database::new(":memory:").unwrap();
        // Build a transaction with two CreateNotes; the second has a
        // fabricated claim and should cause the whole batch to roll back.
        let req = SubmitTransactionRequest {
            agent_id: "claude".into(),
            operations: vec![
                WikiOp::CreateNote {
                    title: "Good".into(),
                    content: "aspirin inhibits COX-1".into(),
                    tags: vec![],
                    claims: vec![ClaimInlineRequest {
                        claim_start: 0,
                        claim_end: 22,
                        source_id: None,
                        source_uri: Some("https://example.com/asa".into()),
                        source_content: Some("Aspirin inhibits COX-1 irreversibly".into()),
                        source_span_start: None,
                        source_span_end: None,
                    }],
                },
                WikiOp::CreateNote {
                    title: "Bad".into(),
                    content: "the moon is made of cheese".into(),
                    tags: vec![],
                    claims: vec![ClaimInlineRequest {
                        claim_start: 0,
                        claim_end: 26,
                        source_id: None,
                        source_uri: Some("https://example.com/unrelated".into()),
                        source_content: Some("cheese prices are rising in France".into()),
                        source_span_start: None,
                        source_span_end: None,
                    }],
                },
            ],
            rationale: None,
            pending: false,
            require_provenance: true,
        };
        let result = db.submit_wiki_transaction(req);
        assert!(
            result.is_err(),
            "expected rollback on fabricated second claim"
        );

        // Verify the "Good" note was NOT persisted (full rollback)
        let notes = db
            .list_notes(&crate::models::NoteListQuery {
                limit: 100,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: None,
            })
            .unwrap();
        assert_eq!(notes.len(), 0, "all writes should be rolled back");
    }

    // ─── Bug #2 / Bug #3 regression: inline-extraction parity ─────
    //
    // Before fix: wiki_transaction CreateNote / UpdateNote silently
    // dropped inline `[[wiki-links]]` and `#hashtags`. Same input
    // through `notes_create` standalone produced graph edges + tag
    // index entries; through `wiki_transaction_submit` produced none.
    //
    // Principle violated: Liskov-style API symmetry. Two write paths
    // exposed as equivalent must produce equivalent state on
    // equivalent input. (Saltzer & Schroeder 1975 §3.A.5; Tulach
    // 2008 "Practical API Design" Ch.10 on consistency-of-effects.)

    #[test]
    fn create_note_extracts_inline_hashtags_in_wiki_tx() {
        let db = Database::new(":memory:").unwrap();
        let req = SubmitTransactionRequest {
            agent_id: "test".into(),
            operations: vec![WikiOp::CreateNote {
                title: "Note A".into(),
                content: "Content has #foo and #bar inline tags".into(),
                tags: vec!["explicit".into()],
                claims: vec![],
            }],
            rationale: None,
            pending: false,
            require_provenance: false,
        };
        db.submit_wiki_transaction(req).unwrap();

        // Querying by inline-detected tag should return the note.
        let by_foo = db
            .list_notes(&crate::models::NoteListQuery {
                limit: 10,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: Some("foo".into()),
            })
            .unwrap();
        assert_eq!(by_foo.len(), 1, "inline #foo must populate tag index");

        let by_explicit = db
            .list_notes(&crate::models::NoteListQuery {
                limit: 10,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: Some("explicit".into()),
            })
            .unwrap();
        assert_eq!(by_explicit.len(), 1, "explicit tags array still works");
    }

    #[test]
    fn create_note_creates_graph_edges_for_inline_wikilinks() {
        let db = Database::new(":memory:").unwrap();
        // Pre-create the link target; resolution happens by title.
        db.create_note(crate::models::CreateNoteRequest {
            title: "Target Note".into(),
            content: "I am the target".into(),
            tags: vec![],
        })
        .unwrap();

        let req = SubmitTransactionRequest {
            agent_id: "test".into(),
            operations: vec![WikiOp::CreateNote {
                title: "Source Note".into(),
                content: "Refers to [[Target Note]] for context".into(),
                tags: vec![],
                claims: vec![],
            }],
            rationale: None,
            pending: false,
            require_provenance: false,
        };
        let result = db.submit_wiki_transaction(req).unwrap();
        assert_eq!(
            result.created_link_count, 1,
            "wiki_tx CreateNote must materialise inline [[wiki-link]] edges \
             (Bug #3 regression)"
        );
    }

    #[test]
    fn update_note_re_extracts_inline_tags_after_content_change() {
        let db = Database::new(":memory:").unwrap();
        let note = db
            .create_note(crate::models::CreateNoteRequest {
                title: "Updatable".into(),
                content: "no tags here".into(),
                tags: vec![],
            })
            .unwrap();

        let req = SubmitTransactionRequest {
            agent_id: "test".into(),
            operations: vec![WikiOp::UpdateNote {
                id: note.id.clone(),
                title: None,
                content: Some("now has #fresh tag".into()),
                tags: None,
                claims: vec![],
            }],
            rationale: None,
            pending: false,
            require_provenance: false,
        };
        db.submit_wiki_transaction(req).unwrap();

        let by_fresh = db
            .list_notes(&crate::models::NoteListQuery {
                limit: 10,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: Some("fresh".into()),
            })
            .unwrap();
        assert_eq!(
            by_fresh.len(),
            1,
            "update_note must re-process inline #hashtags on content change \
             (Bug #2 regression)"
        );
    }

    #[test]
    fn update_note_explicit_tags_replace_existing() {
        let db = Database::new(":memory:").unwrap();
        let note = db
            .create_note(crate::models::CreateNoteRequest {
                title: "Tagged".into(),
                content: "body".into(),
                tags: vec!["old".into()],
            })
            .unwrap();

        let req = SubmitTransactionRequest {
            agent_id: "test".into(),
            operations: vec![WikiOp::UpdateNote {
                id: note.id.clone(),
                title: None,
                content: None,
                tags: Some(vec!["new".into()]),
                claims: vec![],
            }],
            rationale: None,
            pending: false,
            require_provenance: false,
        };
        db.submit_wiki_transaction(req).unwrap();

        let by_new = db
            .list_notes(&crate::models::NoteListQuery {
                limit: 10,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: Some("new".into()),
            })
            .unwrap();
        assert_eq!(by_new.len(), 1, "explicit tags Some(vec) must apply");
        let by_old = db
            .list_notes(&crate::models::NoteListQuery {
                limit: 10,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: Some("old".into()),
            })
            .unwrap();
        assert_eq!(
            by_old.len(),
            0,
            "old tags must be cleared by explicit replace"
        );
    }
}

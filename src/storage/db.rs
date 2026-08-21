use rusqlite::ffi::sqlite3_auto_extension;
use rusqlite::Connection;
use sqlite_vec::sqlite3_vec_init;
use std::path::Path;
use std::sync::Mutex;

use crate::errors::{AppError, AppResult};

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> AppResult<Self> {
        // Register sqlite-vec extension before opening any connection.
        // Safe to call multiple times — SQLite deduplicates auto-extensions.
        unsafe {
            #[allow(clippy::missing_transmute_annotations)]
            sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
        }

        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            let p = Path::new(path);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Connection::open(path)?
        };

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.initialize_schema()?;
        Ok(db)
    }

    fn initialize_schema(&self) -> AppResult<()> {
        let conn = self.conn.lock()
            .map_err(|e| AppError::MutexPoisoned(format!("Failed to lock connection: {}", e)))?;
        conn.execute_batch(SCHEMA)?;
        Self::apply_migrations(&conn);
        Ok(())
    }

    /// Idempotent migrations for columns/tables added after the initial schema.
    /// Each statement silently ignores errors if the migration was already applied.
    fn apply_migrations(conn: &Connection) {
        // Migration 001: bi-temporal edge columns (valid_from, valid_until)
        // Research ref: Zep/Graphiti arXiv:2501.13956 §3.2
        let _ = conn.execute_batch("ALTER TABLE links ADD COLUMN valid_from TEXT;");
        let _ = conn.execute_batch("ALTER TABLE links ADD COLUMN valid_until TEXT;");

        // Migration 002: sqlite-vec embedding table for semantic search
        // vec0 does not support IF NOT EXISTS, so ignore "already exists" errors.
        // Accepts any dimensionality — dimension is validated at insert time.
        // Research ref: Graph-Based Memory Survey arXiv:2602.05665
        let _ = conn.execute_batch(
            "CREATE VIRTUAL TABLE notes_vec USING vec0(
                note_id TEXT PRIMARY KEY,
                embedding float[384]
            );",
        );

        // Metadata table to track embedding dimensions and model info per note.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS note_embeddings_meta (
                note_id    TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                dimensions INTEGER NOT NULL,
                model      TEXT,
                created_at TEXT NOT NULL
            );",
        );

        // Migration 003: memory_history table for belief revision
        // Research ref: Graph-Native Belief Revision arXiv:2603.17244 — AGM postulates
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_history (
                id             TEXT PRIMARY KEY,
                agent_id       TEXT NOT NULL,
                namespace      TEXT NOT NULL,
                key            TEXT NOT NULL,
                value          TEXT NOT NULL,
                superseded_at  TEXT NOT NULL,
                superseded_by  TEXT NOT NULL,
                created_at     TEXT NOT NULL
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_memory_history_lookup
             ON memory_history(agent_id, namespace, key, superseded_at DESC);",
        );

        // ── Migration 004: Provenance layer ───────────────────────────────
        // Research refs:
        //   FACTUM — arXiv:2601.05866 (mechanistic citation hallucination detection)
        //   Citation-Grounded Code Comprehension — arXiv:2512.12117 (overlap invariant)
        //
        // Every claim synthesized by an agent MUST be traceable to a source
        // span. Smriti enforces structural overlap at commit time — an agent
        // cannot commit a note whose claim_spans fail verification.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sources (
                id           TEXT PRIMARY KEY,
                uri          TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                title        TEXT,
                excerpt      TEXT,
                ingested_at  TEXT NOT NULL,
                UNIQUE(uri, content_hash)
            );",
        );
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS claim_spans (
                id                 TEXT PRIMARY KEY,
                note_id            TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                claim_start        INTEGER NOT NULL,
                claim_end          INTEGER NOT NULL,
                source_id          TEXT NOT NULL REFERENCES sources(id) ON DELETE RESTRICT,
                source_span_start  INTEGER,
                source_span_end    INTEGER,
                verification_score REAL NOT NULL,
                verified_at        TEXT NOT NULL,
                method             TEXT NOT NULL
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_claim_spans_note ON claim_spans(note_id);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_claim_spans_source ON claim_spans(source_id);",
        );

        // ── Migration 005: Wiki transaction layer (atomic multi-write) ────
        // Research ref: no SOTA has this — unique to Smriti's SQLite substrate.
        // Every wiki_transaction lands as a single SAVEPOINT-wrapped commit.
        // Pending transactions allow human/agent review (GitHub-for-memory pattern).
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS wiki_transactions (
                id            TEXT PRIMARY KEY,
                agent_id      TEXT NOT NULL,
                status        TEXT NOT NULL,
                operations    TEXT NOT NULL,
                rationale     TEXT,
                created_at    TEXT NOT NULL,
                committed_at  TEXT,
                rejected_at   TEXT,
                rejected_by   TEXT,
                rejection_reason TEXT
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_wiki_tx_status ON wiki_transactions(status, created_at DESC);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_wiki_tx_agent ON wiki_transactions(agent_id, created_at DESC);",
        );

        // ── Migration 006: Contradiction events ───────────────────────────
        // Research refs:
        //   MemoTime — arXiv:2510.13614 (memory-augmented temporal KG reasoning)
        //   EvoReasoner/EvoKG — arXiv:2509.15464 (confidence-based contradiction resolution)
        //   AGM — arXiv:2603.17244 (belief revision theory)
        //
        // Contradictions are surfaced as an inbox for human review; Smriti
        // NEVER auto-resolves. A contradiction event carries the confidence
        // score s = w1·semantic + w2·recency + w3·authority.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS contradiction_events (
                id               TEXT PRIMARY KEY,
                note_id_a        TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                note_id_b        TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                semantic_score   REAL NOT NULL,
                recency_score    REAL NOT NULL,
                authority_score  REAL NOT NULL,
                combined_score   REAL NOT NULL,
                status           TEXT NOT NULL DEFAULT 'open',
                detected_at      TEXT NOT NULL,
                resolved_at      TEXT,
                resolution       TEXT,
                UNIQUE(note_id_a, note_id_b)
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_contradiction_status
             ON contradiction_events(status, combined_score DESC);",
        );

        // ── Migration 007: Event log (append-only, Zep T/T′ semantics) ────
        // Research ref: Zep/Graphiti — arXiv:2501.13956 (bi-temporal event time +
        // ingestion time). This table is the *projection source* for time-travel
        // queries. Projection cache (notes/links) can be rebuilt from this log.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id              TEXT PRIMARY KEY,
                event_type      TEXT NOT NULL,
                entity_type     TEXT NOT NULL,
                entity_id       TEXT NOT NULL,
                payload         TEXT NOT NULL,
                agent_id        TEXT,
                transaction_id  TEXT,
                event_time      TEXT NOT NULL,
                ingestion_time  TEXT NOT NULL,
                prev_hash       TEXT,
                event_hash      TEXT NOT NULL
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_events_entity
             ON events(entity_type, entity_id, event_time DESC);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_events_ingestion
             ON events(ingestion_time DESC);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_events_transaction
             ON events(transaction_id);",
        );

        // ── Migration 008: Multi-agent ACL grants ─────────────────────────
        // Lightweight permission primitive for multi-agent deployments.
        // Defers semantic grant discovery — ships the mechanism, not the policy.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_grants (
                id            TEXT PRIMARY KEY,
                grantor       TEXT NOT NULL,
                grantee       TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_glob TEXT NOT NULL,
                permission    TEXT NOT NULL,
                created_at    TEXT NOT NULL,
                expires_at    TEXT,
                UNIQUE(grantor, grantee, resource_type, resource_glob, permission)
            );",
        );

        // ── Migration 009: Consolidation + Schema Formation (CLS) ─────────
        // Research refs:
        //   McClelland, McNaughton, O'Reilly 1995 — Psych Review vol 102 (CLS)
        //   Kumaran, Hassabis, McClelland 2016 — Trends in Cognitive Sciences
        //
        // Episodes (hippocampus-like) are replayed; neocortex-like schemas
        // extract statistical regularities. Smriti mirrors this: frequently-
        // replayed, structurally central notes get promoted to schemas.
        // Rarely-accessed notes are flagged — NEVER hard-deleted. ICH E6(R3)
        // §4.1 compliance requires the full audit trail to remain intact.
        //
        // Deliberate design choices:
        //   - No division by age. Old + replayed + central = schema (CLS).
        //   - Scoring runs in a background pass, not on the request path.
        //   - Conservative is the default policy for any healthcare profile.

        // notes: consolidation columns (ALTER statements — SQLite can't IF NOT EXISTS them)
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN node_type TEXT NOT NULL DEFAULT 'episode';",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN consolidation_score REAL NOT NULL DEFAULT 0.0;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN last_accessed_at TEXT;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN parent_schema_id TEXT REFERENCES notes(id) ON DELETE SET NULL;",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_notes_consolidation
             ON notes(node_type, consolidation_score DESC);",
        );

        // Access log — feeds the replay signal for the scoring pass.
        // Written async from read paths; do NOT block request handlers.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS note_access_log (
                id              TEXT PRIMARY KEY,
                note_id         TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                accessed_at     TEXT NOT NULL,
                access_kind     TEXT NOT NULL,
                query_context   TEXT,
                query_embedding BLOB,
                agent_id        TEXT
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_access_note_time
             ON note_access_log(note_id, accessed_at DESC);",
        );

        // Lineage — which episodes a schema was abstracted from.
        // Required for `GET /api/v1/notes/:id/lineage` and audit replay.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_sources (
                schema_id        TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                source_note_id   TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                similarity_score REAL,
                consolidated_at  TEXT NOT NULL,
                PRIMARY KEY (schema_id, source_note_id)
            );",
        );

        // Auditable consolidation events — every promotion/flag/archive lands here.
        // This is the compliance substrate for ICH E6(R3) §4.1/§8.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS consolidation_events (
                id            TEXT PRIMARY KEY,
                note_id       TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                event_type    TEXT NOT NULL,
                score_before  REAL,
                score_after   REAL,
                reason        TEXT NOT NULL,
                created_at    TEXT NOT NULL
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_consolidation_events_note
             ON consolidation_events(note_id, created_at DESC);",
        );

        // ── Migration 010: LLM audit denormalization ──────────────────────
        // Spec: docs/superpowers/specs/2026-05-02-llm-integration-design.md §6
        //
        // Every LLM call writes one row to the existing `events` table
        // (event_type='llm_call') for hash-chain integrity. This table is a
        // query-performance denormalization — without it, "show all LLM calls
        // by agent X" requires walking JSON payloads in events.
        //
        // Stores hashes only (not raw prompts/responses) to minimize PHI
        // surface. Re-derivation: same notes + same prompt template version +
        // same temperature + same seed + same model = same hash.
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS llm_audit (
                id                       TEXT PRIMARY KEY,
                event_id                 TEXT NOT NULL REFERENCES events(id),
                agent_id                 TEXT NOT NULL,
                tool_name                TEXT NOT NULL,
                model                    TEXT NOT NULL,
                prompt_hash              TEXT NOT NULL,
                response_hash            TEXT,
                prompt_template_version  TEXT NOT NULL,
                note_ids                 TEXT NOT NULL,
                temperature              REAL NOT NULL,
                seed                     INTEGER,
                prompt_tokens            INTEGER,
                completion_tokens        INTEGER,
                duration_ms              INTEGER NOT NULL,
                outcome                  TEXT NOT NULL,
                error_message            TEXT,
                created_at               TEXT NOT NULL
            );",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_llm_audit_agent_time
             ON llm_audit(agent_id, created_at DESC);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_llm_audit_tool_time
             ON llm_audit(tool_name, created_at DESC);",
        );

        // ── Migration 011: Benna-Fusi cascade synapses ───────────────────
        // Research ref: Benna & Fusi 2016 — "Computational principles of
        // synaptic memory consolidation," Nature Neuroscience 19, 1697–1706.
        //
        // Each note carries a cascade state vector u = [u_0, ..., u_{K-1}]
        // representing per-level synaptic strengths at exponentially-spaced
        // timescales. Access events inject signal into u_0; over time, mass
        // cascades toward slower levels where it is robust to interference.
        //
        // The paper proves memory capacity scales as N/log(N) for the
        // multi-timescale cascade, vs √N for a single-timescale synapse —
        // the load-bearing claim for Smriti's "memory that gets cleaner
        // over time" positioning. Single-scalar consolidation_score
        // (Migration 009) becomes a derived view on this richer state.
        //
        // Storage is JSON-encoded for round-trip simplicity. A normalised
        // (note_id, level, value) table is a candidate future migration if
        // we ever need cross-note level-K queries. Today every read is
        // per-note → JSON is cheaper.
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN cascade_state TEXT;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN cascade_updated_at TEXT;",
        );
    }

    pub fn execute<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&Connection) -> AppResult<T>,
    {
        let conn = self.conn.lock()
            .map_err(|e| AppError::MutexPoisoned(format!("Failed to lock connection: {}", e)))?;
        f(&conn)
    }
}

const SCHEMA: &str = r#"
-- Notes table
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Full-text search using FTS5
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    content,
    content=notes,
    content_rowid=rowid,
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO notes_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

-- Tags table
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT,
    created_at TEXT NOT NULL
);

-- Note-to-tag junction
CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (note_id, tag_id),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- Links between notes (wiki-links, backlinks, AI-suggested)
-- Bi-temporal model: valid_from/valid_until track when the relationship holds.
-- Research ref: Zep/Graphiti arXiv:2501.13956 §3.2
CREATE TABLE IF NOT EXISTS links (
    id TEXT PRIMARY KEY,
    source_note_id TEXT NOT NULL,
    target_note_id TEXT NOT NULL,
    link_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    valid_from TEXT,
    valid_until TEXT,
    FOREIGN KEY (source_note_id) REFERENCES notes(id) ON DELETE CASCADE,
    FOREIGN KEY (target_note_id) REFERENCES notes(id) ON DELETE CASCADE,
    UNIQUE(source_note_id, target_note_id, link_type)
);

-- Agent memory store (key-value per agent with namespaces)
CREATE TABLE IF NOT EXISTS agent_memory (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    namespace TEXT NOT NULL DEFAULT 'default',
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    ttl_seconds INTEGER,
    UNIQUE(agent_id, namespace, key)
);

-- Tool execution logs for agents
CREATE TABLE IF NOT EXISTS tool_logs (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    input TEXT NOT NULL,
    output TEXT NOT NULL,
    status TEXT NOT NULL,
    duration_ms INTEGER,
    created_at TEXT NOT NULL
);

-- Sync metadata for cross-device sync
CREATE TABLE IF NOT EXISTS sync_state (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    last_synced_at TEXT NOT NULL,
    device_id TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    UNIQUE(entity_type, entity_id, device_id)
);

-- Daily insights cache (AI-generated)
CREATE TABLE IF NOT EXISTS daily_insights (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL UNIQUE,
    summary TEXT NOT NULL,
    suggested_links TEXT NOT NULL,
    trending_topics TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_notes_updated ON notes(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_notes_created ON notes(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_note_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_note_id);
CREATE INDEX IF NOT EXISTS idx_links_type ON links(link_type);
CREATE INDEX IF NOT EXISTS idx_note_tags_note ON note_tags(note_id);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_agent_memory_agent ON agent_memory(agent_id, namespace);
CREATE INDEX IF NOT EXISTS idx_tool_logs_agent ON tool_logs(agent_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_entity ON sync_state(entity_type, entity_id);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_database() {
        let db = Database::new(":memory:").unwrap();
        db.execute(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }
}

#[cfg(test)]
mod migration_010_tests {
    use super::Database;

    #[test]
    fn llm_audit_table_exists_after_migration() {
        let db = Database::new(":memory:").expect("open in-memory db");
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='llm_audit'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "llm_audit table should exist after migration 010");
    }

    #[test]
    fn llm_audit_indexes_exist() {
        let db = Database::new(":memory:").unwrap();
        let conn = db.conn.lock().unwrap();
        let agent_idx: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_llm_audit_agent_time'",
            [], |row| row.get(0)).unwrap();
        let tool_idx: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_llm_audit_tool_time'",
            [], |row| row.get(0)).unwrap();
        assert_eq!(agent_idx, 1);
        assert_eq!(tool_idx, 1);
    }
}

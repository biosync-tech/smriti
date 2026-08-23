use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::*;
use crate::storage::Database;

/// Sanitize a user-supplied search string so it is safe to pass as the
/// right-hand side of an FTS5 `MATCH` predicate.
///
/// FTS5 query syntax is a DSL distinct from SQL: `-` means NOT, bare
/// integers are column references, and `*`, `:`, `(`, `^`, `"` are
/// reserved. Concatenating raw user input is the FTS5-equivalent of SQL
/// injection — see Su & Wassermann (POPL'06).
///
/// This function wraps the query as a single literal phrase, doubling
/// any embedded double-quote per the FTS5 phrase-quoting rule. Output
/// is always parseable as a plain phrase match, never a query DSL
/// expression.
///
/// Reference: SQLite FTS5 documentation §3 Full-text Query Syntax,
/// https://sqlite.org/fts5.html#full_text_query_syntax
pub(crate) fn sanitize_fts5_query(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        // Empty phrase " " matches nothing, which is the correct
        // semantic for an empty user query.
        return "\"\"".to_string();
    }
    let escaped = trimmed.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

// ─── Note Operations ────────────────────────────────────────────────

impl Database {
    pub fn create_note(&self, req: CreateNoteRequest) -> AppResult<Note> {
        self.execute(|conn| insert_note_with_tags(conn, req))
    }

    pub fn get_note(&self, id: &str) -> AppResult<Note> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, content, created_at, updated_at, node_type, consolidation_score, access_count, last_accessed_at, parent_schema_id FROM notes WHERE id = ?1",
            )?;

            let note = stmt
                .query_row(params![id], |row| {
                    Ok(Note {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                            .unwrap_or_default()
                            .with_timezone(&Utc),
                        updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                            .unwrap_or_default()
                            .with_timezone(&Utc),
                        tags: Vec::new(),
                        backlink_count: 0,
                        wikilink_count: 0,
                        node_type: NodeType::parse(&row.get::<_, String>(5).unwrap_or_default()),
                        consolidation_score: row.get::<_, f64>(6).unwrap_or(0.0) as f32,
                        access_count: row.get::<_, i64>(7).unwrap_or(0) as u64,
                        last_accessed_at: row.get::<_, Option<String>>(8)?
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                        parent_schema_id: row.get(9)?,
                    })
                })
                .map_err(|_| AppError::NoteNotFound(id.to_string()))?;

            // Fetch tags
            let tags = get_note_tags(conn, &note.id)?;
            let backlink_count = count_backlinks(conn, &note.id)?;
            let wikilink_count = count_wikilinks(conn, &note.id)?;

            Ok(Note {
                tags,
                backlink_count,
                wikilink_count,
                ..note
            })
        })
    }

    pub fn update_note(&self, id: &str, req: UpdateNoteRequest) -> AppResult<Note> {
        self.execute(|conn| {
            // Check exists
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM notes WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if !exists {
                return Err(AppError::NoteNotFound(id.to_string()));
            }

            let now = Utc::now().to_rfc3339();

            if let Some(title) = &req.title {
                conn.execute(
                    "UPDATE notes SET title = ?1, updated_at = ?2 WHERE id = ?3",
                    params![title, now, id],
                )?;
            }

            if let Some(content) = &req.content {
                conn.execute(
                    "UPDATE notes SET content = ?1, updated_at = ?2 WHERE id = ?3",
                    params![content, now, id],
                )?;
            }

            if let Some(tags) = &req.tags {
                // Clear existing tags
                conn.execute("DELETE FROM note_tags WHERE note_id = ?1", params![id])?;
                for tag_name in tags {
                    ensure_tag(conn, tag_name)?;
                    let tag_id = get_tag_id(conn, tag_name)?;
                    conn.execute(
                        "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                        params![id, tag_id],
                    )?;
                }
            }

            Ok(())
        })?;

        self.get_note(id)
    }

    pub fn delete_note(&self, id: &str) -> AppResult<()> {
        self.execute(|conn| {
            let affected = conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
            if affected == 0 {
                return Err(AppError::NoteNotFound(id.to_string()));
            }
            // Cascade handles note_tags and links
            Ok(())
        })
    }

    pub fn list_notes(&self, query: &NoteListQuery) -> AppResult<Vec<NoteSummary>> {
        self.execute(|conn| {
            let order_clause = match query.sort {
                SortOrder::UpdatedDesc => "updated_at DESC",
                SortOrder::UpdatedAsc => "updated_at ASC",
                SortOrder::CreatedDesc => "created_at DESC",
                SortOrder::CreatedAsc => "created_at ASC",
                SortOrder::TitleAsc => "title ASC",
            };

            let sql = if let Some(_tag) = &query.tag {
                format!(
                    "SELECT n.id, n.title, n.content, n.created_at, n.updated_at
                     FROM notes n
                     JOIN note_tags nt ON n.id = nt.note_id
                     JOIN tags t ON nt.tag_id = t.id
                     WHERE t.name = ?3
                     ORDER BY n.{} LIMIT ?1 OFFSET ?2",
                    order_clause
                )
            } else {
                format!(
                    "SELECT id, title, content, created_at, updated_at
                     FROM notes ORDER BY {} LIMIT ?1 OFFSET ?2",
                    order_clause
                )
            };

            let mut stmt = conn.prepare(&sql)?;

            let mut summaries = Vec::new();
            if let Some(tag) = &query.tag {
                let rows = stmt.query_map(params![query.limit, query.offset, tag], |row| {
                    build_note_summary(row)
                })?;
                for row in rows {
                    let mut summary = row?;
                    summary.tag_count = get_note_tags(conn, &summary.id).unwrap_or_default().len();
                    summary.backlink_count = count_backlinks(conn, &summary.id).unwrap_or(0);
                    summaries.push(summary);
                }
            } else {
                let rows = stmt.query_map(params![query.limit, query.offset], |row| {
                    build_note_summary(row)
                })?;
                for row in rows {
                    let mut summary = row?;
                    summary.tag_count = get_note_tags(conn, &summary.id).unwrap_or_default().len();
                    summary.backlink_count = count_backlinks(conn, &summary.id).unwrap_or(0);
                    summaries.push(summary);
                }
            };

            Ok(summaries)
        })
    }

    pub fn search_notes(&self, query: &SearchQuery) -> AppResult<Vec<NoteSummary>> {
        // Sanitize the user-supplied query string before handing it to FTS5.
        //
        // FTS5's MATCH clause is its own DSL: `-` is unary NOT, bare integers
        // are column references, `:`/`*`/`(`/`^`/`"` carry meaning. A query
        // like `UTF-8 smoke` is parsed as `UTF MATCH (NOT 8) MATCH smoke`,
        // raising `no such column: 8` and bubbling a SQL error to the caller.
        //
        // Defensive fix: wrap the entire query as a literal FTS5 phrase
        // ("..."), doubling any embedded `"` per the FTS5 phrase-quoting
        // rules (SQLite FTS5 docs §3 Full-text Query Syntax). This trades
        // user-facing operator syntax (which Smriti does not document for
        // notes_search) for total robustness against the FTS5-syntax-
        // injection class of failures (`-`, `:`, `*`, `(`, `^`, `"`).
        //
        // Rationale anchors:
        //   * SQLite FTS5 reference §3 — phrase quoting is the prescribed
        //     escape for arbitrary user input.
        //   * Su & Wassermann 2006, "The Essence of Command Injections to
        //     Web Applications" (POPL'06) — formalizes injection as the
        //     mixing of user input with a target query DSL; the prescribed
        //     mitigation is structural separation (phrase-wrap here).
        //   * Howard & Lipner 2006, "The Security Development Lifecycle"
        //     §6 — input-validation as a layer the database cannot be
        //     trusted to provide.
        let q_safe = sanitize_fts5_query(&query.q);

        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT n.id, n.title, n.content, n.created_at, n.updated_at
                 FROM notes n
                 JOIN notes_fts fts ON n.rowid = fts.rowid
                 WHERE notes_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2 OFFSET ?3",
            )?;

            let rows = stmt.query_map(params![q_safe, query.limit, query.offset], |row| {
                build_note_summary(row)
            })?;

            let mut summaries = Vec::new();
            for row in rows {
                summaries.push(row?);
            }
            Ok(summaries)
        })
    }

    pub fn get_note_by_title(&self, title: &str) -> AppResult<Option<Note>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, content, created_at, updated_at, node_type, consolidation_score, access_count, last_accessed_at, parent_schema_id FROM notes WHERE title = ?1",
            )?;

            let result = stmt.query_row(params![title], |row| {
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    tags: Vec::new(),
                    backlink_count: 0,
                    wikilink_count: 0,
                    node_type: NodeType::parse(&row.get::<_, String>(5).unwrap_or_default()),
                    consolidation_score: row.get::<_, f64>(6).unwrap_or(0.0) as f32,
                    access_count: row.get::<_, i64>(7).unwrap_or(0) as u64,
                    last_accessed_at: row.get::<_, Option<String>>(8)?
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    parent_schema_id: row.get(9)?,
                })
            });

            match result {
                Ok(note) => Ok(Some(note)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::Database(e)),
            }
        })
    }

    // ─── Link Operations ────────────────────────────────────────────

    pub fn create_link(
        &self,
        source_id: &str,
        target_id: &str,
        link_type: LinkType,
    ) -> AppResult<Link> {
        self.execute(|conn| insert_link_on_conn(conn, source_id, target_id, link_type))
    }

    /// BFS over currently-valid links. Does not load the full note table.
    pub fn graph_neighbors(
        &self,
        seed_ids: &[&str],
        depth: usize,
        limit: usize,
    ) -> AppResult<Vec<String>> {
        if depth == 0 || seed_ids.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        self.execute(|conn| {
            let mut frontier: Vec<String> = seed_ids.iter().map(|s| (*s).to_string()).collect();
            let mut seen: std::collections::HashSet<String> =
                frontier.iter().cloned().collect();
            let mut out: Vec<String> = Vec::new();

            for _ in 0..depth {
                if frontier.is_empty() || out.len() >= limit {
                    break;
                }
                let mut next = Vec::new();
                for id in &frontier {
                    let mut stmt = conn.prepare(
                        "SELECT target_note_id FROM links
                         WHERE source_note_id = ?1 AND valid_until IS NULL
                         UNION
                         SELECT source_note_id FROM links
                         WHERE target_note_id = ?1 AND valid_until IS NULL",
                    )?;
                    let rows = stmt.query_map(params![id], |r| r.get::<_, String>(0))?;
                    for row in rows {
                        let nid = row?;
                        if seen.insert(nid.clone()) {
                            next.push(nid.clone());
                            out.push(nid);
                            if out.len() >= limit {
                                return Ok(out);
                            }
                        }
                    }
                }
                frontier = next;
            }
            Ok(out)
        })
    }

    /// Notes that have no row in `note_embeddings_meta` — one query, no N+1.
    pub fn list_note_ids_missing_embeddings(&self) -> AppResult<Vec<String>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT n.id FROM notes n
                 LEFT JOIN note_embeddings_meta m ON m.note_id = n.id
                 WHERE m.note_id IS NULL",
            )?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row?);
            }
            Ok(ids)
        })
    }

    /// Fetch many notes in one lock. Order follows `ids`. Missing ids are skipped.
    pub fn get_notes_by_ids(&self, ids: &[String]) -> AppResult<Vec<Note>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut notes = Vec::with_capacity(ids.len());
        for id in ids {
            match self.get_note(id) {
                Ok(n) => notes.push(n),
                Err(AppError::NoteNotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(notes)
    }

    pub fn get_backlinks(&self, note_id: &str) -> AppResult<Vec<NoteSummary>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT n.id, n.title, n.content, n.created_at, n.updated_at
                 FROM notes n
                 JOIN links l ON n.id = l.source_note_id
                 WHERE l.target_note_id = ?1",
            )?;

            let rows = stmt.query_map(params![note_id], build_note_summary)?;
            let mut summaries = Vec::new();
            for row in rows {
                summaries.push(row?);
            }
            Ok(summaries)
        })
    }

    pub fn get_forward_links(&self, note_id: &str) -> AppResult<Vec<NoteSummary>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT n.id, n.title, n.content, n.created_at, n.updated_at
                 FROM notes n
                 JOIN links l ON n.id = l.target_note_id
                 WHERE l.source_note_id = ?1",
            )?;

            let rows = stmt.query_map(params![note_id], build_note_summary)?;
            let mut summaries = Vec::new();
            for row in rows {
                summaries.push(row?);
            }
            Ok(summaries)
        })
    }

    pub fn get_all_links(&self) -> AppResult<Vec<Link>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_note_id, target_note_id, link_type, created_at, valid_from, valid_until FROM links",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(Link {
                    id: row.get(0)?,
                    source_note_id: row.get(1)?,
                    target_note_id: row.get(2)?,
                    link_type: LinkType::parse(&row.get::<_, String>(3)?),
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    valid_from: row.get::<_, Option<String>>(5)?
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    valid_until: row.get::<_, Option<String>>(6)?
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            })?;

            let mut links = Vec::new();
            for row in rows {
                links.push(row?);
            }
            Ok(links)
        })
    }

    // ─── Agent Memory Operations ────────────────────────────────────

    /// Store a memory entry with conflict resolution policy.
    /// Research ref: Graph-Native Belief Revision arXiv:2603.17244 — AGM postulates.
    pub fn store_memory(&self, agent_id: &str, req: CreateMemoryRequest) -> AppResult<AgentMemory> {
        let ns = req.namespace.unwrap_or_else(|| "default".to_string());
        let policy = req.conflict_policy;
        let memory = AgentMemory::new(
            agent_id.to_string(),
            ns,
            req.key,
            req.value,
            req.ttl_seconds,
        );

        self.execute(|conn| {
            let value_json = serde_json::to_string(&memory.value)?;

            match policy {
                ConflictPolicy::Overwrite => {
                    conn.execute(
                        "INSERT INTO agent_memory (id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(agent_id, namespace, key) DO UPDATE SET
                            value = excluded.value,
                            updated_at = excluded.updated_at,
                            ttl_seconds = excluded.ttl_seconds",
                        params![
                            memory.id, memory.agent_id, memory.namespace, memory.key,
                            value_json, memory.created_at.to_rfc3339(),
                            memory.updated_at.to_rfc3339(), memory.ttl_seconds,
                        ],
                    )?;
                }
                ConflictPolicy::Reject => {
                    let exists: bool = conn
                        .query_row(
                            "SELECT COUNT(*) > 0 FROM agent_memory
                             WHERE agent_id = ?1 AND namespace = ?2 AND key = ?3",
                            params![memory.agent_id, memory.namespace, memory.key],
                            |row| row.get(0),
                        )
                        .unwrap_or(false);

                    if exists {
                        return Err(AppError::Conflict(format!(
                            "Memory key already exists: {}/{}/{}",
                            memory.agent_id, memory.namespace, memory.key
                        )));
                    }

                    conn.execute(
                        "INSERT INTO agent_memory (id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            memory.id, memory.agent_id, memory.namespace, memory.key,
                            value_json, memory.created_at.to_rfc3339(),
                            memory.updated_at.to_rfc3339(), memory.ttl_seconds,
                        ],
                    )?;
                }
                ConflictPolicy::VersionAndKeep => {
                    // Archive existing value if present
                    archive_old_memory(conn, &memory.agent_id, &memory.namespace, &memory.key, &memory.id)?;

                    conn.execute(
                        "INSERT INTO agent_memory (id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(agent_id, namespace, key) DO UPDATE SET
                            value = excluded.value,
                            updated_at = excluded.updated_at,
                            ttl_seconds = excluded.ttl_seconds",
                        params![
                            memory.id, memory.agent_id, memory.namespace, memory.key,
                            value_json, memory.created_at.to_rfc3339(),
                            memory.updated_at.to_rfc3339(), memory.ttl_seconds,
                        ],
                    )?;
                }
                ConflictPolicy::Invalidate => {
                    // Mark existing value as superseded with timestamp, then archive
                    archive_old_memory(conn, &memory.agent_id, &memory.namespace, &memory.key, &memory.id)?;

                    conn.execute(
                        "INSERT INTO agent_memory (id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(agent_id, namespace, key) DO UPDATE SET
                            value = excluded.value,
                            updated_at = excluded.updated_at,
                            ttl_seconds = excluded.ttl_seconds",
                        params![
                            memory.id, memory.agent_id, memory.namespace, memory.key,
                            value_json, memory.created_at.to_rfc3339(),
                            memory.updated_at.to_rfc3339(), memory.ttl_seconds,
                        ],
                    )?;
                }
            }

            Ok(memory)
        })
    }

    pub fn get_memory(&self, agent_id: &str, namespace: &str, key: &str) -> AppResult<AgentMemory> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds
                 FROM agent_memory
                 WHERE agent_id = ?1 AND namespace = ?2 AND key = ?3",
            )?;

            stmt.query_row(params![agent_id, namespace, key], |row| {
                Ok(AgentMemory {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    namespace: row.get(2)?,
                    key: row.get(3)?,
                    value: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    ttl_seconds: row.get(7)?,
                })
            })
            .map_err(|_| AppError::AgentNotFound(format!("{}/{}/{}", agent_id, namespace, key)))
        })
    }

    pub fn list_agent_memory(
        &self,
        agent_id: &str,
        namespace: Option<&str>,
    ) -> AppResult<Vec<AgentMemory>> {
        self.execute(|conn| {
            let (sql, ns) = if let Some(ns) = namespace {
                (
                    "SELECT id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds
                     FROM agent_memory WHERE agent_id = ?1 AND namespace = ?2
                     ORDER BY updated_at DESC",
                    Some(ns.to_string()),
                )
            } else {
                (
                    "SELECT id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds
                     FROM agent_memory WHERE agent_id = ?1
                     ORDER BY updated_at DESC",
                    None,
                )
            };

            let mut stmt = conn.prepare(sql)?;

            let mut memories = Vec::new();
            if let Some(ns) = &ns {
                let rows = stmt.query_map(params![agent_id, ns], build_memory_row)?;
                for row in rows {
                    let mem: AgentMemory = row?;
                    if !mem.is_expired() {
                        memories.push(mem);
                    }
                }
            } else {
                let rows = stmt.query_map(params![agent_id], build_memory_row)?;
                for row in rows {
                    let mem: AgentMemory = row?;
                    if !mem.is_expired() {
                        memories.push(mem);
                    }
                }
            };
            Ok(memories)
        })
    }

    /// Get history of superseded values for a memory key.
    pub fn get_memory_history(
        &self,
        agent_id: &str,
        namespace: &str,
        key: &str,
    ) -> AppResult<Vec<MemoryHistoryEntry>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, namespace, key, value, superseded_at, superseded_by, created_at
                 FROM memory_history
                 WHERE agent_id = ?1 AND namespace = ?2 AND key = ?3
                 ORDER BY superseded_at DESC",
            )?;

            let rows = stmt.query_map(params![agent_id, namespace, key], |row| {
                Ok(MemoryHistoryEntry {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    namespace: row.get(2)?,
                    key: row.get(3)?,
                    value: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                    superseded_at: chrono::DateTime::parse_from_rfc3339(
                        &row.get::<_, String>(5)?,
                    )
                    .unwrap_or_default()
                    .with_timezone(&Utc),
                    superseded_by: row.get(6)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(
                        &row.get::<_, String>(7)?,
                    )
                    .unwrap_or_default()
                    .with_timezone(&Utc),
                })
            })?;

            let mut entries = Vec::new();
            for row in rows {
                entries.push(row?);
            }
            Ok(entries)
        })
    }

    pub fn log_tool_call(&self, agent_id: &str, req: CreateToolLogRequest) -> AppResult<ToolLog> {
        let log = ToolLog::new(
            agent_id.to_string(),
            req.tool_name,
            req.input,
            req.output,
            req.status,
            req.duration_ms,
        );

        self.execute(|conn| {
            conn.execute(
                "INSERT INTO tool_logs (id, agent_id, tool_name, input, output, status, duration_ms, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    log.id,
                    log.agent_id,
                    log.tool_name,
                    serde_json::to_string(&log.input)?,
                    serde_json::to_string(&log.output)?,
                    log.status.as_str(),
                    log.duration_ms,
                    log.created_at.to_rfc3339(),
                ],
            )?;
            Ok(log)
        })
    }

    pub fn get_tool_logs(&self, agent_id: &str, limit: usize) -> AppResult<Vec<ToolLog>> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, tool_name, input, output, status, duration_ms, created_at
                 FROM tool_logs WHERE agent_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![agent_id, limit], |row| {
                Ok(ToolLog {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    tool_name: row.get(2)?,
                    input: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    output: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                    status: ToolStatus::parse(&row.get::<_, String>(5)?),
                    duration_ms: row.get(6)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                })
            })?;

            let mut logs = Vec::new();
            for row in rows {
                logs.push(row?);
            }
            Ok(logs)
        })
    }

    // ─── Stats ──────────────────────────────────────────────────────

    pub fn get_stats(&self) -> AppResult<GraphStats> {
        self.execute(|conn| {
            let total_notes: usize = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?;
            let total_links: usize = conn.query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))?;
            let total_tags: usize = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))?;

            let orphan_notes: usize = conn.query_row(
                "SELECT COUNT(*) FROM notes n
                 WHERE NOT EXISTS (SELECT 1 FROM links l WHERE l.source_note_id = n.id OR l.target_note_id = n.id)",
                [],
                |r| r.get(0),
            )?;

            let most_linked: Option<String> = conn
                .query_row(
                    "SELECT n.title FROM notes n
                     JOIN links l ON n.id = l.target_note_id
                     GROUP BY n.id ORDER BY COUNT(*) DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .ok();

            Ok(GraphStats {
                total_notes,
                total_links,
                total_tags,
                orphan_notes,
                most_linked,
            })
        })
    }

    // ─── Embedding / Semantic Search Operations ────────────────────

    /// Store a pre-computed embedding vector for a note.
    /// Replaces any existing embedding for this note.
    pub fn store_embedding(
        &self,
        note_id: &str,
        embedding: &[f32],
        model: Option<&str>,
    ) -> AppResult<()> {
        // Verify note exists
        let _ = self.get_note(note_id)?;

        self.execute(|conn| {
            let existing_dim: Option<i64> = conn
                .query_row(
                    "SELECT dimensions FROM note_embeddings_meta LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(width) = existing_dim {
                if width as usize != embedding.len() {
                    return Err(AppError::BadRequest(format!(
                        "embedding is {}-d but this database already stores {}-d vectors. \
                         Plug in any embed model whose width matches, or start a new db \
                         / recreate notes_vec after a full re-embed.",
                        embedding.len(),
                        width
                    )));
                }
            }

            let byte_slice = zerocopy::IntoBytes::as_bytes(embedding);

            // Delete existing embedding if any, then insert new one
            conn.execute("DELETE FROM notes_vec WHERE note_id = ?1", params![note_id])?;
            conn.execute(
                "INSERT INTO notes_vec (note_id, embedding) VALUES (?1, ?2)",
                params![note_id, byte_slice],
            )?;

            // Upsert metadata
            conn.execute(
                "INSERT INTO note_embeddings_meta (note_id, dimensions, model, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(note_id) DO UPDATE SET
                    dimensions = excluded.dimensions,
                    model = excluded.model,
                    created_at = excluded.created_at",
                params![note_id, embedding.len(), model, Utc::now().to_rfc3339(),],
            )?;

            Ok(())
        })
    }

    /// KNN semantic search using sqlite-vec cosine distance.
    /// Returns note IDs + distances, sorted by distance ascending.
    pub fn semantic_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> AppResult<Vec<SemanticSearchResult>> {
        self.execute(|conn| {
            let byte_slice = zerocopy::IntoBytes::as_bytes(query_embedding);

            let mut stmt = conn.prepare(
                "SELECT v.note_id, v.distance, n.title, n.content
                 FROM notes_vec v
                 JOIN notes n ON v.note_id = n.id
                 WHERE v.embedding MATCH ?1
                 AND k = ?2
                 ORDER BY v.distance",
            )?;

            let rows = stmt.query_map(params![byte_slice, limit], |row| {
                let content: String = row.get(3)?;
                let preview = if content.len() > 200 {
                    format!("{}...", crate::safe_truncate(&content, 200))
                } else {
                    content
                };
                Ok(SemanticSearchResult {
                    id: row.get(0)?,
                    distance: row.get(1)?,
                    title: row.get(2)?,
                    preview,
                })
            })?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            Ok(results)
        })
    }

    /// Hybrid search: combines FTS5 keyword search with sqlite-vec semantic search.
    /// Uses reciprocal rank fusion to merge both result sets.
    ///
    /// Research ref: Graph-Based Memory Survey arXiv:2602.05665 —
    /// "Graph+BM25 hybrid beats pure vector for multi-hop tasks"
    ///
    /// Thin wrapper over [`Self::hybrid_search_with_salience`] with
    /// `salience_weight = 0.0` — behaviorally identical to the pre-existing
    /// fusion, kept as the default entry point so existing callers
    /// (`retrieve_context`, the `/api/v1/notes/search` semantic route) are
    /// unaffected until the salience term is validated.
    pub fn hybrid_search(
        &self,
        text_query: &str,
        query_embedding: &[f32],
        limit: usize,
        fts_weight: f64,
    ) -> AppResult<Vec<HybridSearchResult>> {
        self.hybrid_search_with_salience(text_query, query_embedding, limit, fts_weight, 0.0)
    }

    /// Hybrid search with an optional consolidation-salience nudge.
    ///
    /// `salience_weight` (λ) applies a bounded multiplicative boost to the
    /// combined RRF score of each candidate that already matched via FTS
    /// or semantic search:
    ///
    /// ```text
    /// final_score = rrf_combined * (1 + λ * consolidation_score)
    /// ```
    ///
    /// Design notes (do not regress these without re-reading Task 9 /
    /// cascade.rs):
    ///
    /// - Reads the **persisted** `notes.consolidation_score` column, not a
    ///   live `cascade::salience_peek_for_note()` call. The live peek does
    ///   bounded-but-real explicit-Euler substepping (up to ~5-6k substeps
    ///   for a note idle 30 days) — calling that per candidate on the
    ///   request path would violate the "request path stays p50-friendly"
    ///   constraint even though the peek itself doesn't mutate state.
    ///   `consolidation_score` is written offline by
    ///   `consolidation::run_consolidation_pass`; reading it here is a
    ///   single indexed `SELECT ... WHERE id IN (...)` over the already
    ///   size-bounded candidate set.
    /// - The boost is **multiplicative around a baseline of 1.0**, not an
    ///   independent ranking signal fused via RRF. `consolidation_score`
    ///   defaults to 0.0 for every note that has never been scored
    ///   (Migration 009 default), so a brand-new or never-replayed note
    ///   gets boost = 1.0 — neutral, never penalized. Fusing salience as a
    ///   third RRF term instead (ranked globally by salience) would reward
    ///   "generically well-worn" over "relevant to this query," and would
    ///   actively bury exactly the content Smriti's compliance use cases
    ///   depend on — a fresh protocol amendment or revised guidance note
    ///   necessarily starts at consolidation_score = 0.0.
    /// - Restricted to candidates already present in `scores` (i.e.
    ///   already matched by FTS or semantic search). This can only
    ///   re-order among relevant results; it cannot pull in an unrelated
    ///   note purely because it's heavily consolidated.
    /// - Residual, accepted risk: among two *near-tied* candidates of
    ///   similar relevance, the boost can tip the tie toward the older,
    ///   more-replayed one even if a newer one is more current. Keep λ
    ///   small (starting default recommendation: 0.1) and validate with a
    ///   labeled recall benchmark before raising it — see the
    ///   `salience_boost_*` tests in this file's `tests` module.
    pub fn hybrid_search_with_salience(
        &self,
        text_query: &str,
        query_embedding: &[f32],
        limit: usize,
        fts_weight: f64,
        salience_weight: f64,
    ) -> AppResult<Vec<HybridSearchResult>> {
        // Run both searches
        let fts_results = self.search_notes(&SearchQuery {
            q: text_query.to_string(),
            limit: limit * 2, // over-fetch for better fusion
            offset: 0,
        })?;

        let semantic_results = self.semantic_search(query_embedding, limit * 2)?;

        // Reciprocal Rank Fusion (k=60 is standard)
        let k = 60.0_f64;
        let sem_weight = 1.0 - fts_weight;

        let mut scores: std::collections::HashMap<String, (f64, String, String, String)> =
            std::collections::HashMap::new();

        // Score FTS results by rank position
        for (rank, note) in fts_results.iter().enumerate() {
            let rrf = fts_weight / (k + rank as f64 + 1.0);
            scores
                .entry(note.id.clone())
                .and_modify(|(s, _, _, src)| {
                    *s += rrf;
                    *src = "both".into();
                })
                .or_insert((rrf, note.title.clone(), note.preview.clone(), "fts".into()));
        }

        // Score semantic results by rank position
        for (rank, result) in semantic_results.iter().enumerate() {
            let rrf = sem_weight / (k + rank as f64 + 1.0);
            scores
                .entry(result.id.clone())
                .and_modify(|(s, _, _, src)| {
                    *s += rrf;
                    if *src == "fts" {
                        *src = "both".into();
                    }
                })
                .or_insert((
                    rrf,
                    result.title.clone(),
                    result.preview.clone(),
                    "semantic".into(),
                ));
        }

        // Bounded salience nudge — persisted score only, candidates already
        // matched only. See doc comment above for why this shape and not a
        // third RRF term / live cascade peek.
        if salience_weight > 0.0 && !scores.is_empty() {
            let ids: Vec<String> = scores.keys().cloned().collect();
            let placeholders = std::iter::repeat("?")
                .take(ids.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT id, consolidation_score FROM notes WHERE id IN ({})",
                placeholders
            );
            self.execute(|conn| {
                let mut stmt = conn.prepare(&sql)?;
                let mut rows = stmt.query(rusqlite::params_from_iter(ids.iter()))?;
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let cscore: f64 = row.get(1)?;
                    if let Some(entry) = scores.get_mut(&id) {
                        let boost = 1.0 + salience_weight * cscore.clamp(0.0, 1.0);
                        entry.0 *= boost;
                    }
                }
                Ok(())
            })?;
        }

        // Sort by combined score descending (higher RRF = better)
        let mut results: Vec<HybridSearchResult> = scores
            .into_iter()
            .map(|(id, (score, title, preview, source))| HybridSearchResult {
                id,
                title,
                preview,
                score,
                match_source: source,
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        results.truncate(limit);

        Ok(results)
    }
}

// ─── Helper Functions ───────────────────────────────────────────────

/// Archive the current value of a memory key to memory_history before overwriting.
/// No-op if no existing value.
fn archive_old_memory(
    conn: &Connection,
    agent_id: &str,
    namespace: &str,
    key: &str,
    superseded_by: &str,
) -> AppResult<()> {
    let old = conn
        .prepare(
            "SELECT id, value, created_at FROM agent_memory
             WHERE agent_id = ?1 AND namespace = ?2 AND key = ?3",
        )?
        .query_row(params![agent_id, namespace, key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        });

    if let Ok((old_id, old_value, old_created_at)) = old {
        conn.execute(
            "INSERT INTO memory_history (id, agent_id, namespace, key, value, superseded_at, superseded_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                old_id,
                agent_id,
                namespace,
                key,
                old_value,
                Utc::now().to_rfc3339(),
                superseded_by,
                old_created_at,
            ],
        )?;
    }

    Ok(())
}

pub(crate) fn insert_note_with_tags(
    conn: &Connection,
    req: CreateNoteRequest,
) -> AppResult<Note> {
    let note = Note::new(req.title, req.content, req.tags.clone());
    conn.execute(
        "INSERT INTO notes (id, title, content, created_at, updated_at, node_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            note.id,
            note.title,
            note.content,
            note.created_at.to_rfc3339(),
            note.updated_at.to_rfc3339(),
            note.node_type.as_str(),
        ],
    )?;
    for tag_name in &req.tags {
        ensure_tag(conn, tag_name)?;
        let tag_id = get_tag_id(conn, tag_name)?;
        conn.execute(
            "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![note.id, tag_id],
        )?;
    }
    Ok(note)
}

pub(crate) fn insert_link_on_conn(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    link_type: LinkType,
) -> AppResult<Link> {
    let link = Link::new(source_id.to_string(), target_id.to_string(), link_type);
    conn.execute(
        "INSERT OR IGNORE INTO links (id, source_note_id, target_note_id, link_type, created_at, valid_from, valid_until)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            link.id,
            link.source_note_id,
            link.target_note_id,
            link.link_type.as_str(),
            link.created_at.to_rfc3339(),
            link.valid_from.map(|dt| dt.to_rfc3339()),
            link.valid_until.map(|dt| dt.to_rfc3339()),
        ],
    )?;
    Ok(link)
}

fn ensure_tag(conn: &Connection, name: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO tags (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![Uuid::new_v4().to_string(), name, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn get_tag_id(conn: &Connection, name: &str) -> AppResult<String> {
    let id: String = conn.query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
        r.get(0)
    })?;
    Ok(id)
}

fn get_note_tags(conn: &Connection, note_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN note_tags nt ON t.id = nt.tag_id WHERE nt.note_id = ?1",
    )?;
    let rows = stmt.query_map(params![note_id], |row| row.get(0))?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

fn count_backlinks(conn: &Connection, note_id: &str) -> AppResult<usize> {
    let count: usize = conn.query_row(
        "SELECT COUNT(*) FROM links WHERE target_note_id = ?1",
        params![note_id],
        |r| r.get(0),
    )?;
    Ok(count)
}

fn count_wikilinks(conn: &Connection, note_id: &str) -> AppResult<usize> {
    let count: usize = conn.query_row(
        "SELECT COUNT(*) FROM links WHERE source_note_id = ?1",
        params![note_id],
        |r| r.get(0),
    )?;
    Ok(count)
}

fn build_note_summary(row: &rusqlite::Row) -> rusqlite::Result<NoteSummary> {
    let content: String = row.get(2)?;
    let preview = if content.len() > 200 {
        format!("{}...", crate::safe_truncate(&content, 200))
    } else {
        content
    };

    Ok(NoteSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        preview,
        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        tag_count: 0,
        backlink_count: 0,
    })
}

fn build_memory_row(row: &rusqlite::Row) -> rusqlite::Result<AgentMemory> {
    Ok(AgentMemory {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        namespace: row.get(2)?,
        key: row.get(3)?,
        value: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        ttl_seconds: row.get(7)?,
    })
}

// ─── Web UI KV / Stats helpers ──────────────────────────────────────

impl Database {
    /// The fixed agent_id used for all web-UI KV entries.
    pub const WEBUI_AGENT: &'static str = "__webui__";

    /// Delete a single memory entry by (agent_id, namespace, key).
    pub fn delete_memory(&self, agent_id: &str, namespace: &str, key: &str) -> AppResult<()> {
        self.execute(|conn| {
            let affected = conn.execute(
                "DELETE FROM agent_memory WHERE agent_id = ?1 AND namespace = ?2 AND key = ?3",
                params![agent_id, namespace, key],
            )?;
            if affected == 0 {
                return Err(AppError::AgentNotFound(format!(
                    "{}/{}/{}",
                    agent_id, namespace, key
                )));
            }
            Ok(())
        })
    }

    /// List web-UI KV entries, optionally filtered by key prefix.
    pub fn list_kv(&self, prefix: Option<&str>) -> AppResult<Vec<AgentMemory>> {
        self.execute(|conn| {
            let entries: Vec<AgentMemory> = if let Some(p) = prefix {
                let pattern = format!("{}%", p);
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds
                     FROM agent_memory WHERE agent_id = ?1 AND key LIKE ?2
                     ORDER BY key ASC",
                )?;
                let rows: Vec<AgentMemory> = stmt
                    .query_map(params![Self::WEBUI_AGENT, pattern], build_memory_row)?
                    .filter_map(|r| r.ok())
                    .filter(|m: &AgentMemory| !m.is_expired())
                    .collect();
                rows
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds
                     FROM agent_memory WHERE agent_id = ?1
                     ORDER BY key ASC",
                )?;
                let rows: Vec<AgentMemory> = stmt
                    .query_map(params![Self::WEBUI_AGENT], build_memory_row)?
                    .filter_map(|r| r.ok())
                    .filter(|m: &AgentMemory| !m.is_expired())
                    .collect();
                rows
            };
            Ok(entries)
        })
    }

    /// Get a single web-UI KV entry by key (namespace = "default").
    pub fn get_kv(&self, key: &str) -> AppResult<AgentMemory> {
        self.get_memory(Self::WEBUI_AGENT, "default", key)
    }

    /// Full graph export for the web UI explorer.
    ///
    /// Returns up to `limit` nodes (most-linked first) plus all edges between them.
    /// When `q` is supplied the result is pre-filtered to notes whose title contains
    /// that substring (case-insensitive).
    pub fn get_full_graph(&self, limit: usize, q: Option<&str>) -> AppResult<FullGraphData> {
        self.execute(|conn| {
            // ── 1. Fetch notes ────────────────────────────────────────────
            // We gather ALL notes so we can compute link_count correctly, then
            // slice to `limit` after sorting by link_count DESC.
            let mut note_stmt = conn.prepare(
                "SELECT n.id, n.title, n.created_at,
                        COUNT(DISTINCT nt.tag_id)           AS tag_count,
                        (SELECT name FROM tags t
                           JOIN note_tags nt2 ON nt2.tag_id = t.id
                          WHERE nt2.note_id = n.id
                          ORDER BY t.name LIMIT 1)          AS primary_tag
                 FROM notes n
                 LEFT JOIN note_tags nt ON nt.note_id = n.id
                 GROUP BY n.id",
            )?;

            struct NoteRow {
                id: String,
                title: String,
                created_at: String,
                tag_count: usize,
                primary_tag: Option<String>,
            }

            let all_notes: Vec<NoteRow> = note_stmt
                .query_map([], |row| {
                    Ok(NoteRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        created_at: row.get(2)?,
                        tag_count: row.get::<_, i64>(3)? as usize,
                        primary_tag: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            // ── 2. Fetch all links ────────────────────────────────────────
            let mut link_stmt = conn.prepare(
                "SELECT source_note_id, target_note_id, link_type, valid_from, valid_until
                 FROM links",
            )?;

            struct LinkRow {
                source: String,
                target: String,
                link_type: String,
                valid_from: Option<String>,
                valid_until: Option<String>,
            }

            let all_links: Vec<LinkRow> = link_stmt
                .query_map([], |row| {
                    Ok(LinkRow {
                        source: row.get(0)?,
                        target: row.get(1)?,
                        link_type: row.get(2)?,
                        valid_from: row.get(3)?,
                        valid_until: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            // ── 3. Compute per-note link counts ───────────────────────────
            let mut link_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for lnk in &all_links {
                *link_counts.entry(lnk.source.clone()).or_insert(0) += 1;
                *link_counts.entry(lnk.target.clone()).or_insert(0) += 1;
            }

            // ── 4. Apply optional title filter ────────────────────────────
            let q_lower = q.map(|s| s.to_lowercase());
            let mut filtered: Vec<&NoteRow> = all_notes
                .iter()
                .filter(|n| {
                    if let Some(ref ql) = q_lower {
                        n.title.to_lowercase().contains(ql.as_str())
                    } else {
                        true
                    }
                })
                .collect();

            // Sort by link_count DESC, then truncate
            filtered.sort_by(|a, b| {
                let lc_a = link_counts.get(&a.id).copied().unwrap_or(0);
                let lc_b = link_counts.get(&b.id).copied().unwrap_or(0);
                lc_b.cmp(&lc_a)
            });
            let total_notes = filtered.len();
            filtered.truncate(limit);

            // Set of IDs in the result so we can filter edges
            let id_set: std::collections::HashSet<&str> =
                filtered.iter().map(|n| n.id.as_str()).collect();

            let nodes: Vec<FullGraphNode> = filtered
                .iter()
                .map(|n| FullGraphNode {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    tag_count: n.tag_count,
                    link_count: link_counts.get(&n.id).copied().unwrap_or(0),
                    created_at: n.created_at.clone(),
                    primary_tag: n.primary_tag.clone(),
                })
                .collect();

            // ── 5. Filter edges to only those within visible nodes ─────────
            let links: Vec<FullGraphEdge> = all_links
                .iter()
                .filter(|l| id_set.contains(l.source.as_str()) && id_set.contains(l.target.as_str()))
                .map(|l| FullGraphEdge {
                    source: l.source.clone(),
                    target: l.target.clone(),
                    rel_type: l.link_type.clone(),
                    valid_from: l.valid_from.clone(),
                    valid_until: l.valid_until.clone(),
                })
                .collect();

            let total_links = links.len();
            Ok(FullGraphData { nodes, links, total_notes, total_links })
        })
    }

    /// Enhanced stats for the web UI: note count, edge count, KV entry count, DB file size.
    pub fn web_stats(&self, db_path: &str) -> AppResult<WebStats> {
        let stats = self.get_stats()?;
        let kv_count: usize = self.execute(|conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM agent_memory WHERE agent_id = ?1",
                params![Self::WEBUI_AGENT],
                |r| r.get(0),
            )?)
        })?;
        let db_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
        Ok(WebStats {
            note_count: stats.total_notes,
            edge_count: stats.total_links,
            kv_count,
            db_size_bytes,
        })
    }

    /// Insert a denormalized LLM audit row. Caller is responsible for having
    /// already inserted the corresponding `events` row (FK enforced).
    pub fn insert_llm_audit_row(
        &self,
        row: &crate::models::llm_audit::LlmAuditRow,
    ) -> crate::errors::AppResult<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::errors::AppError::MutexPoisoned(format!("Failed to lock connection: {}", e))
        })?;
        let note_ids_json = serde_json::to_string(&row.note_ids)?;
        conn.execute(
            "INSERT INTO llm_audit
                (id, event_id, agent_id, tool_name, model, prompt_hash, response_hash,
                 prompt_template_version, note_ids, temperature, seed, prompt_tokens,
                 completion_tokens, duration_ms, outcome, error_message, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            rusqlite::params![
                row.id,
                row.event_id,
                row.agent_id,
                row.tool_name,
                row.model,
                row.prompt_hash,
                row.response_hash,
                row.prompt_template_version,
                note_ids_json,
                row.temperature,
                row.seed,
                row.prompt_tokens,
                row.completion_tokens,
                row.duration_ms as i64,
                row.outcome.as_str(),
                row.error_message,
                row.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    fn test_db() -> Database {
        Database::new(":memory:").expect("in-memory DB")
    }

    // ─── FTS5 query sanitization (Bug #1 regression suite) ────────

    #[test]
    fn sanitize_fts5_wraps_phrase() {
        // Plain phrase: gets wrapped in double quotes.
        assert_eq!(sanitize_fts5_query("hello world"), "\"hello world\"");
    }

    #[test]
    fn sanitize_fts5_neutralises_hyphen_digit_token() {
        // The original bug: `UTF-8 smoke` was parsed as `UTF NOT 8 smoke`,
        // and FTS5 tried to resolve column `8`. After sanitization it is
        // a single literal phrase.
        let out = sanitize_fts5_query("UTF-8 smoke");
        assert_eq!(out, "\"UTF-8 smoke\"");
    }

    #[test]
    fn sanitize_fts5_doubles_embedded_quotes() {
        // Embedded `"` is the only char with special meaning *inside* a
        // phrase. SQLite FTS5 expects it doubled.
        assert_eq!(sanitize_fts5_query("foo\"bar"), "\"foo\"\"bar\"");
    }

    #[test]
    fn sanitize_fts5_handles_empty_query() {
        assert_eq!(sanitize_fts5_query(""), "\"\"");
        assert_eq!(sanitize_fts5_query("   "), "\"\"");
    }

    #[test]
    fn search_notes_does_not_crash_on_hyphen_digit() {
        // End-to-end regression for the `Database error: no such column: 8`
        // crash observed via MCP `notes_search(query="UTF-8 smoke")`.
        let db = test_db();
        db.create_note(CreateNoteRequest {
            title: "UTF-8 smoke test".into(),
            content: "Em-dash, arrow, smoke detector check.".into(),
            tags: vec![],
        })
        .unwrap();

        let q = SearchQuery {
            q: "UTF-8 smoke".into(),
            limit: 10,
            offset: 0,
        };
        let res = db.search_notes(&q).expect("search must not crash");
        assert!(res.iter().any(|n| n.title.contains("UTF-8")));
    }

    #[test]
    fn search_notes_handles_other_hyphen_digit_patterns() {
        // Same bug class for `H1-2026`, `v1-3`, `Q4-2025` etc.
        let db = test_db();
        for title in [
            "Plan H1-2026",
            "Release v1-3",
            "Q4-2025 review",
            "Token claude-4 update",
        ] {
            db.create_note(CreateNoteRequest {
                title: title.into(),
                content: format!("Content for {}", title),
                tags: vec![],
            })
            .unwrap();
        }
        for query in ["H1-2026", "v1-3", "Q4-2025", "claude-4"] {
            let q = SearchQuery {
                q: query.into(),
                limit: 10,
                offset: 0,
            };
            // Should NOT raise `no such column: <digits>`.
            let res = db.search_notes(&q).expect("search must not crash");
            assert!(
                !res.is_empty(),
                "query {:?} should match its seeded note",
                query
            );
        }
    }

    // ─── get_full_graph tests ──────────────────────────────────────

    #[test]
    fn test_full_graph_empty_db() {
        let db = test_db();
        let result = db.get_full_graph(200, None).unwrap();
        assert_eq!(result.nodes.len(), 0);
        assert_eq!(result.links.len(), 0);
        assert_eq!(result.total_notes, 0);
    }

    #[test]
    fn test_full_graph_returns_nodes_and_edges() {
        let db = test_db();
        let a = db.create_note(CreateNoteRequest { title: "Alpha".into(), content: "".into(), tags: vec![] }).unwrap();
        let b = db.create_note(CreateNoteRequest { title: "Beta".into(),  content: "".into(), tags: vec![] }).unwrap();
        db.create_link(&a.id, &b.id, LinkType::Causal).unwrap();

        let result = db.get_full_graph(200, None).unwrap();
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].rel_type, "causal");
    }

    #[test]
    fn test_full_graph_title_filter() {
        let db = test_db();
        db.create_note(CreateNoteRequest { title: "Project Apollo".into(), content: "".into(), tags: vec![] }).unwrap();
        db.create_note(CreateNoteRequest { title: "Meeting notes".into(),  content: "".into(), tags: vec![] }).unwrap();

        let result = db.get_full_graph(200, Some("Apollo")).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].title, "Project Apollo");
    }

    #[test]
    fn test_full_graph_limit() {
        let db = test_db();
        for i in 0..10 {
            db.create_note(CreateNoteRequest {
                title: format!("Note {i}"),
                content: "".into(),
                tags: vec![],
            }).unwrap();
        }
        let result = db.get_full_graph(3, None).unwrap();
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.total_notes, 10); // reports full count
    }

    #[test]
    fn test_full_graph_edges_filtered_to_visible_nodes() {
        let db = test_db();
        let a = db.create_note(CreateNoteRequest { title: "AAA".into(), content: "".into(), tags: vec![] }).unwrap();
        let b = db.create_note(CreateNoteRequest { title: "BBB".into(), content: "".into(), tags: vec![] }).unwrap();
        let c = db.create_note(CreateNoteRequest { title: "CCC".into(), content: "".into(), tags: vec![] }).unwrap();
        db.create_link(&a.id, &b.id, LinkType::Semantic).unwrap();
        db.create_link(&b.id, &c.id, LinkType::Causal).unwrap();

        // Limit to 2 nodes — the edge between the excluded node should not appear
        let result = db.get_full_graph(2, None).unwrap();
        assert!(result.links.len() <= 1, "edges must only connect visible nodes");
    }

    // ─── Link tests ────────────────────────────────────────────────

    #[test]
    fn test_create_link_has_temporal_fields() {
        let db = test_db();
        let note_a = db
            .create_note(CreateNoteRequest {
                title: "Note A".into(),
                content: "Hello".into(),
                tags: vec![],
            })
            .unwrap();
        let note_b = db
            .create_note(CreateNoteRequest {
                title: "Note B".into(),
                content: "World".into(),
                tags: vec![],
            })
            .unwrap();

        let link = db
            .create_link(&note_a.id, &note_b.id, LinkType::WikiLink)
            .unwrap();

        assert!(
            link.valid_from.is_some(),
            "valid_from should be set on new links"
        );
        assert!(
            link.valid_until.is_none(),
            "valid_until should be None (currently valid)"
        );
        assert!(link.is_currently_valid());
    }

    #[test]
    fn test_get_all_links_returns_temporal_fields() {
        let db = test_db();
        let note_a = db
            .create_note(CreateNoteRequest {
                title: "A".into(),
                content: "a".into(),
                tags: vec![],
            })
            .unwrap();
        let note_b = db
            .create_note(CreateNoteRequest {
                title: "B".into(),
                content: "b".into(),
                tags: vec![],
            })
            .unwrap();

        db.create_link(&note_a.id, &note_b.id, LinkType::WikiLink)
            .unwrap();

        let links = db.get_all_links().unwrap();
        assert_eq!(links.len(), 1);
        assert!(links[0].valid_from.is_some());
        assert!(links[0].valid_until.is_none());
    }

    #[test]
    fn test_link_roundtrip_preserves_type() {
        let db = test_db();
        let a = db
            .create_note(CreateNoteRequest {
                title: "X".into(),
                content: "x".into(),
                tags: vec![],
            })
            .unwrap();
        let b = db
            .create_note(CreateNoteRequest {
                title: "Y".into(),
                content: "y".into(),
                tags: vec![],
            })
            .unwrap();

        db.create_link(&a.id, &b.id, LinkType::AiSuggested).unwrap();

        let links = db.get_all_links().unwrap();
        assert_eq!(links[0].link_type, LinkType::AiSuggested);
    }

    // ─── Embedding / semantic search tests ─────────────────────────

    #[test]
    fn test_store_and_search_embedding() {
        let db = test_db();
        let note = db
            .create_note(CreateNoteRequest {
                title: "Embedding Test".into(),
                content: "A note about Rust programming".into(),
                tags: vec![],
            })
            .unwrap();

        // Store a 384-dim embedding (use a simple pattern)
        let mut embedding = vec![0.0_f32; 384];
        embedding[0] = 1.0;
        embedding[1] = 0.5;

        db.store_embedding(&note.id, &embedding, Some("test-model"))
            .unwrap();

        // Search with a similar query vector
        let mut query = vec![0.0_f32; 384];
        query[0] = 0.9;
        query[1] = 0.4;

        let results = db.semantic_search(&query, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, note.id);
        assert_eq!(results[0].title, "Embedding Test");
    }

    #[test]
    fn test_store_embedding_replaces_existing() {
        let db = test_db();
        let note = db
            .create_note(CreateNoteRequest {
                title: "Replace Test".into(),
                content: "content".into(),
                tags: vec![],
            })
            .unwrap();

        let emb1 = vec![1.0_f32; 384];
        db.store_embedding(&note.id, &emb1, None).unwrap();

        // Replace with different embedding
        let emb2 = vec![0.5_f32; 384];
        db.store_embedding(&note.id, &emb2, Some("v2")).unwrap();

        // Should still only find one result
        let query = vec![0.5_f32; 384];
        let results = db.semantic_search(&query, 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn store_embedding_rejects_width_mismatch() {
        let db = test_db();
        let note = db
            .create_note(CreateNoteRequest {
                title: "Width".into(),
                content: "c".into(),
                tags: vec![],
            })
            .unwrap();
        db.store_embedding(&note.id, &vec![0.1_f32; 384], Some("all-minilm"))
            .unwrap();
        let err = db
            .store_embedding(&note.id, &vec![0.1_f32; 8], Some("toy-model"))
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("8-d") && msg.contains("384-d"),
            "expected width mismatch, got {msg}"
        );
    }

    #[test]
    fn test_hybrid_search() {
        let db = test_db();

        // Create notes with different content
        let note_a = db
            .create_note(CreateNoteRequest {
                title: "Rust Programming".into(),
                content: "Rust is a systems programming language focused on safety".into(),
                tags: vec![],
            })
            .unwrap();
        let note_b = db
            .create_note(CreateNoteRequest {
                title: "Python Scripting".into(),
                content: "Python is great for scripting and data science".into(),
                tags: vec![],
            })
            .unwrap();

        // Store embeddings — make note_a closer to query in vector space
        let mut emb_a = vec![0.0_f32; 384];
        emb_a[0] = 1.0;
        let mut emb_b = vec![0.0_f32; 384];
        emb_b[1] = 1.0;

        db.store_embedding(&note_a.id, &emb_a, None).unwrap();
        db.store_embedding(&note_b.id, &emb_b, None).unwrap();

        // Query: text matches "Rust", embedding close to note_a
        let mut query_emb = vec![0.0_f32; 384];
        query_emb[0] = 0.9;

        let results = db.hybrid_search("Rust", &query_emb, 10, 0.5).unwrap();
        assert!(!results.is_empty());
        // note_a should rank first (matches both FTS and semantic)
        assert_eq!(results[0].id, note_a.id);
        assert_eq!(results[0].match_source, "both");
    }

    // ─── Conflict policy tests ─────────────────────────────────────

    #[test]
    fn test_overwrite_policy_default() {
        let db = test_db();
        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v1"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::Overwrite,
            },
        )
        .unwrap();

        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v2"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::Overwrite,
            },
        )
        .unwrap();

        let mem = db.get_memory("agent1", "ns", "k").unwrap();
        assert_eq!(mem.value, serde_json::json!("v2"));

        // No history for overwrite
        let history = db.get_memory_history("agent1", "ns", "k").unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_reject_policy() {
        let db = test_db();
        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v1"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::Reject,
            },
        )
        .unwrap();

        let result = db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v2"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::Reject,
            },
        );

        assert!(result.is_err());
        // Original value preserved
        let mem = db.get_memory("agent1", "ns", "k").unwrap();
        assert_eq!(mem.value, serde_json::json!("v1"));
    }

    #[test]
    fn test_version_and_keep_policy() {
        let db = test_db();
        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v1"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::VersionAndKeep,
            },
        )
        .unwrap();

        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v2"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::VersionAndKeep,
            },
        )
        .unwrap();

        // Current value is v2
        let mem = db.get_memory("agent1", "ns", "k").unwrap();
        assert_eq!(mem.value, serde_json::json!("v2"));

        // v1 archived in history
        let history = db.get_memory_history("agent1", "ns", "k").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].value, serde_json::json!("v1"));
    }

    #[test]
    fn test_invalidate_policy() {
        let db = test_db();
        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v1"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::Invalidate,
            },
        )
        .unwrap();

        db.store_memory(
            "agent1",
            CreateMemoryRequest {
                namespace: Some("ns".into()),
                key: "k".into(),
                value: serde_json::json!("v2"),
                ttl_seconds: None,
                conflict_policy: ConflictPolicy::Invalidate,
            },
        )
        .unwrap();

        // Current value is v2
        let mem = db.get_memory("agent1", "ns", "k").unwrap();
        assert_eq!(mem.value, serde_json::json!("v2"));

        // v1 superseded in history with timestamp
        let history = db.get_memory_history("agent1", "ns", "k").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].value, serde_json::json!("v1"));
        assert!(!history[0].superseded_by.is_empty());
    }

    // ─── Salience-weighted hybrid search (consolidation_score nudge) ──
    //
    // These tests exist to validate the design claims in the doc comment
    // on `hybrid_search_with_salience`, not just that the code compiles:
    //   1. weight = 0.0 is bit-identical to the pre-existing fusion.
    //   2. the boost breaks near-ties toward the more-consolidated note.
    //   3. the boost cannot invert a real relevance gap — a fresh,
    //      strongly-matching note still beats a stale, weakly-matching
    //      but heavily-consolidated one. This is the specific failure
    //      mode ("buries fresh protocol amendments") flagged as the
    //      biggest risk of this feature before it was scoped down to a
    //      bounded multiplicative nudge.

    fn set_consolidation_score(db: &Database, note_id: &str, score: f64) {
        db.execute(|conn| {
            conn.execute(
                "UPDATE notes SET consolidation_score = ?1 WHERE id = ?2",
                params![score, note_id],
            )?;
            Ok(())
        })
        .unwrap();
    }

    /// 384-dim one-hot-ish vector so cosine similarity is controllable:
    /// identical `dim` => identical direction (tied semantic rank);
    /// disjoint `dim` => near-orthogonal (poor semantic rank).
    fn dir384(dim: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; 384];
        v[dim % 384] = 1.0;
        v
    }

    #[test]
    fn salience_weight_zero_is_backward_compatible() {
        let db = test_db();
        let n1 = db
            .create_note(CreateNoteRequest {
                title: "Aspirin dosing protocol".into(),
                content: "Aspirin dosing protocol for cardiac patients.".into(),
                tags: vec![],
            })
            .unwrap();
        let n2 = db
            .create_note(CreateNoteRequest {
                title: "Warfarin dosing protocol".into(),
                content: "Warfarin dosing protocol for anticoagulation.".into(),
                tags: vec![],
            })
            .unwrap();
        db.store_embedding(&n1.id, &dir384(0), Some("test")).unwrap();
        db.store_embedding(&n2.id, &dir384(1), Some("test")).unwrap();

        let plain = db
            .hybrid_search("dosing protocol", &dir384(0), 10, 0.5)
            .unwrap();
        let via_wrapper = db
            .hybrid_search_with_salience("dosing protocol", &dir384(0), 10, 0.5, 0.0)
            .unwrap();

        assert_eq!(plain.len(), via_wrapper.len());
        for (a, b) in plain.iter().zip(via_wrapper.iter()) {
            assert_eq!(a.id, b.id, "weight=0.0 must preserve ordering");
            assert!(
                (a.score - b.score).abs() < 1e-12,
                "weight=0.0 must preserve exact scores: {} vs {}",
                a.score,
                b.score
            );
        }
    }

    #[test]
    fn salience_boost_breaks_ties_toward_consolidated_note() {
        let db = test_db();
        // Identical content and identical embedding direction => FTS and
        // semantic rank should be effectively tied between the two notes.
        let fresh = db
            .create_note(CreateNoteRequest {
                title: "Sepsis screening checklist".into(),
                content: "Sepsis screening checklist for triage nurses.".into(),
                tags: vec![],
            })
            .unwrap();
        let consolidated = db
            .create_note(CreateNoteRequest {
                title: "Sepsis screening checklist".into(),
                content: "Sepsis screening checklist for triage nurses.".into(),
                tags: vec![],
            })
            .unwrap();
        db.store_embedding(&fresh.id, &dir384(5), Some("test")).unwrap();
        db.store_embedding(&consolidated.id, &dir384(5), Some("test"))
            .unwrap();

        set_consolidation_score(&db, &fresh.id, 0.0);
        set_consolidation_score(&db, &consolidated.id, 0.8);

        let results = db
            .hybrid_search_with_salience("sepsis screening checklist", &dir384(5), 10, 0.5, 0.1)
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(
            results[0].id, consolidated.id,
            "with equal relevance, the more-consolidated note should rank first"
        );
    }

    #[test]
    fn salience_boost_does_not_bury_fresh_high_relevance_note() {
        let db = test_db();
        // NEW: exact phrase match + identical embedding direction to the
        // query => strong FTS and semantic rank. Simulates a just-written
        // protocol amendment: consolidation_score = 0.0 (default, never
        // scored yet).
        let new_amendment = db
            .create_note(CreateNoteRequest {
                title: "Protocol amendment 3".into(),
                // Query below is phrase-matched verbatim (see
                // `sanitize_fts5_query`), so the exact phrase must appear
                // contiguously here for the FTS side of the fusion to fire.
                content: "Aspirin washout amendment revises the prior guidance.".into(),
                tags: vec![],
            })
            .unwrap();
        // OLD: shares no keywords with the query and points in a distinct
        // embedding direction (poor semantic rank), but has accumulated a
        // very high consolidation_score from months of replay.
        let old_legacy = db
            .create_note(CreateNoteRequest {
                title: "General site onboarding notes".into(),
                content: "General site onboarding notes, unrelated housekeeping.".into(),
                tags: vec![],
            })
            .unwrap();
        db.store_embedding(&new_amendment.id, &dir384(10), Some("test"))
            .unwrap();
        db.store_embedding(&old_legacy.id, &dir384(200), Some("test"))
            .unwrap();

        set_consolidation_score(&db, &new_amendment.id, 0.0);
        set_consolidation_score(&db, &old_legacy.id, 0.95);

        // Even with a weight 3x the recommended default, a bounded
        // multiplicative nudge must not invert a real relevance gap.
        let results = db
            .hybrid_search_with_salience(
                "aspirin washout amendment",
                &dir384(10),
                10,
                0.5,
                0.3,
            )
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(
            results[0].id, new_amendment.id,
            "a fresh, strongly-matching note must outrank a stale, weakly-matching \
             note regardless of how consolidated the stale note is"
        );
    }

    #[test]
    fn graph_neighbors_walks_requested_depth_without_loading_all_notes() {
        let db = test_db();
        let a = db
            .create_note(CreateNoteRequest {
                title: "A".into(),
                content: "a".into(),
                tags: vec![],
            })
            .unwrap();
        let b = db
            .create_note(CreateNoteRequest {
                title: "B".into(),
                content: "b".into(),
                tags: vec![],
            })
            .unwrap();
        let c = db
            .create_note(CreateNoteRequest {
                title: "C".into(),
                content: "c".into(),
                tags: vec![],
            })
            .unwrap();
        db.create_link(&a.id, &b.id, LinkType::WikiLink).unwrap();
        db.create_link(&b.id, &c.id, LinkType::WikiLink).unwrap();

        let depth1 = db.graph_neighbors(&[&a.id], 1, 20).unwrap();
        assert_eq!(depth1, vec![b.id.clone()]);

        let depth2 = db.graph_neighbors(&[&a.id], 2, 20).unwrap();
        assert!(depth2.contains(&b.id));
        assert!(depth2.contains(&c.id));
        assert!(!depth2.contains(&a.id));
    }

    #[test]
    fn list_note_ids_missing_embeddings_is_single_query() {
        let db = test_db();
        let with_emb = db
            .create_note(CreateNoteRequest {
                title: "Has".into(),
                content: "yes".into(),
                tags: vec![],
            })
            .unwrap();
        let missing = db
            .create_note(CreateNoteRequest {
                title: "Missing".into(),
                content: "no".into(),
                tags: vec![],
            })
            .unwrap();
        db.store_embedding(&with_emb.id, &vec![0.1_f32; 384], Some("test"))
            .unwrap();

        let ids = db.list_note_ids_missing_embeddings().unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], missing.id);
    }
}

#[cfg(test)]
mod llm_audit_op_tests {
    use crate::models::llm_audit::{LlmAuditRow, LlmOutcome};
    use crate::storage::Database;
    use chrono::Utc;

    #[test]
    fn insert_and_count_llm_audit_row() {
        let db = Database::new(":memory:").unwrap();
        // First insert an events row to satisfy the FK constraint
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO events (id, event_type, entity_type, entity_id, payload,
                                 event_time, ingestion_time, event_hash)
             VALUES ('ev1', 'llm_call', 'llm', 'agent1', '{}', ?1, ?1, 'h1')",
            [Utc::now().to_rfc3339()],
        ).unwrap();
        drop(conn);

        let row = LlmAuditRow {
            id: "a1".into(),
            event_id: "ev1".into(),
            agent_id: "agent1".into(),
            tool_name: "notes_summarize".into(),
            model: "mock:v1".into(),
            prompt_hash: "abc".into(),
            response_hash: Some("def".into()),
            prompt_template_version: "summarize@v1".into(),
            note_ids: vec!["n1".into()],
            temperature: 0.0,
            seed: Some(42),
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            duration_ms: 100,
            outcome: LlmOutcome::Success,
            error_message: None,
            created_at: Utc::now(),
        };
        db.insert_llm_audit_row(&row).expect("insert");

        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM llm_audit", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

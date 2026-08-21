use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::ai::document_ingest::{DocumentIngestor, IngestDocumentRequest};
use crate::features::consolidation::{self, AccessKind};
use crate::features::wiki_transaction::SubmitTransactionRequest;
use crate::graph::KnowledgeGraph;
use crate::models::*;
use crate::parser;
use crate::storage::Database;

pub fn handle_notes_create(db: &Database, args: &Value) -> Result<Value, String> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'title'")?
        .to_string();

    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'content'")?
        .to_string();

    let tags: Vec<String> = args
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut all_tags = tags;
    for tag in parser::extract_tags(&content) {
        if !all_tags.contains(&tag) {
            all_tags.push(tag);
        }
    }

    let note = db
        .create_note(CreateNoteRequest {
            title,
            content: content.clone(),
            tags: all_tags,
        })
        .map_err(|e| e.to_string())?;

    // Process wiki-links with inferred type
    let wikilinks = parser::extract_wikilinks(&content);
    for wl in &wikilinks {
        if let Ok(Some(target)) = db.get_note_by_title(&wl.target) {
            let link_type = LinkType::parse(&wl.relation);
            let _ = db.create_link(&note.id, &target.id, link_type);
        }
    }

    Ok(serde_json::to_value(&note).unwrap_or_default())
}

pub fn handle_notes_read(db: &Database, args: &Value) -> Result<Value, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'id'")?;

    // Try by ID first, then by title
    let note = match db.get_note(id) {
        Ok(n) => n,
        Err(_) => match db.get_note_by_title(id).map_err(|e| e.to_string())? {
            Some(n) => db.get_note(&n.id).map_err(|e| e.to_string())?,
            None => return Err(format!("Note not found: {}", id)),
        },
    };

    // Instrument access for consolidation scoring (CLS replay signal)
    let note_id = note.id.clone();
    let _ = db.execute(move |conn| {
        consolidation::log_access(conn, &note_id, AccessKind::McpRetrieve, None, None)
    });

    Ok(serde_json::to_value(&note).unwrap_or_default())
}

pub fn handle_notes_search(db: &Database, args: &Value) -> Result<Value, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'query'")?;

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let results = db
        .search_notes(&SearchQuery {
            q: query.to_string(),
            limit,
            offset: 0,
        })
        .map_err(|e| e.to_string())?;

    // Log search hits for consolidation scoring (CLS replay signal)
    let hit_ids: Vec<String> = results.iter().map(|s| s.id.clone()).collect();
    let q = query.to_string();
    let _ = db.execute(move |conn| {
        for nid in &hit_ids {
            let _ = consolidation::log_access(
                conn, nid, AccessKind::SearchHit, Some(&q), None,
            );
        }
        Ok(())
    });

    Ok(serde_json::to_value(&results).unwrap_or_default())
}

pub fn handle_notes_list(db: &Database, args: &Value) -> Result<Value, String> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let tag = args.get("tag").and_then(|v| v.as_str()).map(String::from);

    let notes = db
        .list_notes(&NoteListQuery {
            limit,
            offset: 0,
            sort: SortOrder::UpdatedDesc,
            tag,
        })
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(&notes).unwrap_or_default())
}

pub fn handle_notes_graph(db: &Database, args: &Value) -> Result<Value, String> {
    let center_id = args.get("center_id").and_then(|v| v.as_str());
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    let link_type_filter = args.get("link_type").and_then(|v| v.as_str());
    let path_to = args.get("path_to").and_then(|v| v.as_str());

    // Log graph traversal access for consolidation scoring (CLS replay signal)
    if let Some(cid) = center_id {
        let cid_owned = cid.to_string();
        let _ = db.execute(move |conn| {
            consolidation::log_access(
                conn, &cid_owned, AccessKind::GraphTraverse, None, None,
            )
        });
    }

    let links = db.get_all_links().map_err(|e| e.to_string())?;
    let notes = db
        .list_notes(&NoteListQuery {
            limit: 10000,
            offset: 0,
            sort: SortOrder::UpdatedDesc,
            tag: None,
        })
        .map_err(|e| e.to_string())?;

    let mut titles: HashMap<String, String> = HashMap::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for note in &notes {
        titles.insert(note.id.clone(), note.title.clone());
        tag_counts.insert(note.id.clone(), note.tag_count);
    }

    let kg = KnowledgeGraph::from_links(&links, &titles, &tag_counts);

    // Shortest path mode: center_id + path_to
    if let (Some(from), Some(to)) = (center_id, path_to) {
        let allowed = link_type_filter.map(|f| {
            LinkType::parse_filter(f)
                .iter()
                .map(|t| t.as_str().to_string())
                .collect::<Vec<_>>()
        });
        let path = kg.shortest_path(from, to, allowed.as_deref());
        return Ok(serde_json::to_value(&path).unwrap_or_default());
    }

    // Subgraph mode (with optional type filter)
    let type_strings = link_type_filter.map(|f| {
        LinkType::parse_filter(f)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect::<Vec<_>>()
    });

    let graph_data = match (center_id, &type_strings) {
        (Some(id), Some(types)) => kg.export_subgraph_filtered(id, depth, types),
        (Some(id), None) => kg.export_subgraph(id, depth),
        _ => kg.export(),
    };

    Ok(serde_json::to_value(&graph_data).unwrap_or_default())
}

pub fn handle_memory_store(db: &Database, args: &Value) -> Result<Value, String> {
    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'agent_id'")?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'key'")?;

    let value = args.get("value").cloned().ok_or("Missing 'value'")?;

    let namespace = args
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(String::from);

    let ttl_seconds = args.get("ttl_seconds").and_then(|v| v.as_i64());

    let conflict_policy = args
        .get("conflict_policy")
        .and_then(|v| v.as_str())
        .map(ConflictPolicy::parse)
        .unwrap_or_default();

    let memory = db
        .store_memory(
            agent_id,
            CreateMemoryRequest {
                namespace,
                key: key.to_string(),
                value,
                ttl_seconds,
                conflict_policy,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(&memory).unwrap_or_default())
}

pub fn handle_memory_retrieve(db: &Database, args: &Value) -> Result<Value, String> {
    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'agent_id'")?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'key'")?;

    let namespace = args
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    let memory = db
        .get_memory(agent_id, namespace, key)
        .map_err(|e| e.to_string())?;

    if memory.is_expired() {
        return Err("Memory entry has expired".to_string());
    }

    Ok(serde_json::to_value(&memory).unwrap_or_default())
}

pub fn handle_memory_list(db: &Database, args: &Value) -> Result<Value, String> {
    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'agent_id'")?;

    let namespace = args.get("namespace").and_then(|v| v.as_str());

    let memories = db
        .list_agent_memory(agent_id, namespace)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(&memories).unwrap_or_default())
}

pub fn handle_memory_history(db: &Database, args: &Value) -> Result<Value, String> {
    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'agent_id'")?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'key'")?;

    let namespace = args
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    let history = db
        .get_memory_history(agent_id, namespace, key)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(&history).unwrap_or_default())
}

pub fn handle_notes_search_semantic(db: &Database, args: &Value) -> Result<Value, String> {
    let embedding: Vec<f32> = args
        .get("embedding")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'embedding' (array of floats)")?
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0) as f32)
        .collect();

    if embedding.is_empty() {
        return Err("Embedding vector cannot be empty".into());
    }

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    // Check if hybrid mode requested (text query provided alongside embedding)
    if let Some(q) = args.get("query").and_then(|v| v.as_str()) {
        if !q.is_empty() {
            let fts_weight = args
                .get("fts_weight")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);

            let results = db
                .hybrid_search(q, &embedding, limit, fts_weight)
                .map_err(|e| e.to_string())?;

            return Ok(serde_json::to_value(&results).unwrap_or_default());
        }
    }

    // Pure semantic search
    let results = db
        .semantic_search(&embedding, limit)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(&results).unwrap_or_default())
}

// ─────────────────────────────────────────────────────────────────────────────
// Integrity layer handlers — wiki_transaction, wiki_verify, contradictions
// Research refs: Zep 2501.13956, FACTUM 2601.05866, Citation-Grounded 2512.12117,
//                MemoTime 2510.13614, A-MEM 2502.12110
// ─────────────────────────────────────────────────────────────────────────────

pub fn handle_wiki_transaction_submit(db: &Database, args: &Value) -> Result<Value, String> {
    let req: SubmitTransactionRequest =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid request: {}", e))?;
    let result = db
        .submit_wiki_transaction(req)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&result).unwrap_or_default())
}

pub fn handle_wiki_transaction_commit(db: &Database, args: &Value) -> Result<Value, String> {
    let id = args
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'transaction_id'")?;
    let result = db
        .commit_wiki_transaction(id)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&result).unwrap_or_default())
}

pub fn handle_wiki_transaction_reject(db: &Database, args: &Value) -> Result<Value, String> {
    let id = args
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'transaction_id'")?;
    let rejected_by = args
        .get("rejected_by")
        .and_then(|v| v.as_str())
        .unwrap_or("human");
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("rejected");
    db.reject_wiki_transaction(id, rejected_by, reason)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "ok": true, "transaction_id": id }))
}

pub fn handle_wiki_transaction_list_pending(
    db: &Database,
    args: &Value,
) -> Result<Value, String> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let txs = db.list_pending_transactions(limit).map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&txs).unwrap_or_default())
}

pub fn handle_wiki_verify(db: &Database, _args: &Value) -> Result<Value, String> {
    let report = db.verify().map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&report).unwrap_or_default())
}

pub fn handle_contradictions_list(db: &Database, args: &Value) -> Result<Value, String> {
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
    let events = db
        .list_open_contradictions(limit)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&events).unwrap_or_default())
}

pub fn handle_notes_consolidate(db: &Database, args: &Value) -> Result<Value, String> {
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let policy = args
        .get("policy")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "standard" => crate::features::consolidation::ConsolidationPolicy::Standard,
            "aggressive" => crate::features::consolidation::ConsolidationPolicy::Aggressive,
            _ => crate::features::consolidation::ConsolidationPolicy::Conservative,
        })
        .unwrap_or_default();

    // BREAKING MCP CONTRACT (introduced v0.3, May 2026): ScoreBreakdown shape changed
    // (cascade_salience replaces access_count + days_since_access as the temporal
    // signal). Consumers that parsed the old access_component / recency_component
    // fields will see those fields absent; salience_component and cascade_salience
    // are the replacements.
    let report = db
        .execute(|conn| {
            crate::features::consolidation::run_consolidation_pass(
                conn,
                policy,
                dry_run,
                crate::features::consolidation::ScoreWeights::default(),
                crate::features::consolidation::Thresholds::default(),
            )
        })
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_value(&report).unwrap_or_default())
}

pub fn handle_contradictions_detect(db: &Database, args: &Value) -> Result<Value, String> {
    let scan_limit = args.get("scan_limit").and_then(|v| v.as_i64()).unwrap_or(50);
    let cfg = crate::features::contradiction::ContradictionConfig::default();
    let events = db
        .detect_contradictions(scan_limit, cfg)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&events).unwrap_or_default())
}

// ─────────────────────────────────────────────────────────────────────────────
// Path A: Local KG — document ingestion + context retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// ingest_document — chunk a text/markdown file into the KG.
///
/// Creates a parent document note + N chunk notes, all linked via ChunkOf.
/// No LLM required — chunking is purely structural.
pub fn handle_ingest_document(db: &Database, args: &Value) -> Result<Value, String> {
    let req: IngestDocumentRequest =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid request: {}", e))?;
    let resp = DocumentIngestor::ingest(db, &req).map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&resp).unwrap_or_default())
}

/// retrieve_context — the single call a local LLM makes to get assembled context.
///
/// Algorithm:
///  1. FTS5 keyword search (always runs)
///  2. If `embedding` provided → hybrid semantic+FTS search instead of FTS-only
///  3. BFS graph expansion from seed notes (depth = `graph_depth`, default 1)
///  4. Pull in parent schema nodes (consolidation_score signal)
///  5. Dedup, rank, truncate to `max_tokens` (≈ chars/4)
///  6. Return assembled context string + structured sources
///
/// The calling LLM owns answer generation — Smriti only assembles the context.
pub fn handle_retrieve_context(db: &Database, args: &Value) -> Result<Value, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'query'")?;

    let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let graph_depth = args.get("graph_depth").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    // max_tokens → approximate char budget (4 chars ≈ 1 token)
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(4096) as usize;
    let max_chars = max_tokens * 4;

    // ── Step 1 / 2: Search ────────────────────────────────────────────────────
    #[derive(Debug)]
    struct ScoredNote {
        id: String,
        title: String,
        content: String,
        score: f64,
        match_type: String,
        consolidation_score: f32,
    }

    let mut seed_notes: Vec<ScoredNote> = Vec::new();

    // Try hybrid (embedding + FTS) if embedding provided
    let embedding: Option<Vec<f32>> = args
        .get("embedding")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect()
        });

    let fts_weight = args
        .get("fts_weight")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    if let Some(ref emb) = embedding {
        if !emb.is_empty() {
            let results = db
                .hybrid_search(query, emb, top_k, fts_weight)
                .map_err(|e| e.to_string())?;
            for r in results {
                if let Ok(note) = db.get_note(&r.id) {
                    seed_notes.push(ScoredNote {
                        score: r.score,
                        match_type: r.match_source.clone(),
                        consolidation_score: note.consolidation_score,
                        id: note.id,
                        title: note.title,
                        content: note.content,
                    });
                }
            }
        }
    }

    // FTS-only fallback (or supplement) when no embedding provided.
    // NoteSummary has no score field — assign descending rank-based scores (1.0, 0.9, …).
    if seed_notes.is_empty() {
        let results = db
            .search_notes(&SearchQuery {
                q: query.to_string(),
                limit: top_k,
                offset: 0,
            })
            .map_err(|e| e.to_string())?;
        let total = results.len().max(1) as f64;
        for (rank, r) in results.into_iter().enumerate() {
            // Rank-based score: 1.0 for rank 0, decreasing by 1/total steps
            let score = 1.0 - (rank as f64 / total);
            if let Ok(note) = db.get_note(&r.id) {
                seed_notes.push(ScoredNote {
                    score,
                    match_type: "fts".to_string(),
                    consolidation_score: note.consolidation_score,
                    id: note.id,
                    title: note.title,
                    content: note.content,
                });
            }
        }
    }

    // ── Step 3: BFS graph expansion ───────────────────────────────────────────
    if graph_depth > 0 && !seed_notes.is_empty() {
        let links = db.get_all_links().map_err(|e| e.to_string())?;
        let notes_list = db
            .list_notes(&NoteListQuery {
                limit: 50_000,
                offset: 0,
                sort: SortOrder::UpdatedDesc,
                tag: None,
            })
            .map_err(|e| e.to_string())?;

        let mut titles: HashMap<String, String> = HashMap::new();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for n in &notes_list {
            titles.insert(n.id.clone(), n.title.clone());
            tag_counts.insert(n.id.clone(), n.tag_count);
        }

        let kg = KnowledgeGraph::from_links(&links, &titles, &tag_counts);
        let seed_ids: Vec<&str> = seed_notes.iter().map(|n| n.id.as_str()).collect();
        let existing_ids: HashSet<String> = seed_notes.iter().map(|n| n.id.clone()).collect();

        let mut neighbor_ids: Vec<String> = Vec::new();
        for seed_id in &seed_ids {
            let subgraph = kg.export_subgraph(seed_id, graph_depth);
            for node in &subgraph.nodes {
                if !existing_ids.contains(&node.id) && !neighbor_ids.contains(&node.id) {
                    neighbor_ids.push(node.id.clone());
                }
            }
        }
        // Limit graph expansion to avoid context explosion
        neighbor_ids.truncate(top_k / 2);

        for nid in &neighbor_ids {
            if let Ok(note) = db.get_note(nid) {
                seed_notes.push(ScoredNote {
                    id: note.id,
                    title: note.title,
                    content: note.content,
                    score: 0.0, // graph-expanded, no search score
                    match_type: "graph".to_string(),
                    consolidation_score: note.consolidation_score,
                });
            }
        }
    }

    // ── Step 4: Pull in schema parents ────────────────────────────────────────
    // If any seed note has a parent_schema_id, include the schema note first
    // (schemas are compressed abstractions — high signal, low tokens).
    {
        let mut schema_ids: HashSet<String> = HashSet::new();
        let existing: HashSet<String> = seed_notes.iter().map(|n| n.id.clone()).collect();
        for note_data in &seed_notes {
            if let Ok(note) = db.get_note(&note_data.id) {
                if let Some(schema_id) = note.parent_schema_id {
                    if !existing.contains(&schema_id) {
                        schema_ids.insert(schema_id);
                    }
                }
            }
        }
        for sid in schema_ids {
            if let Ok(note) = db.get_note(&sid) {
                seed_notes.insert(
                    0, // schemas go first
                    ScoredNote {
                        id: note.id,
                        title: note.title,
                        content: note.content,
                        score: 1.0, // always include schemas
                        match_type: "schema".to_string(),
                        consolidation_score: note.consolidation_score,
                    },
                );
            }
        }
    }

    // ── Step 5: Rank + dedup ──────────────────────────────────────────────────
    // composite_score = 0.5*search_score + 0.3*consolidation_score + 0.2*(1 if schema)
    seed_notes.sort_by(|a, b| {
        let schema_bonus_a = if a.match_type == "schema" { 0.2_f64 } else { 0.0 };
        let schema_bonus_b = if b.match_type == "schema" { 0.2_f64 } else { 0.0 };
        let score_a = 0.5 * a.score + 0.3 * a.consolidation_score as f64 + schema_bonus_a;
        let score_b = 0.5 * b.score + 0.3 * b.consolidation_score as f64 + schema_bonus_b;
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Dedup by note ID (preserve order after sort)
    let mut seen: HashSet<String> = HashSet::new();
    seed_notes.retain(|n| seen.insert(n.id.clone()));

    // ── Step 6: Assemble context ──────────────────────────────────────────────
    let mut context = String::new();
    let mut chars_used = 0usize;
    let mut sources: Vec<serde_json::Value> = Vec::new();

    for (i, note) in seed_notes.iter().enumerate() {
        let block = format!("### [{}] {}\n{}\n\n", i + 1, note.title, note.content);
        if chars_used + block.len() > max_chars {
            // Try a truncated version
            let remaining = max_chars.saturating_sub(chars_used);
            if remaining > 200 {
                let trunc = &block[..remaining.min(block.len())];
                context.push_str(trunc);
                chars_used += trunc.len();
            }
            break;
        }
        context.push_str(&block);
        chars_used += block.len();

        sources.push(serde_json::json!({
            "note_id": note.id,
            "title": note.title,
            "score": note.score,
            "match_type": note.match_type,
            "consolidation_score": note.consolidation_score,
        }));
    }

    // Log search access for consolidation scoring
    let hit_ids: Vec<String> = sources
        .iter()
        .filter_map(|s| s.get("note_id").and_then(|v| v.as_str()).map(String::from))
        .collect();
    let q = query.to_string();
    let _ = db.execute(move |conn| {
        for nid in &hit_ids {
            let _ = consolidation::log_access(conn, nid, AccessKind::SearchHit, Some(&q), None);
        }
        Ok(())
    });

    Ok(serde_json::json!({
        "context": context,
        "sources": sources,
        "token_estimate": chars_used / 4,
        "note_count": sources.len(),
        "query": query,
    }))
}


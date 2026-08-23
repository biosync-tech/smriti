//! Schema formation — embedding clusters → durable abstractions (CLS Phase 3).
//!
//! When ≥ N episodes cluster by cosine similarity, Smriti can:
//!   * Flag the cluster for review (`FlagOnly`, Conservative default)
//!   * Create a `node_type = 'schema'` note with an extractive abstract
//!     and populate `schema_sources` (`Extractive`)
//!   * Same as Extractive, but the abstract is LLM-generated when a backend
//!     is supplied (`Llm`)
//!
//! Episodes are never deleted. `parent_schema_id` is set so retrieval can
//! pull the schema first. ICH E6(R3) trail stays reconstructable.

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::AppResult;
use crate::models::NodeType;

/// How to turn a cluster into a schema (or not).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AbstractionMode {
    /// Record the cluster in the report only. No schema note is written.
    #[default]
    FlagOnly,
    /// Create a schema from titles + excerpts. Works fully offline.
    Extractive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaFormationConfig {
    pub min_cluster_size: usize,
    pub min_similarity: f32,
    pub mode: AbstractionMode,
}

impl Default for SchemaFormationConfig {
    fn default() -> Self {
        Self {
            min_cluster_size: 3,
            min_similarity: 0.82,
            mode: AbstractionMode::FlagOnly,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaCluster {
    pub member_ids: Vec<String>,
    pub mean_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormedSchema {
    pub schema_id: String,
    pub title: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaFormationReport {
    pub dry_run: bool,
    pub mode: AbstractionMode,
    pub clusters_found: usize,
    pub flagged: Vec<SchemaCluster>,
    pub created: Vec<FormedSchema>,
}

/// Greedy single-linkage clustering on cosine similarity.
///
/// Each item starts alone. If two items (or their clusters) have pairwise
/// cosine ≥ `min_similarity`, they merge. Clusters smaller than
/// `min_size` are dropped. Deterministic: items are processed in input order.
pub fn cluster_embeddings(
    items: &[(String, Vec<f32>)],
    min_similarity: f32,
    min_size: usize,
) -> Vec<Vec<String>> {
    if items.len() < min_size {
        return Vec::new();
    }

    let n = items.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut x = i;
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    for i in 0..n {
        for j in (i + 1)..n {
            if cosine(&items[i].1, &items[j].1) >= min_similarity {
                let a = find(&mut parent, i);
                let b = find(&mut parent, j);
                if a != b {
                    parent[b] = a;
                }
            }
        }
    }

    let mut groups: std::collections::BTreeMap<usize, Vec<String>> =
        std::collections::BTreeMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(items[i].0.clone());
    }

    groups
        .into_values()
        .filter(|g| g.len() >= min_size)
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

/// Distinct query_context values for a note, normalised into [0, 1].
/// 8 unique contexts saturates at 1.0. Missing / empty contexts do not count.
pub fn context_diversity(conn: &Connection, note_id: &str) -> AppResult<f32> {
    let distinct: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT query_context)
         FROM note_access_log
         WHERE note_id = ?1
           AND query_context IS NOT NULL
           AND TRIM(query_context) != ''",
        params![note_id],
        |r| r.get(0),
    )?;
    Ok((distinct as f32 / 8.0).clamp(0.0, 1.0))
}

/// Run schema formation over episode notes that already have embeddings.
pub fn form_schemas(
    conn: &Connection,
    cfg: &SchemaFormationConfig,
    dry_run: bool,
) -> AppResult<SchemaFormationReport> {
    let items = load_episode_embeddings(conn)?;
    let clusters = cluster_embeddings(&items, cfg.min_similarity, cfg.min_cluster_size);

    let mut report = SchemaFormationReport {
        dry_run,
        mode: cfg.mode,
        clusters_found: clusters.len(),
        flagged: Vec::new(),
        created: Vec::new(),
    };

    for member_ids in clusters {
        let mean = mean_pairwise_similarity(&items, &member_ids);
        let cluster = SchemaCluster {
            member_ids: member_ids.clone(),
            mean_similarity: mean,
        };

        match cfg.mode {
            AbstractionMode::FlagOnly => {
                report.flagged.push(cluster);
            }
            AbstractionMode::Extractive => {
                if dry_run {
                    report.flagged.push(cluster);
                    continue;
                }
                let formed = create_extractive_schema(conn, &member_ids, mean)?;
                report.created.push(formed);
            }
        }
    }

    Ok(report)
}

fn load_episode_embeddings(conn: &Connection) -> AppResult<Vec<(String, Vec<f32>)>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, v.embedding
         FROM notes n
         JOIN notes_vec v ON v.note_id = n.id
         WHERE n.node_type = 'episode'
           AND n.parent_schema_id IS NULL",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        Ok((id, blob))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, blob) = row?;
        if blob.len() % 4 != 0 {
            continue;
        }
        let mut vec = Vec::with_capacity(blob.len() / 4);
        for chunk in blob.chunks_exact(4) {
            vec.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        items.push((id, vec));
    }
    Ok(items)
}

fn mean_pairwise_similarity(items: &[(String, Vec<f32>)], member_ids: &[String]) -> f32 {
    let vecs: Vec<&Vec<f32>> = member_ids
        .iter()
        .filter_map(|id| items.iter().find(|(i, _)| i == id).map(|(_, v)| v))
        .collect();
    if vecs.len() < 2 {
        return 1.0;
    }
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for i in 0..vecs.len() {
        for j in (i + 1)..vecs.len() {
            sum += cosine(vecs[i], vecs[j]);
            n += 1;
        }
    }
    if n == 0 {
        1.0
    } else {
        sum / n as f32
    }
}

fn create_extractive_schema(
    conn: &Connection,
    member_ids: &[String],
    mean_similarity: f32,
) -> AppResult<FormedSchema> {
    let mut titles = Vec::new();
    let mut excerpts = Vec::new();
    for id in member_ids {
        let (title, content): (String, String) = conn.query_row(
            "SELECT title, content FROM notes WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        titles.push(title.clone());
        let excerpt = crate::safe_truncate(&content, 240);
        excerpts.push(format!("- {}: {}", title, excerpt));
    }

    let schema_title = if titles.len() == 1 {
        format!("Schema: {}", titles[0])
    } else {
        format!("Schema: {} (+{} more)", titles[0], titles.len() - 1)
    };
    let schema_content = format!(
        "Extractive schema over {} episodes (mean cosine {:.2}).\n\n{}",
        member_ids.len(),
        mean_similarity,
        excerpts.join("\n")
    );

    let schema_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO notes (id, title, content, created_at, updated_at, node_type, consolidation_score)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            schema_id,
            schema_title,
            schema_content,
            now,
            now,
            NodeType::Schema.as_str(),
            mean_similarity as f64,
        ],
    )?;

    for id in member_ids {
        conn.execute(
            "INSERT OR IGNORE INTO schema_sources (schema_id, source_note_id, similarity_score, consolidated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![schema_id, id, mean_similarity as f64, now],
        )?;
        conn.execute(
            "UPDATE notes SET parent_schema_id = ?1 WHERE id = ?2",
            params![schema_id, id],
        )?;
        conn.execute(
            "INSERT INTO consolidation_events
               (id, note_id, event_type, score_before, score_after, reason, created_at)
             VALUES (?1, ?2, 'promoted_to_schema', NULL, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                id,
                mean_similarity as f64,
                format!("subsumed by schema {schema_id} (extractive, mean cosine {mean_similarity:.3})"),
                now,
            ],
        )?;
    }

    Ok(FormedSchema {
        schema_id,
        title: schema_title,
        source_ids: member_ids.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::note::CreateNoteRequest;
    use crate::storage::Database;

    fn unit(i: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; 8];
        v[i % 8] = 1.0;
        v
    }

    #[test]
    fn cluster_embeddings_merges_near_duplicates() {
        let items = vec![
            ("a".into(), unit(0)),
            ("b".into(), unit(0)),
            ("c".into(), unit(0)),
            ("d".into(), unit(3)),
        ];
        let clusters = cluster_embeddings(&items, 0.99, 3);
        assert_eq!(clusters.len(), 1);
        let mut ids = clusters[0].clone();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn cluster_embeddings_drops_small_groups() {
        let items = vec![("a".into(), unit(0)), ("b".into(), unit(0))];
        assert!(cluster_embeddings(&items, 0.99, 3).is_empty());
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn context_diversity_counts_distinct_queries() {
        let db = Database::new(":memory:").unwrap();
        let note = db
            .create_note(CreateNoteRequest {
                title: "N".into(),
                content: "c".into(),
                tags: vec![],
            })
            .unwrap();
        db.execute(|conn| {
            crate::features::consolidation::log_access(
                conn,
                &note.id,
                crate::features::consolidation::AccessKind::SearchHit,
                Some("protocol v2.1"),
                None,
            )?;
            crate::features::consolidation::log_access(
                conn,
                &note.id,
                crate::features::consolidation::AccessKind::SearchHit,
                Some("inclusion criterion"),
                None,
            )?;
            crate::features::consolidation::log_access(
                conn,
                &note.id,
                crate::features::consolidation::AccessKind::Read,
                Some("protocol v2.1"),
                None,
            )?;
            let d = context_diversity(conn, &note.id)?;
            assert!((d - 2.0 / 8.0).abs() < 1e-6);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn extractive_schema_writes_lineage_and_does_not_delete_episodes() {
        let db = Database::new(":memory:").unwrap();
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

        let report = db
            .execute(|conn| {
                form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Extractive,
                    },
                    false,
                )
            })
            .unwrap();

        assert_eq!(report.created.len(), 1);
        let schema_id = report.created[0].schema_id.clone();

        for id in &ids {
            let note = db.get_note(id).unwrap();
            assert_eq!(note.node_type, NodeType::Episode);
            assert_eq!(note.parent_schema_id.as_deref(), Some(schema_id.as_str()));
        }
        let schema = db.get_note(&schema_id).unwrap();
        assert_eq!(schema.node_type, NodeType::Schema);
    }
}

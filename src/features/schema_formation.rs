//! Schema formation — WikiSkill-mapped CLS Phase 3.
//!
//! WikiSkill (arXiv:2608.27454) showed that a persistent knowledge layer
//! between raw traces and executable procedures is the largest driver of
//! improvement. Smriti already had the tables; this module is the missing
//! maintainer: cluster episodes → propose one atomic schema (or a patch to
//! one sibling) → gate → commit.
//!
//! Isolation (WikiSkill ablation 63.7% → 60.9% when inference sees the wiki
//! during training): a proposal is NEVER a `notes` row until it is accepted.
//! Live retrieval (`retrieve_context`, `notes_search_semantic`) cannot see
//! half-formed abstracts. Pending state lives only in `consolidation_events`.
//!
//! Gating is honest about the missing WikiSkill signal: Smriti has no
//! `y_i` / validation-set accuracy. Conservative never auto-promotes.
//! Standard/Aggressive may use a retrieve-context proxy built from
//! `note_access_log.query_context`. That proxy is not task-accuracy gating.
//!
//! Episodes are never deleted. ICH E6(R3) trail stays reconstructable.

use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::NodeType;
use crate::storage::operations::sanitize_fts5_query;

/// Prefix stored in `consolidation_events.reason` for a pending proposal.
/// Follow-up accept/reject events stay append-only (never UPDATE this row).
pub const PROPOSAL_PREFIX: &str = "SCHEMA_PROPOSAL ";

/// How to turn a cluster into an abstract (or not).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AbstractionMode {
    /// Record the cluster in the report only. No schema note is written.
    #[default]
    FlagOnly,
    /// Create a schema from titles + excerpts. Works fully offline.
    Extractive,
    /// Call a user-supplied [`SchemaAbstractor`] (Ollama / mock). If none is
    /// provided, the cluster is flagged — we do not silently fall back to
    /// extractive and pretend an LLM wrote it.
    Llm,
}

/// When a formed proposal may become a live schema note.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoCommit {
    /// Persist as `flagged_for_review`. Human accepts later. Default / Conservative.
    #[default]
    Never,
    /// Commit only if the retrieve-context proxy improves held-out queries.
    Proxy,
    /// Commit extractive schemas immediately. Test / debug only.
    Immediate,
}

/// What "acceptance" recorded in the audit trail. Do not blur these.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatingSignal {
    HumanApproved {
        by: String,
    },
    ProxyRetrieve {
        detail: String,
    },
    /// Test/debug path. Reason text must say this is not a production gate.
    ImmediateExtractive,
}

impl GatingSignal {
    pub fn reason_prefix(&self) -> String {
        match self {
            GatingSignal::HumanApproved { by } => {
                format!("gating=human_approved by={by}")
            }
            GatingSignal::ProxyRetrieve { detail } => {
                format!("gating=proxy_retrieve_accepted {detail}")
            }
            GatingSignal::ImmediateExtractive => {
                "gating=immediate_extractive (not a production gate; test/debug only)".into()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    Create,
    Patch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaFormationConfig {
    pub min_cluster_size: usize,
    pub min_similarity: f32,
    pub mode: AbstractionMode,
    pub auto_commit: AutoCommit,
    pub proxy: ProxyGateConfig,
}

impl Default for SchemaFormationConfig {
    fn default() -> Self {
        Self {
            min_cluster_size: 3,
            min_similarity: 0.82,
            mode: AbstractionMode::FlagOnly,
            auto_commit: AutoCommit::Never,
            proxy: ProxyGateConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProxyGateConfig {
    /// Minimum mean groundedness lift required to auto-accept.
    pub min_delta: f32,
    /// Max distinct `query_context` values sampled per cluster.
    pub sample_limit: usize,
    /// FTS top-k for the without-schema retrieve. Small on purpose: a schema
    /// that covers the cluster should improve coverage vs a short hit list.
    pub fts_top_k: usize,
}

impl Default for ProxyGateConfig {
    fn default() -> Self {
        Self {
            min_delta: 0.05,
            sample_limit: 8,
            fts_top_k: 3,
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
pub struct SchemaProposal {
    pub id: String,
    pub title: String,
    pub content: String,
    pub source_ids: Vec<String>,
    pub mean_similarity: f32,
    pub rationale: String,
    pub action: ProposalAction,
    pub sibling_schema_id: Option<String>,
    pub abstractor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaFormationReport {
    pub dry_run: bool,
    pub mode: AbstractionMode,
    pub auto_commit: AutoCommit,
    pub clusters_found: usize,
    pub flagged: Vec<SchemaCluster>,
    pub created: Vec<FormedSchema>,
    pub pending: Vec<SchemaProposal>,
    pub proxy_rejected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeExcerpt {
    pub id: String,
    pub title: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingSchema {
    pub id: String,
    pub title: String,
    pub content_excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbstractionInput {
    pub episodes: Vec<EpisodeExcerpt>,
    pub siblings: Vec<SiblingSchema>,
    pub mean_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbstractionOutput {
    pub title: String,
    pub content: String,
    pub rationale: String,
    pub action: ProposalAction,
    pub sibling_schema_id: Option<String>,
}

/// Sync abstraction boundary. Tests inject a mock; production CLI may wrap
/// a completed Ollama call. The trait itself stays sync so MCP / `db.execute`
/// do not block the request runtime on a network round-trip.
pub trait SchemaAbstractor: Send + Sync {
    fn abstract_cluster(&self, input: &AbstractionInput) -> AppResult<AbstractionOutput>;
}

pub struct ExtractiveAbstractor;

impl SchemaAbstractor for ExtractiveAbstractor {
    fn abstract_cluster(&self, input: &AbstractionInput) -> AppResult<AbstractionOutput> {
        Ok(extractive_output(input))
    }
}

/// Wrap a `SharedBackend` for sync `db.execute` without a second Tokio runtime.
pub struct BackendAbstractor {
    pub backend: crate::inference::SharedBackend,
}

impl SchemaAbstractor for BackendAbstractor {
    fn abstract_cluster(&self, input: &AbstractionInput) -> AppResult<AbstractionOutput> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                let backend = self.backend.clone();
                let input = input.clone();
                std::thread::spawn(move || handle.block_on(llm_abstract(backend.as_ref(), &input)))
                    .join()
                    .map_err(|_| {
                        AppError::BadRequest("schema LLM abstractor thread panicked".into())
                    })?
            }
            Err(_) => Err(AppError::BadRequest(
                "LLM abstraction requires a Tokio runtime; cluster will be flagged".into(),
            )),
        }
    }
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
    for (i, item) in items.iter().enumerate() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(item.0.clone());
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

/// Token-overlap groundedness: fraction of query tokens present in context.
/// Offline, no embeddings required. Not WikiSkill task-accuracy.
pub fn groundedness(query: &str, context: &str) -> f32 {
    let tokens = significant_tokens(query);
    if tokens.is_empty() {
        return 0.0;
    }
    let ctx = context.to_lowercase();
    let hits = tokens.iter().filter(|t| ctx.contains(t.as_str())).count();
    hits as f32 / tokens.len() as f32
}

/// Combined proxy score: groundedness + cluster-coverage of retrieved ids.
pub fn proxy_score(
    query: &str,
    context: &str,
    retrieved_ids: &[String],
    source_ids: &[String],
) -> f32 {
    let g = groundedness(query, context);
    let coverage = if source_ids.is_empty() {
        0.0
    } else {
        let hit = source_ids
            .iter()
            .filter(|id| retrieved_ids.iter().any(|r| r == *id))
            .count();
        hit as f32 / source_ids.len() as f32
    };
    0.5 * g + 0.5 * coverage
}

/// Run schema formation over episode notes that already have embeddings.
///
/// `eligible_ids` is the promotion allowlist from the scoring pass:
///   * `None` — cluster every unparented episode that has an embedding
///     (standalone / tests).
///   * `Some(ids)` — only those notes. An empty slice yields no clusters.
///
/// Half-formed proposals are not inserted into `notes`. Conservative /
/// `AutoCommit::Never` writes `flagged_for_review` events only.
pub fn form_schemas(
    conn: &Connection,
    cfg: &SchemaFormationConfig,
    dry_run: bool,
    eligible_ids: Option<&[String]>,
    abstractor: Option<&dyn SchemaAbstractor>,
) -> AppResult<SchemaFormationReport> {
    let proposals = propose_schemas(conn, cfg, eligible_ids, abstractor)?;

    let mut report = SchemaFormationReport {
        dry_run,
        mode: cfg.mode,
        auto_commit: cfg.auto_commit,
        clusters_found: proposals.len(),
        flagged: Vec::new(),
        created: Vec::new(),
        pending: Vec::new(),
        proxy_rejected: Vec::new(),
    };

    for proposal in proposals {
        let cluster = SchemaCluster {
            member_ids: proposal.source_ids.clone(),
            mean_similarity: proposal.mean_similarity,
        };

        if dry_run {
            report.flagged.push(cluster);
            report.pending.push(proposal);
            continue;
        }

        // LLM miss: never commit extractive text as if a model wrote it.
        if proposal.rationale.starts_with("llm_failed:")
            || proposal.rationale.starts_with("llm_unavailable:")
        {
            persist_proposal_flagged(conn, &proposal)?;
            report.flagged.push(cluster);
            report.pending.push(proposal);
            continue;
        }

        match cfg.auto_commit {
            AutoCommit::Never => {
                persist_proposal_flagged(conn, &proposal)?;
                report.flagged.push(cluster);
                report.pending.push(proposal);
            }
            AutoCommit::Immediate => {
                let formed = commit_proposal(conn, &proposal, &GatingSignal::ImmediateExtractive)?;
                report.created.push(formed);
            }
            AutoCommit::Proxy => match evaluate_proxy(conn, &proposal, &cfg.proxy)? {
                ProxyDecision::Accept { detail } => {
                    let formed =
                        commit_proposal(conn, &proposal, &GatingSignal::ProxyRetrieve { detail })?;
                    report.created.push(formed);
                }
                ProxyDecision::Reject { detail } => {
                    persist_proposal_flagged(conn, &proposal)?;
                    write_event(
                        conn,
                        proposal
                            .source_ids
                            .first()
                            .map(String::as_str)
                            .unwrap_or("unknown"),
                        "schema_proposal_rejected",
                        None,
                        proposal.mean_similarity,
                        &format!(
                            "proposal_id={} gating=proxy_retrieve_rejected {detail}",
                            proposal.id
                        ),
                    )?;
                    report.flagged.push(cluster);
                    report.pending.push(proposal.clone());
                    report.proxy_rejected.push(proposal.id);
                }
                ProxyDecision::Unavailable { detail } => {
                    persist_proposal_flagged(conn, &proposal)?;
                    report.flagged.push(cluster);
                    report.pending.push(proposal);
                    tracing::info!("schema proxy unavailable: {detail}");
                }
            },
        }
    }

    Ok(report)
}

/// Cluster + abstract. Writes nothing.
pub fn propose_schemas(
    conn: &Connection,
    cfg: &SchemaFormationConfig,
    eligible_ids: Option<&[String]>,
    abstractor: Option<&dyn SchemaAbstractor>,
) -> AppResult<Vec<SchemaProposal>> {
    let mut items = load_episode_embeddings(conn)?;
    if let Some(allow) = eligible_ids {
        let allow: HashSet<&str> = allow.iter().map(String::as_str).collect();
        items.retain(|(id, _)| allow.contains(id.as_str()));
    }
    let clusters = cluster_embeddings(&items, cfg.min_similarity, cfg.min_cluster_size);
    let mut proposals = Vec::new();

    for member_ids in clusters {
        let mean = mean_pairwise_similarity(&items, &member_ids);
        let input = build_abstraction_input(conn, &member_ids, mean, &items, cfg.min_similarity)?;

        let output = match cfg.mode {
            AbstractionMode::FlagOnly => extractive_output(&input),
            AbstractionMode::Extractive => match abstractor {
                Some(a) => a.abstract_cluster(&input)?,
                None => extractive_output(&input),
            },
            AbstractionMode::Llm => match abstractor {
                Some(a) => match a.abstract_cluster(&input) {
                    Ok(out) => out,
                    Err(e) => {
                        let mut flagged = extractive_output(&input);
                        flagged.rationale =
                            format!("llm_failed: {e}; cluster flagged, no abstraction committed");
                        flagged
                    }
                },
                None => {
                    let mut flagged = extractive_output(&input);
                    flagged.rationale =
                        "llm_unavailable: cluster flagged, no abstraction committed".into();
                    flagged
                }
            },
        };

        let abstractor_name = match cfg.mode {
            AbstractionMode::Llm if abstractor.is_some() => "llm",
            AbstractionMode::Llm => "llm_unavailable",
            _ => "extractive",
        };

        proposals.push(SchemaProposal {
            id: Uuid::new_v4().to_string(),
            title: output.title,
            content: output.content,
            source_ids: member_ids,
            mean_similarity: mean,
            rationale: output.rationale,
            action: output.action,
            sibling_schema_id: output.sibling_schema_id,
            abstractor: abstractor_name.into(),
        });
    }

    Ok(proposals)
}

pub fn persist_proposal_flagged(conn: &Connection, proposal: &SchemaProposal) -> AppResult<()> {
    let note_id = proposal
        .source_ids
        .first()
        .cloned()
        .ok_or_else(|| AppError::BadRequest("schema proposal has no source episodes".into()))?;
    let payload = serde_json::to_string(proposal)?;
    write_event(
        conn,
        &note_id,
        "flagged_for_review",
        None,
        proposal.mean_similarity,
        &format!("{PROPOSAL_PREFIX}{payload}"),
    )?;
    Ok(())
}

/// Commit one proposal atomically: one schema note (create or patch), lineage,
/// parent pointers, wiki-links, audit row. Never deletes an episode.
pub fn commit_proposal(
    conn: &Connection,
    proposal: &SchemaProposal,
    signal: &GatingSignal,
) -> AppResult<FormedSchema> {
    let now = Utc::now().to_rfc3339();
    let schema_id = match (proposal.action, proposal.sibling_schema_id.as_deref()) {
        (ProposalAction::Patch, Some(existing)) => {
            let exists: Option<String> = conn
                .query_row(
                    "SELECT id FROM notes WHERE id = ?1 AND node_type = 'schema'",
                    params![existing],
                    |r| r.get(0),
                )
                .optional()?;
            match exists {
                Some(id) => {
                    conn.execute(
                        "UPDATE notes SET title = ?1, content = ?2, updated_at = ?3
                         WHERE id = ?4",
                        params![proposal.title, proposal.content, now, id],
                    )?;
                    id
                }
                None => insert_schema_note(conn, proposal, &now)?,
            }
        }
        _ => insert_schema_note(conn, proposal, &now)?,
    };

    for id in &proposal.source_ids {
        conn.execute(
            "INSERT OR IGNORE INTO schema_sources (schema_id, source_note_id, similarity_score, consolidated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![schema_id, id, proposal.mean_similarity as f64, now],
        )?;
        conn.execute(
            "UPDATE notes SET parent_schema_id = ?1 WHERE id = ?2",
            params![schema_id, id],
        )?;
        let _ = crate::storage::operations::insert_link_on_conn(
            conn,
            &schema_id,
            id,
            crate::models::LinkType::WikiLink,
        );
        write_event(
            conn,
            id,
            "promoted_to_schema",
            None,
            proposal.mean_similarity,
            &format!(
                "proposal_id={} schema_id={} {} abstractor={} rationale={}",
                proposal.id,
                schema_id,
                signal.reason_prefix(),
                proposal.abstractor,
                proposal.rationale
            ),
        )?;
    }

    write_event(
        conn,
        &schema_id,
        "promoted_to_schema",
        None,
        proposal.mean_similarity,
        &format!(
            "proposal_id={} {} abstractor={} sources={} rationale={}",
            proposal.id,
            signal.reason_prefix(),
            proposal.abstractor,
            proposal.source_ids.len(),
            proposal.rationale
        ),
    )?;

    Ok(FormedSchema {
        schema_id,
        title: proposal.title.clone(),
        source_ids: proposal.source_ids.clone(),
    })
}

/// Reject a pending proposal. Writes an audit row. Touches no schema notes.
pub fn reject_proposal(
    conn: &Connection,
    proposal: &SchemaProposal,
    by: &str,
    reason: &str,
) -> AppResult<()> {
    let note_id = proposal
        .source_ids
        .first()
        .cloned()
        .ok_or_else(|| AppError::BadRequest("schema proposal has no source episodes".into()))?;
    write_event(
        conn,
        &note_id,
        "schema_proposal_rejected",
        None,
        proposal.mean_similarity,
        &format!(
            "proposal_id={} gating=human_rejected by={by} reason={reason}",
            proposal.id
        ),
    )?;
    Ok(())
}

pub fn list_pending_proposals(conn: &Connection) -> AppResult<Vec<SchemaProposal>> {
    let resolved = resolved_proposal_ids(conn)?;
    let mut stmt = conn.prepare(
        "SELECT reason FROM consolidation_events
         WHERE event_type = 'flagged_for_review'
           AND reason LIKE 'SCHEMA_PROPOSAL %'
         ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        let reason = row?;
        if let Some(p) = parse_proposal_reason(&reason) {
            if !resolved.contains(&p.id) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

pub fn load_proposal(conn: &Connection, proposal_id: &str) -> AppResult<SchemaProposal> {
    resolve_pending_proposal(conn, proposal_id)
}

/// Accept a proposal id **or** a source episode id from `smriti approve`.
pub fn resolve_pending_proposal(conn: &Connection, id: &str) -> AppResult<SchemaProposal> {
    let pending = list_pending_proposals(conn)?;
    if let Some(p) = pending.iter().find(|p| p.id == id) {
        return Ok(p.clone());
    }
    if let Some(p) = pending
        .iter()
        .find(|p| p.source_ids.iter().any(|s| s == id))
    {
        return Ok(p.clone());
    }
    Err(AppError::BadRequest(format!(
        "pending schema proposal not found: {id} (use `smriti proposals`)"
    )))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageSource {
    pub note_id: String,
    pub title: String,
    pub similarity_score: Option<f32>,
    pub consolidated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageEvent {
    pub event_type: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaExplain {
    pub note_id: String,
    pub node_type: String,
    pub title: String,
    pub parent_schema_id: Option<String>,
    pub sources: Vec<LineageSource>,
    pub events: Vec<LineageEvent>,
}

/// WikiSkill-style provenance chain for `smriti consolidate --explain`.
pub fn explain_schema(conn: &Connection, note_id: &str) -> AppResult<SchemaExplain> {
    let (title, node_type, parent): (String, String, Option<String>) = conn.query_row(
        "SELECT title, node_type, parent_schema_id FROM notes WHERE id = ?1",
        params![note_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let schema_id = if node_type == "schema" {
        note_id.to_string()
    } else {
        parent.clone().unwrap_or_else(|| note_id.to_string())
    };

    let mut src_stmt = conn.prepare(
        "SELECT s.source_note_id, n.title, s.similarity_score, s.consolidated_at
         FROM schema_sources s
         JOIN notes n ON n.id = s.source_note_id
         WHERE s.schema_id = ?1
         ORDER BY s.consolidated_at ASC",
    )?;
    let src_rows = src_stmt.query_map(params![schema_id], |r| {
        Ok(LineageSource {
            note_id: r.get(0)?,
            title: r.get(1)?,
            similarity_score: r.get::<_, Option<f64>>(2)?.map(|v| v as f32),
            consolidated_at: r.get(3)?,
        })
    })?;
    let mut sources = Vec::new();
    for row in src_rows {
        sources.push(row?);
    }

    let mut ev_stmt = conn.prepare(
        "SELECT event_type, reason, created_at FROM consolidation_events
         WHERE note_id = ?1 OR note_id IN (
            SELECT source_note_id FROM schema_sources WHERE schema_id = ?1
         ) OR (reason LIKE '%' || ?1 || '%')
         ORDER BY created_at ASC",
    )?;
    let ev_rows = ev_stmt.query_map(params![schema_id], |r| {
        Ok(LineageEvent {
            event_type: r.get(0)?,
            reason: r.get(1)?,
            created_at: r.get(2)?,
        })
    })?;
    let mut events = Vec::new();
    for row in ev_rows {
        events.push(row?);
    }

    Ok(SchemaExplain {
        note_id: note_id.to_string(),
        node_type,
        title,
        parent_schema_id: parent,
        sources,
        events,
    })
}

pub async fn llm_abstract(
    backend: &dyn crate::inference::InferenceBackend,
    input: &AbstractionInput,
) -> AppResult<AbstractionOutput> {
    use crate::inference::GenerateRequest;

    let mut episode_block = String::new();
    for e in &input.episodes {
        episode_block.push_str(&format!("- [{}] {}: {}\n", e.id, e.title, e.excerpt));
    }
    let mut sibling_block = String::new();
    for s in &input.siblings {
        sibling_block.push_str(&format!(
            "- [{}] {}: {}\n",
            s.id, s.title, s.content_excerpt
        ));
    }

    let prompt = format!(
        "You maintain a persistent knowledge wiki. Propose ONE atomic schema update \
         (create a new schema page OR patch exactly one existing sibling). \
         Do not rewrite multiple pages.\n\n\
         Episode cluster (mean cosine {:.3}):\n{episode_block}\n\
         Existing sibling schemas:\n{}\n\n\
         Reply as JSON only: {{\"title\":\"...\",\"content\":\"...\",\"rationale\":\"...\",\
         \"action\":\"create\"|\"patch\",\"sibling_schema_id\":null|\"id\"}}",
        input.mean_similarity,
        if sibling_block.is_empty() {
            "(none)".into()
        } else {
            sibling_block
        }
    );

    let resp = backend
        .generate(&GenerateRequest {
            prompt,
            system: Some(
                "You are Smriti's wiki maintainer. One schema per proposal. \
                 State a human-readable rationale. Never delete source episodes."
                    .into(),
            ),
            max_tokens: Some(1024),
            temperature: Some(0.2),
            ..Default::default()
        })
        .await
        .map_err(|e| AppError::BadRequest(format!("schema LLM abstraction failed: {e}")))?;

    parse_llm_abstraction(&resp.text, input)
}

fn parse_llm_abstraction(text: &str, input: &AbstractionInput) -> AppResult<AbstractionOutput> {
    let json_slice = extract_json_object(text).unwrap_or(text);
    let v: serde_json::Value = serde_json::from_str(json_slice).map_err(|_| {
        AppError::ParseError("LLM abstraction was not valid JSON; refusing to commit".into())
    })?;
    let title = v
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("Schema")
        .to_string();
    let content = v
        .get("content")
        .and_then(|x| x.as_str())
        .ok_or_else(|| AppError::ParseError("LLM abstraction missing content".into()))?
        .to_string();
    let rationale = v
        .get("rationale")
        .and_then(|x| x.as_str())
        .unwrap_or("llm rationale omitted")
        .to_string();
    let action = match v.get("action").and_then(|x| x.as_str()) {
        Some("patch") => ProposalAction::Patch,
        _ => ProposalAction::Create,
    };
    let sibling = v
        .get("sibling_schema_id")
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty() && s != "null");
    let sibling_schema_id = match action {
        ProposalAction::Patch => sibling.or_else(|| input.siblings.first().map(|s| s.id.clone())),
        ProposalAction::Create => None,
    };
    Ok(AbstractionOutput {
        title,
        content,
        rationale,
        action,
        sibling_schema_id,
    })
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

fn extractive_output(input: &AbstractionInput) -> AbstractionOutput {
    let titles: Vec<&str> = input.episodes.iter().map(|e| e.title.as_str()).collect();
    let schema_title = if let Some(sib) = input.siblings.first() {
        sib.title.clone()
    } else if titles.len() == 1 {
        format!("Schema: {}", titles[0])
    } else {
        format!("Schema: {} (+{} more)", titles[0], titles.len() - 1)
    };
    let excerpts: Vec<String> = input
        .episodes
        .iter()
        .map(|e| format!("- {}: {}", e.title, e.excerpt))
        .collect();
    let schema_content = format!(
        "Extractive schema over {} episodes (mean cosine {:.2}).\n\n{}",
        input.episodes.len(),
        input.mean_similarity,
        excerpts.join("\n")
    );
    let (action, sibling_schema_id) = if let Some(sib) = input.siblings.first() {
        (ProposalAction::Patch, Some(sib.id.clone()))
    } else {
        (ProposalAction::Create, None)
    };
    AbstractionOutput {
        title: schema_title,
        content: schema_content,
        rationale: format!(
            "extractive abstract over {} episodes (mean cosine {:.3})",
            input.episodes.len(),
            input.mean_similarity
        ),
        action,
        sibling_schema_id,
    }
}

fn build_abstraction_input(
    conn: &Connection,
    member_ids: &[String],
    mean: f32,
    items: &[(String, Vec<f32>)],
    min_similarity: f32,
) -> AppResult<AbstractionInput> {
    let mut episodes = Vec::new();
    for id in member_ids {
        let (title, content): (String, String) = conn.query_row(
            "SELECT title, content FROM notes WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        episodes.push(EpisodeExcerpt {
            id: id.clone(),
            title,
            excerpt: crate::safe_truncate(&content, 240).to_string(),
        });
    }

    let centroid = cluster_centroid(items, member_ids);
    let siblings = if let Some(c) = centroid {
        nearest_sibling_schemas(conn, &c, min_similarity)?
    } else {
        Vec::new()
    };

    Ok(AbstractionInput {
        episodes,
        siblings,
        mean_similarity: mean,
    })
}

fn cluster_centroid(items: &[(String, Vec<f32>)], member_ids: &[String]) -> Option<Vec<f32>> {
    let vecs: Vec<&Vec<f32>> = member_ids
        .iter()
        .filter_map(|id| items.iter().find(|(i, _)| i == id).map(|(_, v)| v))
        .collect();
    let dim = vecs.first()?.len();
    if dim == 0 || vecs.iter().any(|v| v.len() != dim) {
        return None;
    }
    let mut acc = vec![0.0f32; dim];
    for v in &vecs {
        for (i, x) in v.iter().enumerate() {
            acc[i] += *x;
        }
    }
    let n = vecs.len() as f32;
    for x in &mut acc {
        *x /= n;
    }
    Some(acc)
}

/// At most one sibling — WikiSkill: one schema touched per proposal.
fn nearest_sibling_schemas(
    conn: &Connection,
    centroid: &[f32],
    min_similarity: f32,
) -> AppResult<Vec<SiblingSchema>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, n.title, n.content, v.embedding
         FROM notes n
         JOIN notes_vec v ON v.note_id = n.id
         WHERE n.node_type = 'schema'",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
        ))
    })?;

    let mut best: Option<(f32, SiblingSchema)> = None;
    for row in rows {
        let (id, title, content, blob) = row?;
        let vec = blob_to_f32(&blob);
        if vec.is_empty() {
            continue;
        }
        let sim = cosine(centroid, &vec);
        if sim >= min_similarity {
            let sib = SiblingSchema {
                id,
                title,
                content_excerpt: crate::safe_truncate(&content, 320).to_string(),
            };
            if best.as_ref().map(|(s, _)| sim > *s).unwrap_or(true) {
                best = Some((sim, sib));
            }
        }
    }
    Ok(best.into_iter().map(|(_, s)| s).collect())
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
        let vec = blob_to_f32(&blob);
        if !vec.is_empty() {
            items.push((id, vec));
        }
    }
    Ok(items)
}

fn blob_to_f32(blob: &[u8]) -> Vec<f32> {
    if !blob.len().is_multiple_of(4) {
        return Vec::new();
    }
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
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

fn insert_schema_note(
    conn: &Connection,
    proposal: &SchemaProposal,
    now: &str,
) -> AppResult<String> {
    let schema_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO notes (id, title, content, created_at, updated_at, node_type, consolidation_score)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            schema_id,
            proposal.title,
            proposal.content,
            now,
            now,
            NodeType::Schema.as_str(),
            proposal.mean_similarity as f64,
        ],
    )?;
    Ok(schema_id)
}

#[derive(Debug)]
enum ProxyDecision {
    Accept { detail: String },
    Reject { detail: String },
    Unavailable { detail: String },
}

fn evaluate_proxy(
    conn: &Connection,
    proposal: &SchemaProposal,
    cfg: &ProxyGateConfig,
) -> AppResult<ProxyDecision> {
    let queries = held_out_queries(conn, &proposal.source_ids, cfg.sample_limit)?;
    if queries.is_empty() {
        return Ok(ProxyDecision::Unavailable {
            detail: "no held-out query_context on source episodes".into(),
        });
    }

    let mut deltas = Vec::new();
    for q in &queries {
        let (ctx_without, ids_without) = fts_context(conn, q, cfg.fts_top_k)?;
        let score_without = proxy_score(q, &ctx_without, &ids_without, &proposal.source_ids);

        let mut ctx_with = String::new();
        ctx_with.push_str(&proposal.title);
        ctx_with.push('\n');
        ctx_with.push_str(&proposal.content);
        ctx_with.push('\n');
        ctx_with.push_str(&ctx_without);
        // Schema inclusion covers the cluster — that is the point of the page.
        let mut ids_with = ids_without.clone();
        for id in &proposal.source_ids {
            if !ids_with.iter().any(|x| x == id) {
                ids_with.push(id.clone());
            }
        }
        let score_with = proxy_score(q, &ctx_with, &ids_with, &proposal.source_ids);
        deltas.push(score_with - score_without);
    }

    let mean_delta = deltas.iter().sum::<f32>() / deltas.len() as f32;
    let detail = format!(
        "n_queries={} mean_delta={mean_delta:.3} min_delta={:.3} (retrieve-context proxy, not WikiSkill task-accuracy)",
        queries.len(),
        cfg.min_delta
    );
    if mean_delta > cfg.min_delta {
        Ok(ProxyDecision::Accept { detail })
    } else {
        Ok(ProxyDecision::Reject { detail })
    }
}

fn held_out_queries(
    conn: &Connection,
    source_ids: &[String],
    limit: usize,
) -> AppResult<Vec<String>> {
    if source_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = source_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    let limit_idx = source_ids.len() + 1;
    let sql = format!(
        "SELECT DISTINCT query_context FROM note_access_log
         WHERE note_id IN ({placeholders})
           AND query_context IS NOT NULL
           AND TRIM(query_context) != ''
         ORDER BY accessed_at DESC
         LIMIT ?{limit_idx}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params_vec: Vec<&dyn rusqlite::types::ToSql> = Vec::new();
    for id in source_ids {
        params_vec.push(id);
    }
    let lim = limit as i64;
    params_vec.push(&lim);
    let rows = stmt.query_map(params_vec.as_slice(), |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// FTS assemble for gating only. Does NOT write `note_access_log`.
fn fts_context(conn: &Connection, query: &str, top_k: usize) -> AppResult<(String, Vec<String>)> {
    let q_safe = sanitize_fts5_query(query);
    let mut stmt = conn.prepare(
        "SELECT n.id, n.title, n.content
         FROM notes n
         JOIN notes_fts fts ON n.rowid = fts.rowid
         WHERE notes_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![q_safe, top_k as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut context = String::new();
    let mut ids = Vec::new();
    for row in rows {
        let (id, title, content) = row?;
        context.push_str(&title);
        context.push('\n');
        context.push_str(&content);
        context.push('\n');
        ids.push(id);
    }
    Ok((context, ids))
}

fn significant_tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

fn parse_proposal_reason(reason: &str) -> Option<SchemaProposal> {
    let json = reason.strip_prefix(PROPOSAL_PREFIX)?;
    serde_json::from_str(json).ok()
}

fn resolved_proposal_ids(conn: &Connection) -> AppResult<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT reason FROM consolidation_events
         WHERE event_type IN ('promoted_to_schema', 'schema_proposal_rejected')",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut ids = HashSet::new();
    for row in rows {
        let reason = row?;
        if let Some(rest) = reason.strip_prefix("proposal_id=") {
            if let Some(id) = rest.split_whitespace().next() {
                ids.insert(id.to_string());
            }
        } else if let Some(idx) = reason.find("proposal_id=") {
            let rest = &reason[idx + "proposal_id=".len()..];
            if let Some(id) = rest.split_whitespace().next() {
                ids.insert(id.to_string());
            }
        }
    }
    Ok(ids)
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

    struct MockAbstractor {
        title: String,
    }

    impl SchemaAbstractor for MockAbstractor {
        fn abstract_cluster(&self, input: &AbstractionInput) -> AppResult<AbstractionOutput> {
            Ok(AbstractionOutput {
                title: self.title.clone(),
                content: format!("mocked abstract over {} episodes", input.episodes.len()),
                rationale: "mock llm: cluster is about aspirin adverse events".into(),
                action: ProposalAction::Create,
                sibling_schema_id: None,
            })
        }
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
    fn groundedness_counts_query_tokens() {
        let g = groundedness("aspirin 81mg daily", "patient continues aspirin 81mg daily");
        assert!(g > 0.9, "got {g}");
    }

    #[test]
    fn proxy_score_rewards_schema_coverage() {
        let q = "aspirin daily";
        let without_ctx = "unrelated protocol note";
        let with_ctx = "Schema: AE aspirin\npatient continues aspirin 81mg daily";
        let sources = vec!["e1".into(), "e2".into(), "e3".into()];
        let s_without = proxy_score(q, without_ctx, &["other".into()], &sources);
        let s_with = proxy_score(q, with_ctx, &sources, &sources);
        assert!(s_with > s_without + 0.05, "{s_with} vs {s_without}");
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

    fn three_similar(db: &Database) -> Vec<String> {
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
    fn extractive_schema_writes_lineage_and_does_not_delete_episodes() {
        let db = Database::new(":memory:").unwrap();
        let ids = three_similar(&db);
        let before: i64 = db
            .execute(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?))
            .unwrap();

        let report = db
            .execute(|conn| {
                form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Extractive,
                        auto_commit: AutoCommit::Immediate,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    None,
                    None,
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

        let after: i64 = db
            .execute(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(after, before + 1, "commit adds a schema; never deletes");
    }

    #[test]
    fn form_schemas_allowlist_excludes_unlisted_episodes() {
        let db = Database::new(":memory:").unwrap();
        let ids = three_similar(&db);

        let report = db
            .execute(|conn| {
                form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Extractive,
                        auto_commit: AutoCommit::Immediate,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    Some(&[]),
                    None,
                )
            })
            .unwrap();
        assert!(report.created.is_empty());
        for id in &ids {
            assert!(db.get_note(id).unwrap().parent_schema_id.is_none());
        }
    }

    #[test]
    fn conservative_never_writes_a_schema_note() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        let report = db
            .execute(|conn| {
                form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Extractive,
                        auto_commit: AutoCommit::Never,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    None,
                    None,
                )
            })
            .unwrap();
        assert!(report.created.is_empty());
        assert_eq!(report.pending.len(), 1);
        let schemas: i64 = db
            .execute(|conn| {
                Ok(conn.query_row(
                    "SELECT COUNT(*) FROM notes WHERE node_type = 'schema'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(schemas, 0, "pending proposal must not leak into notes");
    }

    #[test]
    fn mock_llm_abstractor_is_used_and_rationale_is_stored() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        let mock = MockAbstractor {
            title: "Aspirin AE schema".into(),
        };
        let report = db
            .execute(|conn| {
                form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Llm,
                        auto_commit: AutoCommit::Immediate,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    None,
                    Some(&mock),
                )
            })
            .unwrap();
        assert_eq!(report.created[0].title, "Aspirin AE schema");
        let reason: String = db
            .execute(|conn| {
                Ok(conn.query_row(
                    "SELECT reason FROM consolidation_events
                     WHERE event_type = 'promoted_to_schema'
                     ORDER BY created_at DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert!(reason.contains("gating=immediate_extractive"));
        assert!(reason.contains("mock llm"));
    }

    #[test]
    fn llm_mode_without_abstractor_does_not_commit() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        let report = db
            .execute(|conn| {
                form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Llm,
                        auto_commit: AutoCommit::Never,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    None,
                    None,
                )
            })
            .unwrap();
        assert!(report.created.is_empty());
        assert!(report.pending[0].rationale.contains("llm_unavailable"));
    }

    #[test]
    fn reject_does_not_touch_already_committed_schema() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        let (schema_id, pending) = db
            .execute(|conn| {
                let committed = form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Extractive,
                        auto_commit: AutoCommit::Immediate,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    None,
                    None,
                )?;
                let schema_id = committed.created[0].schema_id.clone();
                let extra = SchemaProposal {
                    id: Uuid::new_v4().to_string(),
                    title: "Should not land".into(),
                    content: "rejected body".into(),
                    source_ids: committed.created[0].source_ids.clone(),
                    mean_similarity: 0.9,
                    rationale: "bad proposal".into(),
                    action: ProposalAction::Create,
                    sibling_schema_id: None,
                    abstractor: "extractive".into(),
                };
                persist_proposal_flagged(conn, &extra)?;
                reject_proposal(conn, &extra, "tester", "does not improve retrieval")?;
                Ok((schema_id, extra.id))
            })
            .unwrap();

        let schema = db.get_note(&schema_id).unwrap();
        assert_eq!(schema.node_type, NodeType::Schema);
        assert!(!schema.content.contains("rejected body"));
        let rogue: i64 = db
            .execute(|conn| {
                Ok(conn.query_row(
                    "SELECT COUNT(*) FROM notes WHERE title = 'Should not land'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(rogue, 0);
        let rejected: i64 = db
            .execute(|conn| {
                Ok(conn.query_row(
                    "SELECT COUNT(*) FROM consolidation_events
                     WHERE event_type = 'schema_proposal_rejected'
                       AND reason LIKE ?1",
                    params![format!("%{pending}%")],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(rejected, 1);
    }

    #[test]
    fn human_accept_records_human_approved_signal() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        db.execute(|conn| {
            form_schemas(
                conn,
                &SchemaFormationConfig {
                    min_cluster_size: 3,
                    min_similarity: 0.9,
                    mode: AbstractionMode::Extractive,
                    auto_commit: AutoCommit::Never,
                    ..SchemaFormationConfig::default()
                },
                false,
                None,
                None,
            )?;
            let pending = list_pending_proposals(conn)?;
            assert_eq!(pending.len(), 1);
            commit_proposal(
                conn,
                &pending[0],
                &GatingSignal::HumanApproved {
                    by: "auditor".into(),
                },
            )?;
            let reason: String = conn.query_row(
                "SELECT reason FROM consolidation_events
                 WHERE event_type = 'promoted_to_schema'
                   AND note_id IN (SELECT id FROM notes WHERE node_type = 'schema')
                 LIMIT 1",
                [],
                |r| r.get(0),
            )?;
            assert!(
                reason.contains("gating=human_approved"),
                "audit must state the human signal: {reason}"
            );
            assert!(!reason.contains("proxy_retrieve"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn proxy_accepts_when_held_out_queries_improve() {
        let db = Database::new(":memory:").unwrap();
        let ids = three_similar(&db);
        db.execute(|conn| {
            for id in &ids {
                crate::features::consolidation::log_access(
                    conn,
                    id,
                    crate::features::consolidation::AccessKind::SearchHit,
                    Some("aspirin 81mg daily"),
                    None,
                )?;
            }
            let report = form_schemas(
                conn,
                &SchemaFormationConfig {
                    min_cluster_size: 3,
                    min_similarity: 0.9,
                    mode: AbstractionMode::Extractive,
                    auto_commit: AutoCommit::Proxy,
                    proxy: ProxyGateConfig {
                        min_delta: 0.05,
                        sample_limit: 8,
                        fts_top_k: 1,
                    },
                },
                false,
                None,
                None,
            )?;
            assert_eq!(
                report.created.len(),
                1,
                "proxy should accept: {:?}",
                report.proxy_rejected
            );
            let reason: String = conn.query_row(
                "SELECT reason FROM consolidation_events
                 WHERE event_type = 'promoted_to_schema'
                   AND note_id IN (SELECT id FROM notes WHERE node_type = 'schema')
                 LIMIT 1",
                [],
                |r| r.get(0),
            )?;
            assert!(reason.contains("gating=proxy_retrieve_accepted"));
            assert!(reason.contains("not WikiSkill task-accuracy"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn proxy_without_query_context_flags_not_promotes() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        db.execute(|conn| {
            let report = form_schemas(
                conn,
                &SchemaFormationConfig {
                    min_cluster_size: 3,
                    min_similarity: 0.9,
                    mode: AbstractionMode::Extractive,
                    auto_commit: AutoCommit::Proxy,
                    ..SchemaFormationConfig::default()
                },
                false,
                None,
                None,
            )?;
            assert!(report.created.is_empty());
            assert_eq!(report.pending.len(), 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn parse_llm_json_is_atomic_one_schema() {
        let input = AbstractionInput {
            episodes: vec![EpisodeExcerpt {
                id: "e1".into(),
                title: "A".into(),
                excerpt: "x".into(),
            }],
            siblings: vec![],
            mean_similarity: 0.9,
        };
        let out = parse_llm_abstraction(
            "here you go\n{\"title\":\"T\",\"content\":\"Body\",\"rationale\":\"because\",\"action\":\"create\"}\n",
            &input,
        )
        .unwrap();
        assert_eq!(out.title, "T");
        assert_eq!(out.action, ProposalAction::Create);
    }

    struct FailAbstractor;

    impl SchemaAbstractor for FailAbstractor {
        fn abstract_cluster(&self, _input: &AbstractionInput) -> AppResult<AbstractionOutput> {
            Err(AppError::BadRequest("ollama down".into()))
        }
    }

    #[test]
    fn llm_failure_flags_and_never_commits_extractive() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        db.execute(|conn| {
            let report = form_schemas(
                conn,
                &SchemaFormationConfig {
                    min_cluster_size: 3,
                    min_similarity: 0.9,
                    mode: AbstractionMode::Llm,
                    auto_commit: AutoCommit::Proxy,
                    ..SchemaFormationConfig::default()
                },
                false,
                None,
                Some(&FailAbstractor),
            )?;
            assert!(
                report.created.is_empty(),
                "must not write a schema: {:?}",
                report.created
            );
            assert_eq!(report.pending.len(), 1);
            assert!(
                report.pending[0].rationale.starts_with("llm_failed:"),
                "{}",
                report.pending[0].rationale
            );
            let schemas: i64 = conn.query_row(
                "SELECT COUNT(*) FROM notes WHERE node_type = 'schema'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(schemas, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn explain_schema_returns_lineage() {
        let db = Database::new(":memory:").unwrap();
        three_similar(&db);
        let schema_id = db
            .execute(|conn| {
                let report = form_schemas(
                    conn,
                    &SchemaFormationConfig {
                        min_cluster_size: 3,
                        min_similarity: 0.9,
                        mode: AbstractionMode::Extractive,
                        auto_commit: AutoCommit::Immediate,
                        ..SchemaFormationConfig::default()
                    },
                    false,
                    None,
                    None,
                )?;
                Ok(report.created[0].schema_id.clone())
            })
            .unwrap();
        let explain = db.execute(|conn| explain_schema(conn, &schema_id)).unwrap();
        assert_eq!(explain.sources.len(), 3);
        assert!(!explain.events.is_empty());
    }
}

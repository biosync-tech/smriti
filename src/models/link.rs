use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Edge type for links between notes.
/// Research ref: MAGMA arXiv:2601.03236 §3 — typed relational views.
///
/// Healthcare layer types (for clinical knowledge graphs):
///   Treats        — drug treats condition
///   Contraindicts — drug contraindicated for condition
///   Interacts     — drug interacts with drug
///   Indicates     — symptom indicates condition
///   Causes        — condition/drug causes condition (comorbidity / side-effect)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    // ── Core ─────────────────────────────────────────────────────────────
    WikiLink,
    Backlink,
    Tag,
    AiSuggested,
    // ── Graph layers (MAGMA arXiv:2601.03236) ────────────────────────────
    Semantic,
    Causal,
    Temporal,
    // ── Healthcare ───────────────────────────────────────────────────────
    Treats,
    Contraindicts,
    Interacts,
    Indicates,
    Causes,
    // ── Document ingestion (Path A: local KG) ────────────────────────────
    /// chunk_note → parent document_note (provenance for ingest_document)
    ChunkOf,
    // ── Escape hatch ─────────────────────────────────────────────────────
    Custom(String),
}

impl LinkType {
    pub fn as_str(&self) -> &str {
        match self {
            LinkType::WikiLink => "wikilink",
            LinkType::Backlink => "backlink",
            LinkType::Tag => "tag",
            LinkType::AiSuggested => "ai_suggested",
            LinkType::Semantic => "semantic",
            LinkType::Causal => "causal",
            LinkType::Temporal => "temporal",
            LinkType::Treats => "treats",
            LinkType::Contraindicts => "contraindicts",
            LinkType::Interacts => "interacts",
            LinkType::Indicates => "indicates",
            LinkType::Causes => "causes",
            LinkType::ChunkOf => "chunk_of",
            LinkType::Custom(s) => s.as_str(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "wikilink" => LinkType::WikiLink,
            "backlink" => LinkType::Backlink,
            "tag" => LinkType::Tag,
            "ai_suggested" => LinkType::AiSuggested,
            "semantic" => LinkType::Semantic,
            "causal" => LinkType::Causal,
            "temporal" => LinkType::Temporal,
            "treats" => LinkType::Treats,
            "contraindicts" => LinkType::Contraindicts,
            "interacts" => LinkType::Interacts,
            "indicates" => LinkType::Indicates,
            "causes" => LinkType::Causes,
            "chunk_of" => LinkType::ChunkOf,
            other => LinkType::Custom(other.to_string()),
        }
    }

    /// Parse a comma-separated filter string into a set of link types.
    /// e.g. "semantic,causal" → [Semantic, Causal]
    pub fn parse_filter(s: &str) -> Vec<Self> {
        s.split(',').map(|t| Self::parse(t.trim())).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: String,
    pub source_note_id: String,
    pub target_note_id: String,
    pub link_type: LinkType,
    pub created_at: DateTime<Utc>,
    /// When this relationship became valid (bi-temporal model).
    /// Research ref: Zep/Graphiti arXiv:2501.13956 §3.2
    pub valid_from: Option<DateTime<Utc>>,
    /// When this relationship stopped being valid. NULL = currently valid.
    pub valid_until: Option<DateTime<Utc>>,
}

impl Link {
    pub fn new(source: String, target: String, link_type: LinkType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            source_note_id: source,
            target_note_id: target,
            link_type,
            created_at: now,
            valid_from: Some(now),
            valid_until: None,
        }
    }

    /// Whether the link is currently valid (valid_until is None or in the future).
    pub fn is_currently_valid(&self) -> bool {
        match self.valid_until {
            None => true,
            Some(until) => until > Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    pub tag_count: usize,
    pub link_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub link_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_notes: usize,
    pub total_links: usize,
    pub total_tags: usize,
    pub orphan_notes: usize,
    pub most_linked: Option<String>,
}

/// A node in the full graph export (includes created_at for pulse animation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullGraphNode {
    pub id: String,
    pub title: String,
    pub tag_count: usize,
    pub link_count: usize,
    pub created_at: String,
    /// First tag name, used for color coding in the UI.
    pub primary_tag: Option<String>,
}

/// An edge in the full graph export (includes bi-temporal fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullGraphEdge {
    pub source: String,
    pub target: String,
    pub rel_type: String,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
}

/// Full graph payload returned by `GET /api/v1/graph/full`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullGraphData {
    pub nodes: Vec<FullGraphNode>,
    pub links: Vec<FullGraphEdge>,
    pub total_notes: usize,
    pub total_links: usize,
}

/// Richer stats returned by the web UI `/api/v1/stats` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebStats {
    pub note_count: usize,
    pub edge_count: usize,
    pub kv_count: usize,
    pub db_size_bytes: u64,
}

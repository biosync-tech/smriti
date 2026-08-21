use serde::{Deserialize, Serialize};

/// Request to store a pre-computed embedding for a note.
/// Embedding generation is CONDITIONAL — we accept vectors, not text.
#[derive(Debug, Clone, Deserialize)]
pub struct StoreEmbeddingRequest {
    /// The embedding vector (any dimensionality, must match notes_vec table).
    pub embedding: Vec<f32>,
    /// Optional model identifier for provenance tracking (e.g. "nomic-embed-text").
    pub model: Option<String>,
}

/// A semantic search result combining vector distance with note metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub distance: f64,
}

/// A hybrid search result combining FTS5 keyword score + cosine distance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub id: String,
    pub title: String,
    pub preview: String,
    /// Combined score: higher is better (summed reciprocal-rank-fusion
    /// contributions across FTS5 and semantic result sets, optionally
    /// scaled by a bounded consolidation-salience nudge). Results are
    /// sorted descending on this field.
    ///
    /// Doc correction: this previously said "lower is better," which
    /// contradicted the actual fusion (`rrf = weight / (k + rank + 1)`,
    /// summed, sorted descending in `Database::hybrid_search`).
    pub score: f64,
    /// Source of match: "fts", "semantic", or "both".
    pub match_source: String,
}

/// Query for semantic search endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct SemanticSearchQuery {
    /// The query embedding vector.
    pub embedding: Vec<f32>,
    /// Max results (default: 10).
    #[serde(default = "default_semantic_limit")]
    pub limit: usize,
}

/// Query for hybrid search endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct HybridSearchQuery {
    /// Text query for FTS5 keyword search.
    pub q: String,
    /// The query embedding vector for semantic search.
    pub embedding: Vec<f32>,
    /// Max results (default: 10).
    #[serde(default = "default_semantic_limit")]
    pub limit: usize,
    /// Weight for FTS5 score in [0.0, 1.0]. Semantic weight = 1 - fts_weight.
    /// Default: 0.5 (equal weight).
    #[serde(default = "default_fts_weight")]
    pub fts_weight: f64,
}

fn default_semantic_limit() -> usize {
    10
}

fn default_fts_weight() -> f64 {
    0.5
}

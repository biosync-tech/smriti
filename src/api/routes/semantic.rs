use axum::extract::{Path, State};
use axum::Json;

use crate::api::server::AppState;
use crate::errors::AppError;
use crate::models::*;

/// POST /api/v1/notes/:id/embed — Store a pre-computed embedding for a note.
///
/// Accepts a vector of any dimensionality (must match the notes_vec table, default 384).
/// Does NOT generate embeddings — the caller must provide them.
/// Embedding generation strategy: CONDITIONAL via Ollama API (external, user-provided).
pub async fn store_embedding(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<StoreEmbeddingRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req.embedding.is_empty() {
        return Err(AppError::BadRequest(
            "Embedding vector cannot be empty".into(),
        ));
    }

    state
        .db
        .store_embedding(&id, &req.embedding, req.model.as_deref())?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "note_id": id,
        "dimensions": req.embedding.len(),
        "model": req.model,
    })))
}

/// POST /api/v1/notes/search/semantic — KNN search using a query embedding.
pub async fn semantic_search(
    State(state): State<AppState>,
    Json(query): Json<SemanticSearchQuery>,
) -> Result<Json<Vec<SemanticSearchResult>>, AppError> {
    if query.embedding.is_empty() {
        return Err(AppError::BadRequest(
            "Query embedding cannot be empty".into(),
        ));
    }

    let results = state.db.semantic_search(&query.embedding, query.limit)?;
    Ok(Json(results))
}

/// POST /api/v1/notes/search/hybrid — Combined FTS5 + semantic search with RRF re-ranking.
///
/// Research ref: Graph-Based Memory Survey arXiv:2602.05665 —
/// "Graph+BM25 hybrid beats pure vector for multi-hop tasks"
pub async fn hybrid_search(
    State(state): State<AppState>,
    Json(query): Json<HybridSearchQuery>,
) -> Result<Json<Vec<HybridSearchResult>>, AppError> {
    if query.q.is_empty() {
        return Err(AppError::BadRequest("Text query cannot be empty".into()));
    }
    if query.embedding.is_empty() {
        return Err(AppError::BadRequest(
            "Query embedding cannot be empty".into(),
        ));
    }

    let results =
        state
            .db
            .hybrid_search(&query.q, &query.embedding, query.limit, query.fts_weight)?;
    Ok(Json(results))
}

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::api::server::AppState;
use crate::errors::AppError;
use crate::models::{GraphData, GraphStats, LinkType};

#[derive(Debug, Deserialize)]
pub struct SubgraphQuery {
    #[serde(default = "default_depth")]
    pub depth: usize,
    /// Comma-separated link type filter, e.g. "semantic,causal".
    /// If omitted, all edge types are traversed.
    pub link_type: Option<String>,
}

fn default_depth() -> usize {
    2
}

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    /// Source note ID
    pub from: String,
    /// Target note ID
    pub to: String,
    /// Comma-separated link type filter for path traversal.
    /// If omitted, all edge types are used.
    pub via: Option<String>,
}

/// GET /api/v1/graph — Full knowledge graph
pub async fn get_full_graph(State(state): State<AppState>) -> Result<Json<GraphData>, AppError> {
    let mut cache = state.graph_cache.write().await;
    let kg = cache.ensure_fresh(&state.db)?;
    let mut graph_data = kg.export();

    // Fill in total tags from stats
    let stats = state.db.get_stats()?;
    graph_data.stats.total_tags = stats.total_tags;

    Ok(Json(graph_data))
}

/// GET /api/v1/graph/:id — Subgraph around a note, optionally filtered by link_type.
pub async fn get_subgraph(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<SubgraphQuery>,
) -> Result<Json<GraphData>, AppError> {
    let _ = state.db.get_note(&id)?;

    let mut cache = state.graph_cache.write().await;
    let kg = cache.ensure_fresh(&state.db)?;

    let graph_data = if let Some(ref type_filter) = query.link_type {
        let types: Vec<String> = LinkType::parse_filter(type_filter)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect();
        kg.export_subgraph_filtered(&id, query.depth, &types)
    } else {
        kg.export_subgraph(&id, query.depth)
    };

    Ok(Json(graph_data))
}

/// GET /api/v1/graph/path?from=X&to=Y&via=semantic,causal — Shortest path between notes.
pub async fn get_path(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> Result<Json<Vec<String>>, AppError> {
    let _ = state.db.get_note(&query.from)?;
    let _ = state.db.get_note(&query.to)?;

    let mut cache = state.graph_cache.write().await;
    let kg = cache.ensure_fresh(&state.db)?;

    let path = if let Some(ref via) = query.via {
        let types: Vec<String> = LinkType::parse_filter(via)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect();
        kg.shortest_path(&query.from, &query.to, Some(&types))
    } else {
        kg.shortest_path(&query.from, &query.to, None)
    };

    Ok(Json(path))
}

/// GET /api/v1/stats — Graph and database statistics
pub async fn get_stats(State(state): State<AppState>) -> Result<Json<GraphStats>, AppError> {
    let stats = state.db.get_stats()?;
    Ok(Json(stats))
}

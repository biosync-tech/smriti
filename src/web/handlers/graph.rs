use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::models::{FullGraphData, GraphData, LinkType};
use crate::web::response::{ok, WebError, WebResult};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
pub struct SubgraphQuery {
    #[serde(default = "default_depth")]
    pub depth: usize,
    pub layer: Option<String>,
}

fn default_depth() -> usize {
    2
}

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub from: String,
    pub to: String,
    pub via: Option<String>,
}

/// GET /api/v1/graph/:id?depth=2&layer=semantic,causal
pub async fn get_subgraph(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<SubgraphQuery>,
) -> WebResult<GraphData> {
    let _ = state.db.get_note(&id).map_err(WebError::from)?;
    let mut cache = state.graph_cache.write().await;
    let kg = cache.ensure_fresh(&state.db).map_err(WebError::from)?;

    let data = if let Some(ref layer) = query.layer {
        let types: Vec<String> = LinkType::parse_filter(layer)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect();
        kg.export_subgraph_filtered(&id, query.depth, &types)
    } else {
        kg.export_subgraph(&id, query.depth)
    };

    Ok(ok(data))
}

/// Query parameters for the full graph export.
#[derive(Debug, Deserialize)]
pub struct FullGraphQuery {
    #[serde(default = "default_full_limit")]
    pub limit: usize,
    pub q: Option<String>,
}

fn default_full_limit() -> usize {
    200
}

/// GET /api/v1/graph/full?limit=200&q=<filter>
///
/// Returns all nodes (up to `limit`, most-linked first) plus all edges between
/// those nodes.  Suitable for D3 force-graph rendering.
pub async fn get_full_graph(
    State(state): State<AppState>,
    Query(query): Query<FullGraphQuery>,
) -> WebResult<FullGraphData> {
    let data = state
        .db
        .get_full_graph(query.limit.min(500), query.q.as_deref())
        .map_err(WebError::from)?;
    Ok(ok(data))
}

/// GET /api/v1/graph/path?from=X&to=Y&via=...
pub async fn get_path(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> WebResult<Vec<String>> {
    let _ = state.db.get_note(&query.from).map_err(WebError::from)?;
    let _ = state.db.get_note(&query.to).map_err(WebError::from)?;
    let mut cache = state.graph_cache.write().await;
    let kg = cache.ensure_fresh(&state.db).map_err(WebError::from)?;

    let path = if let Some(ref via) = query.via {
        let types: Vec<String> = LinkType::parse_filter(via)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect();
        kg.shortest_path(&query.from, &query.to, Some(&types))
    } else {
        kg.shortest_path(&query.from, &query.to, None)
    };

    Ok(ok(path))
}

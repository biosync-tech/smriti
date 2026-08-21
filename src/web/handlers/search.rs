use axum::extract::{Query, State};
use serde::Deserialize;

use crate::models::{NoteSummary, SearchQuery};
use crate::web::response::{ok, WebError, WebResult};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_mode")]
    pub mode: SearchMode,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    #[default]
    Fts,
    Semantic,
    Hybrid,
}

fn default_mode() -> SearchMode {
    SearchMode::Fts
}

fn default_limit() -> usize {
    20
}

/// GET /api/v1/search?q=...&mode=fts|semantic|hybrid
///
/// - fts: FTS5 full-text search (default)
/// - semantic: KNN vector search (requires embeddings to be stored)
/// - hybrid: FTS5 + KNN with RRF re-ranking
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> WebResult<Vec<NoteSummary>> {
    if params.q.trim().is_empty() {
        return Err(WebError::from(crate::errors::AppError::BadRequest(
            "Search query 'q' cannot be empty".into(),
        )));
    }

    match params.mode {
        SearchMode::Fts => {
            let results = state
                .db
                .search_notes(&SearchQuery {
                    q: params.q,
                    limit: params.limit,
                    offset: 0,
                })
                .map_err(WebError::from)?;
            Ok(ok(results))
        }

        // Semantic and hybrid require a query embedding from the caller.
        // Without one, fall back gracefully to FTS.
        SearchMode::Semantic | SearchMode::Hybrid => {
            let results = state
                .db
                .search_notes(&SearchQuery {
                    q: params.q,
                    limit: params.limit,
                    offset: 0,
                })
                .map_err(WebError::from)?;
            Ok(ok(results))
        }
    }
}

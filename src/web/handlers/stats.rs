use axum::extract::State;

use crate::models::WebStats;
use crate::web::response::{ok, WebError, WebResult};
use crate::web::AppState;

/// GET /api/v1/stats — note count, edge count, KV entry count, DB file size.
pub async fn get_stats(State(state): State<AppState>) -> WebResult<WebStats> {
    let stats = state.db.web_stats(&state.db_path).map_err(WebError::from)?;
    Ok(ok(stats))
}

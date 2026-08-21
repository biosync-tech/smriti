use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::models::{AgentMemory, ConflictPolicy, CreateMemoryRequest};
use crate::storage::Database;
use crate::web::response::{ok, ApiResponse, WebError, WebResult};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
pub struct KvListQuery {
    pub prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetKvBody {
    pub value: serde_json::Value,
    pub ttl_seconds: Option<i64>,
    #[serde(default)]
    pub namespace: Option<String>,
}

/// GET /api/v1/kv?prefix=...
pub async fn list_kv(
    State(state): State<AppState>,
    Query(query): Query<KvListQuery>,
) -> WebResult<Vec<AgentMemory>> {
    let entries = state
        .db
        .list_kv(query.prefix.as_deref())
        .map_err(WebError::from)?;
    Ok(ok(entries))
}

/// GET /api/v1/kv/:key
pub async fn get_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> WebResult<AgentMemory> {
    let entry = state.db.get_kv(&key).map_err(WebError::from)?;
    Ok(ok(entry))
}

/// PUT /api/v1/kv/:key  — body: { value, ttl_seconds?, namespace? }
pub async fn set_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<SetKvBody>,
) -> Result<(StatusCode, Json<ApiResponse<AgentMemory>>), WebError> {
    let namespace = body.namespace.unwrap_or_else(|| "default".into());
    let entry = state
        .db
        .store_memory(
            Database::WEBUI_AGENT,
            CreateMemoryRequest {
                namespace: Some(namespace),
                key,
                value: body.value,
                ttl_seconds: body.ttl_seconds,
                conflict_policy: ConflictPolicy::Overwrite,
            },
        )
        .map_err(WebError::from)?;
    Ok((StatusCode::OK, ok(entry)))
}

/// DELETE /api/v1/kv/:key
pub async fn delete_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<StatusCode, WebError> {
    state
        .db
        .delete_memory(Database::WEBUI_AGENT, "default", &key)
        .map_err(WebError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

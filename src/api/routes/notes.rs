use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::ai::document_ingest::{DocumentIngestor, IngestDocumentRequest, IngestDocumentResponse};
use crate::api::server::AppState;
use crate::errors::AppError;
use crate::models::*;
use crate::parser;

/// POST /api/v1/notes
pub async fn create_note(
    State(state): State<AppState>,
    Json(mut req): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<Note>), AppError> {
    // Auto-extract tags from content
    let content_tags = parser::extract_tags(&req.content);
    for tag in content_tags {
        if !req.tags.contains(&tag) {
            req.tags.push(tag);
        }
    }

    // Extract frontmatter tags
    if let Some((fm, _)) = parser::parse_frontmatter(&req.content) {
        for tag in fm.tags {
            if !req.tags.contains(&tag) {
                req.tags.push(tag);
            }
        }
    }

    let note = state.db.create_note(req)?;

    // Process wiki-links and create link records with inferred type
    let wikilinks = parser::extract_wikilinks(&note.content);
    for wl in &wikilinks {
        if let Ok(Some(target)) = state.db.get_note_by_title(&wl.target) {
            let link_type = LinkType::parse(&wl.relation);
            let _ = state.db.create_link(&note.id, &target.id, link_type);
        }
    }

    // Invalidate the graph cache so the next query rebuilds
    state.graph_cache.write().await.invalidate();

    Ok((StatusCode::CREATED, Json(note)))
}

/// GET /api/v1/notes
pub async fn list_notes(
    State(state): State<AppState>,
    Query(query): Query<NoteListQuery>,
) -> Result<Json<Vec<NoteSummary>>, AppError> {
    let notes = state.db.list_notes(&query)?;
    Ok(Json(notes))
}

/// GET /api/v1/notes/:id
pub async fn get_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Note>, AppError> {
    let note = state.db.get_note(&id)?;
    Ok(Json(note))
}

/// PUT /api/v1/notes/:id
pub async fn update_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNoteRequest>,
) -> Result<Json<Note>, AppError> {
    let note = state.db.update_note(&id, req)?;

    // Re-process wiki-links with inferred type
    let wikilinks = parser::extract_wikilinks(&note.content);
    for wl in &wikilinks {
        if let Ok(Some(target)) = state.db.get_note_by_title(&wl.target) {
            let link_type = LinkType::parse(&wl.relation);
            let _ = state.db.create_link(&note.id, &target.id, link_type);
        }
    }

    // Invalidate the graph cache so the next query rebuilds
    state.graph_cache.write().await.invalidate();

    Ok(Json(note))
}

/// DELETE /api/v1/notes/:id
pub async fn delete_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    state.db.delete_note(&id)?;

    // Invalidate the graph cache so the next query rebuilds
    state.graph_cache.write().await.invalidate();

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/notes/search?q=...
pub async fn search_notes(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<NoteSummary>>, AppError> {
    if query.q.is_empty() {
        return Err(AppError::BadRequest("Search query cannot be empty".into()));
    }
    let results = state.db.search_notes(&query)?;
    Ok(Json(results))
}

/// GET /api/v1/notes/:id/backlinks
pub async fn get_backlinks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<NoteSummary>>, AppError> {
    let backlinks = state.db.get_backlinks(&id)?;
    Ok(Json(backlinks))
}

/// GET /api/v1/notes/:id/links
pub async fn get_forward_links(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<NoteSummary>>, AppError> {
    let links = state.db.get_forward_links(&id)?;
    Ok(Json(links))
}

/// POST /api/v1/ingest/document — ingest a local .txt or .md file into the KG
pub async fn ingest_document(
    State(state): State<AppState>,
    Json(req): Json<IngestDocumentRequest>,
) -> Result<(StatusCode, Json<IngestDocumentResponse>), AppError> {
    let resp = DocumentIngestor::ingest(&state.db, &req)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Invalidate graph cache — new notes + edges were added
    state.graph_cache.write().await.invalidate();

    Ok((StatusCode::CREATED, Json(resp)))
}

/// POST /api/v1/retrieve — assemble context for a local LLM (no LLM in Smriti)
///
/// Wraps the retrieve_context MCP logic as a REST endpoint.
/// The caller provides a query (and optionally a pre-computed embedding);
/// Smriti returns assembled context + ranked sources.
pub async fn retrieve_context(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = crate::mcp::handlers::handle_retrieve_context(&state.db, &req)
        .map_err(AppError::BadRequest)?;
    Ok(Json(result))
}

/// POST /api/v1/notes/:id/tags
pub async fn add_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(tags): Json<Vec<String>>,
) -> Result<Json<Note>, AppError> {
    // Get existing note
    let existing = state.db.get_note(&id)?;
    let mut all_tags = existing.tags.clone();
    for tag in tags {
        if !all_tags.contains(&tag) {
            all_tags.push(tag);
        }
    }

    let note = state.db.update_note(
        &id,
        UpdateNoteRequest {
            title: None,
            content: None,
            tags: Some(all_tags),
        },
    )?;

    Ok(Json(note))
}
